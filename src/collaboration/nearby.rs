//! Opt-in, local-network discovery. Only device metadata is multicast; private
//! invitations travel over a separate pinned-key, encrypted Iroh connection.
//! Receiving an offer never joins a document without the user's approval.
use super::{endpoint_for, live::Invite};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const ALPN: &[u8] = b"alphadocs/nearby/1";
const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 77, 77);
const PORT: u16 = 44764;
const LIMIT: usize = 4096;

#[derive(Clone)]
pub struct Device {
    pub id: EndpointId,
    pub name: String,
    pub platform: String,
    address: SocketAddr,
    seen: Instant,
}
impl Device {
    pub fn short_id(&self) -> String {
        self.id.to_string().chars().take(8).collect()
    }
}
#[derive(Clone)]
pub struct Offer {
    pub sender: String,
    pub name: String,
    pub title: String,
    pub viewer: bool,
    pub invite: Invite,
    received: Instant,
}
impl Offer {
    pub fn same_request(&self, other: &Self) -> bool {
        self.sender == other.sender
            && self.received == other.received
            && self.received.elapsed() < Duration::from_secs(60)
    }
}
#[derive(Default)]
pub struct State {
    pub id: String,
    pub devices: Vec<Device>,
    pub offers: VecDeque<Offer>,
    pub status: String,
}
pub struct Nearby {
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    send: mpsc::SyncSender<(Device, WireOffer)>,
}
impl Drop for Nearby {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
#[derive(Serialize, Deserialize)]
struct Beacon {
    version: u8,
    key: String,
    port: u16,
    name: String,
    platform: String,
}
#[derive(Serialize, Deserialize)]
struct WireOffer {
    name: String,
    title: String,
    viewer: bool,
    invite: String,
}
fn valid_name(s: &str, limit: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= limit && !s.chars().any(char::is_control)
}
fn local(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || ip.is_loopback()
}
fn decode_beacon(bytes: &[u8], from: SocketAddr) -> Option<Device> {
    if bytes.len() > 768 || !matches!(from.ip(), IpAddr::V4(ip) if local(ip)) {
        return None;
    }
    let b: Beacon = serde_json::from_slice(bytes).ok()?;
    if b.version != 1 || b.port == 0 || !valid_name(&b.name, 40) || !valid_name(&b.platform, 16) {
        return None;
    }
    Some(Device {
        id: b.key.parse().ok()?,
        name: b.name,
        platform: b.platform,
        address: SocketAddr::new(from.ip(), b.port),
        seen: Instant::now(),
    })
}
fn queue(state: &mut State, sender: String, wire: WireOffer) -> Result<(), String> {
    if !valid_name(&wire.name, 40) || !valid_name(&wire.title, 160) {
        return Err("Invalid invitation details".into());
    }
    state
        .offers
        .retain(|o| o.received.elapsed() < Duration::from_secs(60));
    if state.offers.len() >= 4 || state.offers.iter().any(|o| o.sender == sender) {
        return Err("Receiver is busy; try again later".into());
    }
    let invite = Invite::parse(&wire.invite)?;
    state.offers.push_back(Offer {
        sender,
        name: wire.name,
        title: wire.title,
        viewer: wire.viewer,
        invite,
        received: Instant::now(),
    });
    Ok(())
}
async fn receive(
    incoming: iroh::endpoint::Incoming,
    state: Arc<Mutex<State>>,
) -> Result<(), String> {
    tokio::time::timeout(Duration::from_secs(6), async {
        let conn = incoming.await.map_err(|_| "Nearby handshake failed")?;
        let (mut send, mut recv) = conn.accept_bi().await.map_err(|_| "Nearby stream failed")?;
        let bytes = recv
            .read_to_end(LIMIT)
            .await
            .map_err(|_| "Invitation is too large")?;
        let wire = serde_json::from_slice(&bytes).map_err(|_| "Invalid nearby message")?;
        let result = {
            let mut locked = state.lock().map_err(|_| "Nearby state unavailable")?;
            queue(&mut locked, conn.remote_id().to_string(), wire)
        };
        let reply = serde_json::to_vec(&result).map_err(|e| e.to_string())?;
        send.write_all(&reply).await.map_err(|e| e.to_string())?;
        send.finish().map_err(|e| e.to_string())?;
        let _ = send.stopped().await;
        Ok(())
    })
    .await
    .map_err(|_| "Nearby request timed out".to_string())?
}
async fn deliver(ep: &Endpoint, device: Device, wire: WireOffer) -> Result<(), String> {
    tokio::time::timeout(Duration::from_secs(6), async {
        let address = EndpointAddr::new(device.id).with_ip_addr(device.address);
        let conn = ep
            .connect(address, ALPN)
            .await
            .map_err(|_| "Device is unreachable; check LAN and firewall")?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&wire).map_err(|e| e.to_string())?;
        if bytes.len() > LIMIT {
            return Err("Invitation exceeds nearby limit".into());
        }
        send.write_all(&bytes).await.map_err(|e| e.to_string())?;
        send.finish().map_err(|e| e.to_string())?;
        let bytes = recv.read_to_end(LIMIT).await.map_err(|e| e.to_string())?;
        serde_json::from_slice::<Result<(), String>>(&bytes)
            .map_err(|_| "Invalid device response")?
    })
    .await
    .map_err(|_| "Device did not respond".to_string())?
}
impl Nearby {
    #[cfg(test)]
    pub(crate) fn pending_for_test(invite: Invite) -> Self {
        let (send, _) = mpsc::sync_channel(1);
        let state = Arc::new(Mutex::new(State::default()));
        state.lock().unwrap().offers.push_back(Offer {
            sender: "test-device-123".into(),
            name: "Sender QA".into(),
            title: "Incoming draft".into(),
            viewer: false,
            invite,
            received: Instant::now(),
        });
        Self {
            state,
            stop: Arc::new(AtomicBool::new(false)),
            send,
        }
    }
    pub fn start(ip: IpAddr, name: String) -> Result<Self, String> {
        let IpAddr::V4(ip) = ip else {
            return Err(
                "Nearby discovery needs a local IPv4 address; manual invitations also support IPv6"
                    .into(),
            );
        };
        if !local(ip) || !valid_name(&name, 40) {
            return Err("Choose a private LAN IPv4 address and a display name".into());
        }
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PORT)).map_err(|e| {
            format!("Cannot enable nearby discovery (another AlphaDocs may be listening): {e}")
        })?;
        socket
            .join_multicast_v4(&GROUP, &ip)
            .map_err(|e| e.to_string())?;
        socket2::SockRef::from(&socket)
            .set_multicast_if_v4(&ip)
            .map_err(|e| e.to_string())?;
        socket.set_multicast_ttl_v4(1).map_err(|e| e.to_string())?;
        socket
            .set_multicast_loop_v4(false)
            .map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let (send, commands) = mpsc::sync_channel(4);
        let state = Arc::new(Mutex::new(State {
            status: "Starting nearby discovery…".into(),
            ..Default::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, ending) = (state.clone(), stop.clone());
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(e) => {
                    if let Ok(mut s) = shared.lock() {
                        s.status = e.to_string();
                    }
                    return;
                }
            };
            let failure = runtime.block_on(async {
                let ep = endpoint_for(IpAddr::V4(ip), vec![ALPN.to_vec()]).await?;
                let address = ep.bound_sockets().into_iter().find(|a| a.ip() == IpAddr::V4(ip)).ok_or("Nearby interface was not bound")?;
                if let Ok(mut s) = shared.lock() { s.id = ep.id().to_string(); s.status = "Visible to nearby AlphaDocs devices".into(); }
                let beacon = serde_json::to_vec(&Beacon { version: 1, key: ep.id().to_string(), port: address.port(), name, platform: std::env::consts::OS.into() }).map_err(|e| e.to_string())?;
                let mut devices = HashMap::<EndpointId, Device>::new();
                let mut announced = Instant::now() - Duration::from_secs(3);
                let mut tasks = tokio::task::JoinSet::new();
                let mut buffer = [0u8; 1024];
                while !ending.load(Ordering::Acquire) {
                    if announced.elapsed() > Duration::from_secs(2) {
                        if let Err(e) = socket.send_to(&beacon, (GROUP, PORT)) && let Ok(mut s) = shared.lock() { s.status = format!("Discovery is blocked: {e}"); }
                        announced = Instant::now();
                    }
                    for _ in 0..16 {
                        match socket.recv_from(&mut buffer) {
                            Ok((n, from)) => { if let Some(device) = decode_beacon(&buffer[..n], from) && device.id != ep.id() && (devices.len() < 32 || devices.contains_key(&device.id)) { devices.insert(device.id, device); } }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(_) => break,
                        }
                    }
                    devices.retain(|_, d| d.seen.elapsed() < Duration::from_secs(8));
                    if let Ok(mut s) = shared.lock() { s.devices = devices.values().cloned().collect(); s.devices.sort_by(|a,b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id))); s.offers.retain(|o| o.received.elapsed() < Duration::from_secs(60)); }
                    if tasks.len() < 8 && let Ok((device, wire)) = commands.try_recv() {
                        let (ep, shared) = (ep.clone(), shared.clone());
                        tasks.spawn(async move { let result = deliver(&ep, device, wire).await; if let Ok(mut s) = shared.lock() { s.status = match result { Ok(()) => "Invitation sent. They must choose Join on their device.".into(), Err(e) => e }; } });
                    }
                    tokio::select! {
                        incoming = ep.accept() => { if let Some(incoming) = incoming { if tasks.len() < 8 { let state = shared.clone(); tasks.spawn(async move { let _ = receive(incoming, state).await; }); } } else { break; } },
                        _ = tasks.join_next(), if !tasks.is_empty() => {},
                        _ = tokio::time::sleep(Duration::from_millis(150)) => {},
                    }
                }
                tasks.abort_all(); ep.close().await;
                Ok::<(), String>(())
            });
            if let Err(e) = failure
                && let Ok(mut s) = shared.lock()
            {
                s.status = e;
            }
        });
        Ok(Self { state, stop, send })
    }
    pub fn invite(
        &self,
        device: Device,
        name: String,
        title: String,
        viewer: bool,
        invitation: &Invite,
    ) -> Result<(), String> {
        if device.seen.elapsed() > Duration::from_secs(8) {
            return Err("Device went offline".into());
        }
        let wire = WireOffer {
            name,
            title: title
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect(),
            viewer,
            invite: invitation.encode()?,
        };
        self.send
            .try_send((device, wire))
            .map_err(|_| "Nearby sender is busy; try again".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_rejects_public_sources_and_never_contains_document_or_capability() {
        let beacon = Beacon {
            version: 1,
            key: iroh::SecretKey::generate().public().to_string(),
            port: 1234,
            name: "Mac".into(),
            platform: "macos".into(),
        };
        let bytes = serde_json::to_vec(&beacon).unwrap();
        assert!(decode_beacon(&bytes, "192.168.1.2:44764".parse().unwrap()).is_some());
        assert!(decode_beacon(&bytes, "8.8.8.8:44764".parse().unwrap()).is_none());
        assert!(!String::from_utf8(bytes).unwrap().contains("invite"));
    }
    #[test]
    fn encrypted_offer_is_queued_until_explicit_acceptance() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let ip = "127.0.0.1".parse().unwrap();
            let receiver = endpoint_for(ip, vec![ALPN.to_vec()]).await.unwrap();
            let sender = endpoint_for(ip, vec![ALPN.to_vec()]).await.unwrap();
            let host = super::super::live::host(
                crate::model::Document::new("Nearby test"),
                ip,
                "Host".into(),
            )
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
            let invite = host.edit_invite.as_ref().unwrap();
            let state = Arc::new(Mutex::new(State::default()));
            let shared = state.clone();
            let target = Device {
                id: receiver.id(),
                name: "Mac".into(),
                platform: "macos".into(),
                address: receiver.bound_sockets()[0],
                seen: Instant::now(),
            };
            let wire = WireOffer {
                name: "Host".into(),
                title: "A private draft".into(),
                viewer: false,
                invite: invite.encode().unwrap(),
            };
            let (sent, received) = tokio::join!(deliver(&sender, target, wire), async {
                receive(receiver.accept().await.unwrap(), shared).await
            });
            sent.unwrap();
            received.unwrap();
            {
                let state = state.lock().unwrap();
                assert_eq!(state.offers.len(), 1);
                assert_eq!(state.offers[0].name, "Host");
                assert_eq!(state.offers[0].title, "A private draft");
                assert!(
                    host.room.lock().unwrap().peers.is_empty(),
                    "An offer must not auto-join the document"
                );
            }
            sender.close().await;
            receiver.close().await;
        });
    }
}
