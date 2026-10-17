//! Account-free, explicit direct-IP QUIC sessions. No discovery or relay traffic.
use super::{
    Access, endpoint,
    replica::{Anchor, Replica},
};
use crate::model::Document;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
pub(super) const ALPN: &[u8] = b"alphadocs/live/3";
const MAX_WIRE: usize = 48 * 1024 * 1024; // JSON encoding includes immutable binary assets.
const TIMEOUT: Duration = Duration::from_secs(6);
const INTERVAL: Duration = Duration::from_millis(250);
const MAX_PEERS: usize = 8;
type Pending = mpsc::Receiver<Result<LiveSession, String>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Invite {
    host: EndpointId,
    address: SocketAddr,
    capability: [u8; 32],
}
impl std::fmt::Debug for Invite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invite([redacted])")
    }
}
impl Invite {
    pub fn encode(&self) -> Result<String, String> {
        Ok(format!(
            "alphadocs://join/{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).map_err(|e| e.to_string())?)
        ))
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 2048 {
            return Err("Invitation is too long".into());
        }
        let value = text
            .trim()
            .strip_prefix("alphadocs://join/")
            .ok_or("Paste an AlphaDocs invitation")?;
        let bytes = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| "Invalid invitation")?;
        let invite: Self = serde_json::from_slice(&bytes).map_err(|_| "Invalid invitation")?;
        if invite.address.ip().is_unspecified()
            || invite.address.ip().is_multicast()
            || invite.address.port() == 0
        {
            return Err("Invitation has an invalid destination".into());
        }
        Ok(invite)
    }
    pub fn destination(&self) -> String {
        format!("{} · {}", self.address, self.host)
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Presence {
    pub name: String,
    pub caret: Option<Anchor>,
    pub anchor: Option<Anchor>,
}
#[derive(Clone)]
pub struct Participant {
    pub id: String,
    pub presence: Presence,
    pub seen: Instant,
}
struct Grant {
    capability: [u8; 32],
    access: Access,
    expires: Instant,
}
pub struct Room {
    #[cfg(test)]
    pause_transport: bool,
    pub replica: Replica,
    pub writable: bool,
    pub status: String,
    pub local: Presence,
    pub peers: HashMap<String, Participant>,
    grants: Vec<Grant>,
    pub connected: bool,
    pub rejected: bool,
}
impl Room {
    pub(crate) fn new(replica: Replica, writable: bool, name: String) -> Self {
        Self {
            #[cfg(test)]
            pause_transport: false,
            replica,
            writable,
            status: "Connecting…".into(),
            local: Presence {
                name,
                ..Default::default()
            },
            peers: HashMap::new(),
            grants: vec![],
            connected: false,
            rejected: false,
        }
    }
}
pub struct LiveSession {
    pub room: Arc<Mutex<Room>>,
    pub host: bool,
    pub edit_invite: Option<Invite>,
    pub view_invite: Option<Invite>,
    stop: Arc<AtomicBool>,
}
impl Drop for LiveSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl LiveSession {
    #[cfg(test)]
    pub(crate) fn pause_transport(&self, pause: bool) {
        self.room.lock().unwrap().pause_transport = pause;
    }
    pub fn editor_replica(&self) -> Result<Replica, String> {
        Replica::load(
            &self
                .room
                .lock()
                .map_err(|_| "Session unavailable")?
                .replica
                .save(),
        )
    }
    pub fn revoke(&self) {
        if let Ok(mut r) = self.room.lock() {
            r.grants.clear();
            r.peers.clear();
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Request {
    version: u32,
    capability: [u8; 32],
    #[serde(with = "wire_bytes")]
    vector: Vec<u8>,
    #[serde(with = "wire_bytes")]
    update: Vec<u8>,
    presence: Presence,
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Sync {
        #[serde(with = "wire_bytes")]
        vector: Vec<u8>,
        #[serde(with = "wire_bytes")]
        update: Vec<u8>,
        access: Access,
        peers: Vec<(String, Presence)>,
    },
    Denied,
    Invalid(String),
}
mod wire_bytes {
    use super::*;
    pub fn serialize<S: serde::Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&URL_SAFE_NO_PAD.encode(value))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() > 45 * 1024 * 1024 {
            return Err(serde::de::Error::custom("Update exceeds session limit"));
        }
        URL_SAFE_NO_PAD
            .decode(text)
            .map_err(|_| serde::de::Error::custom("Invalid synchronization encoding"))
    }
}
fn valid_presence(p: &Presence) -> bool {
    !p.name.trim().is_empty()
        && p.name.chars().count() <= 40
        && !p.name.chars().any(char::is_control)
        && [&p.caret, &p.anchor].iter().all(|a| {
            a.as_ref()
                .is_none_or(|a| a.cursor.len() <= 512 && a.tab.len() <= 128)
        })
}
fn process(room: &mut Room, peer: String, request: Request, host_key: &str) -> Reply {
    if request.version != 3
        || request.vector.len() > 64 * 1024
        || !valid_presence(&request.presence)
    {
        return Reply::Invalid(
            "Incompatible peer message. Update AlphaDocs on both devices".into(),
        );
    }
    let access = room
        .grants
        .iter()
        .find(|g| bool::from(g.capability.ct_eq(&request.capability)) && g.expires > Instant::now())
        .map(|g| g.access);
    let Some(access) = access else {
        return Reply::Denied;
    };
    if let Err(error) = room.replica.diff(&request.vector) {
        return Reply::Invalid(error);
    }
    room.peers
        .retain(|_, p| p.seen.elapsed() < Duration::from_secs(10));
    if !room.peers.contains_key(&peer) && room.peers.len() >= MAX_PEERS {
        return Reply::Invalid("Session is full (8 guests)".into());
    }
    // A viewer's clock never grants edit rights, even when the UI is modified.
    if access == Access::View && request.update != [0, 0] {
        return Reply::Denied;
    }
    if access == Access::Edit
        && let Err(error) = room.replica.accept(&request.update)
    {
        return Reply::Invalid(error);
    }
    let update = match room.replica.diff(&request.vector) {
        Ok(u) => u,
        Err(e) => return Reply::Invalid(e),
    };
    room.peers.insert(
        peer.clone(),
        Participant {
            id: peer,
            presence: request.presence,
            seen: Instant::now(),
        },
    );
    let mut peers: Vec<_> = room
        .peers
        .values()
        .map(|p| (p.id.clone(), p.presence.clone()))
        .collect();
    peers.push((host_key.to_owned(), room.local.clone()));
    Reply::Sync {
        vector: room.replica.vector(),
        update,
        access,
        peers,
    }
}
async fn serve(
    ep: &Endpoint,
    room: Arc<Mutex<Room>>,
    incoming: iroh::endpoint::Incoming,
) -> Result<(), String> {
    tokio::time::timeout(TIMEOUT, async {
        let connection = incoming.await.map_err(|_| "Peer handshake failed")?;
        let peer = connection.remote_id().to_string();
        let (mut send, mut recv) = connection
            .accept_bi()
            .await
            .map_err(|_| "Peer stream failed")?;
        let bytes = recv
            .read_to_end(MAX_WIRE)
            .await
            .map_err(|_| "Peer request too large or incomplete")?;
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid peer message")?;
        let reply = {
            let mut r = room.lock().map_err(|_| "Session unavailable")?;
            process(&mut r, peer, request, &ep.id().to_string())
        };
        let bytes = serde_json::to_vec(&reply).map_err(|_| "Could not encode synchronization")?;
        if bytes.len() > MAX_WIRE {
            return Err("Shared document exceeds transfer limit".to_string());
        }
        send.write_all(&bytes)
            .await
            .map_err(|_| "Peer disconnected")?;
        send.finish()
            .map_err(|_| "Could not finish synchronization")?;
        let _ = send.stopped().await;
        Ok(())
    })
    .await
    .map_err(|_| "Peer request timed out".to_string())?
}
async fn request(ep: &Endpoint, invite: &Invite, request: Request) -> Result<Reply, String> {
    tokio::time::timeout(TIMEOUT, async {
        let address = EndpointAddr::new(invite.host).with_ip_addr(invite.address);
        let connection = ep
            .connect(address, ALPN)
            .await
            .map_err(|_| "Cannot reach the host; check address, LAN/VPN and firewall")?;
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|_| "Host stream failed")?;
        let bytes = serde_json::to_vec(&request).map_err(|_| "Could not encode local changes")?;
        if bytes.len() > MAX_WIRE {
            return Err("Shared document exceeds transfer limit".to_string());
        }
        send.write_all(&bytes)
            .await
            .map_err(|_| "Host disconnected")?;
        send.finish()
            .map_err(|_| "Could not finish synchronization")?;
        let bytes = recv
            .read_to_end(MAX_WIRE)
            .await
            .map_err(|_| "Host response too large or incomplete")?;
        let reply = serde_json::from_slice(&bytes).map_err(|_| "Invalid host response")?;
        connection.close(0u8.into(), b"synchronized");
        Ok(reply)
    })
    .await
    .map_err(|_| "Host connection timed out; retrying".to_string())?
}
fn start(
    f: impl FnOnce(mpsc::SyncSender<Result<LiveSession, String>>) + Send + 'static,
) -> Pending {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || f(tx));
    rx
}
pub fn suggest_ip() -> String {
    // UDP connect chooses a local route but sends no packets. No discovery service.
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}
pub fn host(document: Document, ip: IpAddr, name: String) -> Pending {
    start(move |tx| {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        runtime.block_on(async move {
            let result = async {
                if ip.is_unspecified() || ip.is_multicast() {
                    return Err("Choose a specific local network address".into());
                }
                let ep = endpoint(ip).await?;
                let address = ep
                    .bound_sockets()
                    .into_iter()
                    .find(|a| a.ip() == ip)
                    .ok_or("Could not bind chosen interface")?;
                let room = Arc::new(Mutex::new(Room::new(Replica::new(&document)?, true, name)));
                let mut invites = vec![];
                {
                    let mut r = room.lock().map_err(|_| "Session unavailable")?;
                    for access in [Access::Edit, Access::View] {
                        let capability = rand::random();
                        r.grants.push(Grant {
                            capability,
                            access,
                            expires: Instant::now() + Duration::from_secs(3600),
                        });
                        invites.push(Invite {
                            host: ep.id(),
                            address,
                            capability,
                        });
                    }
                    r.connected = true;
                    r.status = "Hosting · encrypted direct connection".into();
                }
                let stop = Arc::new(AtomicBool::new(false));
                let session = LiveSession {
                    room: room.clone(),
                    host: true,
                    edit_invite: Some(invites.remove(0)),
                    view_invite: Some(invites.remove(0)),
                    stop: stop.clone(),
                };
                Ok((ep, room, stop, session))
            }
            .await;
            let (ep, room, stop, session) = match result {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            if tx.send(Ok(session)).is_err() {
                ep.close().await;
                return;
            }
            let mut tasks = tokio::task::JoinSet::new();
            while !stop.load(Ordering::Acquire) {
                tokio::select! {
                    incoming=ep.accept()=>{if let Some(incoming)=incoming{
                        #[cfg(test)]
                        if room.lock().is_ok_and(|r|r.pause_transport){drop(incoming);continue;}
                        if tasks.len()>=MAX_PEERS{drop(incoming);continue;}
                        let(ep,room)=(ep.clone(),room.clone());
                        tasks.spawn(async move{let _=serve(&ep,room,incoming).await;});
                    }else{break;}},
                    _=tokio::time::sleep(INTERVAL)=>{},
                    _=tasks.join_next(),if !tasks.is_empty()=>{},
                }
            }
            tasks.abort_all();
            ep.close().await;
        });
    })
}
pub fn join(invite: Invite, name: String) -> Pending {
    start(move |tx| {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
                return;
            }
        };
        runtime.block_on(async move {
            let ip = if invite.address.is_ipv4() {
                "0.0.0.0"
            } else {
                "::"
            }
            .parse()
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
            let ep = match endpoint(ip).await {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let initial = Request {
                version: 3,
                capability: invite.capability,
                vector: vec![0],
                update: vec![0, 0],
                presence: Presence {
                    name: name.clone(),
                    ..Default::default()
                },
            };
            let first = request(&ep, &invite, initial).await;
            let (vector, update, access, peers) = match first {
                Ok(Reply::Sync {
                    vector,
                    update,
                    access,
                    peers,
                }) => (vector, update, access, peers),
                Ok(Reply::Denied) => {
                    let _ = tx.send(Err("Invitation expired or was revoked".into()));
                    ep.close().await;
                    return;
                }
                Ok(Reply::Invalid(e)) | Err(e) => {
                    let _ = tx.send(Err(e));
                    ep.close().await;
                    return;
                }
            };
            let replica = match Replica::load(&update).and_then(|mut r| {
                r.project(&Document::new(""))?;
                Ok(r)
            }) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    ep.close().await;
                    return;
                }
            };
            let room = Arc::new(Mutex::new(Room::new(replica, access == Access::Edit, name)));
            if let Ok(mut r) = room.lock() {
                r.connected = true;
                r.status = "Connected · encrypted direct connection".into();
                set_peers(&mut r, peers, &ep.id().to_string());
            }
            let stop = Arc::new(AtomicBool::new(false));
            let session = LiveSession {
                room: room.clone(),
                host: false,
                edit_invite: None,
                view_invite: None,
                stop: stop.clone(),
            };
            if tx.send(Ok(session)).is_err() {
                ep.close().await;
                return;
            }
            let mut accepted_vector = vector;
            while !stop.load(Ordering::Acquire) {
                tokio::time::sleep(INTERVAL).await;
                let outgoing = room.lock().ok().and_then(|r| {
                    Some(Request {
                        version: 3,
                        capability: invite.capability,
                        vector: r.replica.vector(),
                        update: if r.writable {
                            r.replica.diff(&accepted_vector).ok()?
                        } else {
                            vec![0, 0]
                        },
                        presence: r.local.clone(),
                    })
                });
                let Some(outgoing) = outgoing else {
                    break;
                };
                let response = request(&ep, &invite, outgoing).await;
                let Ok(mut r) = room.lock() else {
                    break;
                };
                match response {
                    Ok(Reply::Sync {
                        vector,
                        update,
                        access,
                        peers,
                    }) => match r.replica.accept(&update) {
                        Ok(()) => {
                            accepted_vector = vector;
                            r.connected = true;
                            r.writable = access == Access::Edit;
                            r.status = "Connected · encrypted direct connection".into();
                            set_peers(&mut r, peers, &ep.id().to_string());
                        }
                        Err(e) => {
                            r.status = e;
                            r.connected = false;
                        }
                    },
                    Ok(Reply::Denied) => {
                        r.status = "Access ended · local copy retained".into();
                        r.connected = false;
                        r.writable = false;
                        r.rejected = true;
                        break;
                    }
                    Ok(Reply::Invalid(e)) => {
                        r.status = format!("Host rejected update: {e} · local draft retained");
                        r.connected = false;
                        r.writable = false;
                        r.rejected = true;
                        break;
                    }
                    Err(e) => {
                        r.status = format!("Reconnecting · {e} · local edits retained");
                        r.connected = false;
                        r.peers.clear();
                    }
                }
            }
            ep.close().await;
        });
    })
}
fn set_peers(room: &mut Room, peers: Vec<(String, Presence)>, local: &str) {
    room.peers = peers
        .into_iter()
        .filter(|(id, p)| id != local && valid_presence(p))
        .take(MAX_PEERS + 1)
        .map(|(id, presence)| {
            (
                id.clone(),
                Participant {
                    id,
                    presence,
                    seen: Instant::now(),
                },
            )
        })
        .collect();
}
#[cfg(test)]
mod wire_tests {
    use super::*;
    #[test]
    fn viewer_cannot_forge_edits_and_invalid_expired_or_revoked_tokens_release_no_state() {
        let doc = Document::new("Private test");
        let mut room = Room::new(Replica::new(&doc).unwrap(), true, "Host".into());
        let capability = [7; 32];
        room.grants.push(Grant {
            capability,
            access: Access::View,
            expires: Instant::now() + Duration::from_secs(10),
        });
        let original = room.replica.save();
        let mut edit = Replica::load(&original).unwrap();
        let mut changed = doc.clone();
        changed.title = "Forged".into();
        edit.commit(&changed).unwrap();
        let request = |capability, update| Request {
            version: 3,
            capability,
            vector: vec![0],
            update,
            presence: Presence {
                name: "Viewer".into(),
                ..Default::default()
            },
        };
        assert!(matches!(
            process(
                &mut room,
                "peer".into(),
                request(capability, edit.save()),
                "host"
            ),
            Reply::Denied
        ));
        assert_eq!(room.replica.project(&doc).unwrap().title, "Private test");
        assert!(matches!(
            process(
                &mut room,
                "peer".into(),
                request([8; 32], vec![0, 0]),
                "host"
            ),
            Reply::Denied
        ));
        assert!(matches!(
            process(
                &mut room,
                "peer".into(),
                request(capability, vec![0, 0]),
                "host"
            ),
            Reply::Sync {
                access: Access::View,
                ..
            }
        ));
        room.grants[0].expires = Instant::now();
        assert!(matches!(
            process(
                &mut room,
                "peer".into(),
                request(capability, vec![0, 0]),
                "host"
            ),
            Reply::Denied
        ));
        room.grants.clear();
        assert!(matches!(
            process(
                &mut room,
                "peer".into(),
                request(capability, vec![0, 0]),
                "host"
            ),
            Reply::Denied
        ));
    }
}
