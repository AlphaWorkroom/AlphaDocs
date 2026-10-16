//! Native direct-peer collaboration. `live` implements the v2 CRDT session/UI backend.
//! The retained v1 snapshot/proposal experiment below is not used by the UI.
use crate::model::Document;
pub mod live;
pub mod nearby;
pub mod replica;
#[cfg(test)]
mod tests;
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::{Builder, PortmapperConfig},
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
const ALPN: &[u8] = b"alphadocs/direct-preview/1";
const PROTOCOL: u32 = 1;
const MAX_FRAME: usize = 16 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(8);
#[derive(Clone)]
pub struct Invitation {
    pub host: EndpointId,
    pub address: SocketAddr,
    capability: [u8; 32],
}
impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitation")
            .field("host", &self.host)
            .field("address", &self.address)
            .field("capability", &"[redacted]")
            .finish()
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Access {
    View,
    Edit,
}
struct Grant {
    capability: [u8; 32],
    access: Access,
    expires: Instant,
}
struct State {
    revision: u64,
    document: Document,
}
pub struct Host {
    endpoint: Endpoint,
    address: SocketAddr,
    state: Mutex<State>,
    grants: Mutex<Vec<Grant>>,
}
#[derive(Serialize, Deserialize)]
pub enum Request {
    Snapshot,
    Propose { base: u64, document: Box<Document> },
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Snapshot {
        revision: u64,
        document: Box<Document>,
    },
    Conflict {
        revision: u64,
        document: Box<Document>,
    },
    Denied,
    Invalid(String),
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    protocol: u32,
    capability: [u8; 32],
    request: Request,
}
async fn endpoint(ip: IpAddr) -> Result<Endpoint, String> {
    endpoint_for(ip, vec![ALPN.to_vec(), b"alphadocs/live/2".to_vec()]).await
}
async fn endpoint_for(ip: IpAddr, alpns: Vec<Vec<u8>>) -> Result<Endpoint, String> {
    Builder::empty()
        .preset(iroh::endpoint::presets::Minimal)
        .clear_ip_transports()
        .portmapper_config(PortmapperConfig::Disabled)
        .net_report_config(iroh::NetReportConfig::minimal())
        .alpns(alpns)
        .bind_addr(SocketAddr::new(ip, 0))
        .map_err(|e| e.to_string())?
        .bind()
        .await
        .map_err(|e| e.to_string())
}
impl Host {
    pub async fn bind(document: Document, ip: IpAddr) -> Result<Self, String> {
        document.validate()?;
        let ep = endpoint(ip).await?;
        let address = ep
            .bound_sockets()
            .into_iter()
            .find(|a| a.ip() == ip)
            .ok_or("Requested interface was not bound")?;
        Ok(Self {
            endpoint: ep,
            address,
            state: Mutex::new(State {
                revision: 0,
                document,
            }),
            grants: Mutex::new(vec![]),
        })
    }
    pub fn invite(&self, access: Access, lifetime: Duration) -> Result<Invitation, String> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(3600) {
            return Err("Invitations expire within one hour".into());
        }
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Session grants unavailable")?;
        grants.retain(|g| g.expires > Instant::now());
        if grants.len() >= 64 {
            return Err("Too many invitations".into());
        }
        let capability = rand::random();
        let expires = Instant::now()
            .checked_add(lifetime)
            .ok_or("Invalid invitation lifetime")?;
        grants.push(Grant {
            capability,
            access,
            expires,
        });
        Ok(Invitation {
            host: self.endpoint.id(),
            address: self.address,
            capability,
        })
    }
    pub fn revoke(&self, invite: &Invitation) -> Result<(), String> {
        self.grants
            .lock()
            .map_err(|_| "Session grants unavailable")?
            .retain(|g| !bool::from(g.capability.ct_eq(&invite.capability)));
        Ok(())
    }
    fn handle(&self, envelope: Envelope) -> Response {
        if envelope.protocol != PROTOCOL {
            return Response::Invalid("Incompatible peer protocol".into());
        }
        let access = self.grants.lock().ok().and_then(|gs| {
            gs.iter()
                .find(|g| {
                    bool::from(g.capability.ct_eq(&envelope.capability))
                        && g.expires > Instant::now()
                })
                .map(|g| g.access)
        });
        let Some(access) = access else {
            return Response::Denied;
        };
        let Ok(mut state) = self.state.lock() else {
            return Response::Invalid("Session document unavailable".into());
        };
        if let Request::Propose { base, document } = envelope.request {
            if access != Access::Edit {
                return Response::Denied;
            }
            if base != state.revision {
                return Response::Conflict {
                    revision: state.revision,
                    document: Box::new(state.document.clone()),
                };
            }
            if document.id != state.document.id {
                return Response::Invalid("Proposal is for another document".into());
            }
            if let Err(error) = document.validate() {
                return Response::Invalid(error);
            }
            let Some(next) = state.revision.checked_add(1) else {
                return Response::Invalid("Session revision exhausted".into());
            };
            state.document = *document;
            state.revision = next;
        }
        Response::Snapshot {
            revision: state.revision,
            document: Box::new(state.document.clone()),
        }
    }
    /// One bounded request. A future runtime must add connection quotas, peer
    /// review, durable recovery and a merge engine before exposing this in UI.
    pub async fn serve_once(&self) -> Result<(), String> {
        tokio::time::timeout(DEADLINE, async {
            let incoming = self.endpoint.accept().await.ok_or("Session closed")?;
            let connection = incoming.await.map_err(|e| e.to_string())?;
            let (mut send, mut recv) = connection.accept_bi().await.map_err(|e| e.to_string())?;
            let bytes = recv
                .read_to_end(MAX_FRAME)
                .await
                .map_err(|e| e.to_string())?;
            let envelope: Envelope =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid peer message")?;
            let response = self.handle(envelope);
            let bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
            if bytes.len() > MAX_FRAME {
                return Err("Document exceeds preview transfer limit".into());
            }
            send.write_all(&bytes).await.map_err(|e| e.to_string())?;
            send.finish().map_err(|e| e.to_string())?;
            let _ = send.stopped().await;
            Ok(())
        })
        .await
        .map_err(|_| "Peer request timed out".to_owned())?
    }
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}
pub struct Peer {
    endpoint: Endpoint,
}
impl Peer {
    pub async fn bind(ip: IpAddr) -> Result<Self, String> {
        Ok(Self {
            endpoint: endpoint(ip).await?,
        })
    }
    pub async fn exchange(
        &self,
        invite: &Invitation,
        request: Request,
    ) -> Result<Response, String> {
        let bytes = serde_json::to_vec(&Envelope {
            protocol: PROTOCOL,
            capability: invite.capability,
            request,
        })
        .map_err(|_| "Could not encode proposal")?;
        if bytes.len() > MAX_FRAME {
            return Err("Proposal exceeds preview transfer limit".into());
        }
        tokio::time::timeout(DEADLINE, async {
            let address = EndpointAddr::new(invite.host).with_ip_addr(invite.address);
            let connection = self
                .endpoint
                .connect(address, ALPN)
                .await
                .map_err(|e| e.to_string())?;
            let (mut send, mut recv) = connection.open_bi().await.map_err(|e| e.to_string())?;
            send.write_all(&bytes).await.map_err(|e| e.to_string())?;
            send.finish().map_err(|e| e.to_string())?;
            let bytes = recv
                .read_to_end(MAX_FRAME)
                .await
                .map_err(|e| e.to_string())?;
            let response = serde_json::from_slice(&bytes).map_err(|_| "Invalid host response")?;
            connection.close(0u8.into(), b"request complete");
            Ok(response)
        })
        .await
        .map_err(|_| "Host connection timed out".to_owned())?
    }
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}
#[cfg(test)]
mod protocol_tests {
    use super::*;
    async fn exchange(host: &Host, peer: &Peer, invite: &Invitation, request: Request) -> Response {
        let (served, response) = tokio::join!(host.serve_once(), peer.exchange(invite, request));
        served.unwrap();
        response.unwrap()
    }
    #[tokio::test]
    async fn encrypted_loopback_checks_roles_conflicts_revocation_and_preserves_rejected_draft() {
        let ip = "127.0.0.1".parse().unwrap();
        let document = Document::new("Private QA document");
        let host = Host::bind(document.clone(), ip).await.unwrap();
        let peer = Peer::bind(ip).await.unwrap();
        let editor = host.invite(Access::Edit, Duration::from_secs(60)).unwrap();
        let viewer = host.invite(Access::View, Duration::from_secs(60)).unwrap();
        assert!(!format!("{editor:?}").contains(&format!("{:?}", editor.capability)));
        assert!(matches!(
            exchange(&host, &peer, &viewer, Request::Snapshot).await,
            Response::Snapshot { revision: 0, .. }
        ));
        let mut changed = document.clone();
        changed.title = "Edited by a peer".into();
        assert!(matches!(
            exchange(
                &host,
                &peer,
                &viewer,
                Request::Propose {
                    base: 0,
                    document: Box::new(changed.clone())
                }
            )
            .await,
            Response::Denied
        ));
        assert!(matches!(
            exchange(
                &host,
                &peer,
                &editor,
                Request::Propose {
                    base: 0,
                    document: Box::new(changed)
                }
            )
            .await,
            Response::Snapshot { revision: 1, .. }
        ));
        let mut stale = document.clone();
        stale.title = "Retained rejected draft".into();
        let response = exchange(
            &host,
            &peer,
            &editor,
            Request::Propose {
                base: 0,
                document: Box::new(stale.clone()),
            },
        )
        .await;
        assert!(
            matches!(response,Response::Conflict{revision:1,document} if document.title=="Edited by a peer")
        );
        assert_eq!(stale.title, "Retained rejected draft");
        let mut wrong = viewer.clone();
        wrong.capability = rand::random();
        assert!(matches!(
            exchange(&host, &peer, &wrong, Request::Snapshot).await,
            Response::Denied
        ));
        host.revoke(&viewer).unwrap();
        assert!(matches!(
            exchange(&host, &peer, &viewer, Request::Snapshot).await,
            Response::Denied
        ));
        host.grants
            .lock()
            .unwrap()
            .iter_mut()
            .for_each(|g| g.expires = Instant::now());
        assert!(matches!(
            exchange(&host, &peer, &editor, Request::Snapshot).await,
            Response::Denied
        ));
        peer.close().await;
        host.close().await;
    }
    #[tokio::test]
    async fn a_wrong_host_key_cannot_read_the_document() {
        let ip = "127.0.0.1".parse().unwrap();
        let host = Host::bind(Document::new("Secret QA"), ip).await.unwrap();
        let peer = Peer::bind(ip).await.unwrap();
        let mut invite = host.invite(Access::View, Duration::from_secs(60)).unwrap();
        invite.host = peer.endpoint.id();
        let (_, response) =
            tokio::join!(host.serve_once(), peer.exchange(&invite, Request::Snapshot));
        assert!(response.is_err());
        peer.close().await;
        host.close().await;
    }
}
