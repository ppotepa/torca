//! RuntimeHost adapter for the completed pairing coordinator/runtime.

use torca_client_engine::EngineHandle;
use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::{OpaqueId, Timestamp};
use torca_pairing::{PairingCode, PairingSessionId, PairingState};
use torca_pairing_coordinator::{LocalPairingContext, PairingApprovalPort, PairingCryptoPort, PairingPeerSecretStore, PairingRendezvousPort, PairingRuntime};
use torca_runtime_host::{PairingDriver, PairingInvitationView, RuntimeDriverError};
use torca_storage_sqlite::SqlCipherRelationshipAdmin;
use torca_tor_driver::SharedTorEndpoint;

pub struct RuntimePairingDriver<R,C,A,S>{runtime:PairingRuntime<R,C,A,S>,engine:EngineHandle,tor_endpoint:SharedTorEndpoint,random:RustCryptoProvider,contact_metadata:Option<SqlCipherRelationshipAdmin>}
impl<R,C,A,S> RuntimePairingDriver<R,C,A,S>
where R:PairingRendezvousPort,C:PairingCryptoPort,A:PairingApprovalPort,S:PairingPeerSecretStore {
 pub const fn new(runtime:PairingRuntime<R,C,A,S>,engine:EngineHandle,tor_endpoint:SharedTorEndpoint)->Self{Self{runtime,engine,tor_endpoint,random:RustCryptoProvider,contact_metadata:None}}
 pub fn with_contact_metadata(mut self,metadata:SqlCipherRelationshipAdmin)->Self{self.contact_metadata=Some(metadata);self}
 fn context(&mut self)->Result<LocalPairingContext,RuntimeDriverError>{let identity=self.engine.snapshot().map_err(|_|RuntimeDriverError::Pairing)?.identity.ok_or(RuntimeDriverError::Pairing)?;let onion_address=self.tor_endpoint.get().ok_or(RuntimeDriverError::Pairing)?;Ok(LocalPairingContext{display_name:identity.profile().display_name().as_str().to_owned(),public_identity:identity.public().clone(),onion_address,capability_id:self.random_id()?})}
 fn random_id(&mut self)->Result<OpaqueId,RuntimeDriverError>{for _ in 0..8{let mut bytes=[0_u8;16];self.random.fill_random(&mut bytes).map_err(|_|RuntimeDriverError::Pairing)?;let id=OpaqueId::from_bytes(bytes);if!id.is_nil(){return Ok(id)}}Err(RuntimeDriverError::Pairing)}
 fn active_sessions(&self)->Result<Vec<PairingSessionId>,RuntimeDriverError>{self.engine.snapshot().map_err(|_|RuntimeDriverError::Pairing).map(|snapshot|snapshot.pairings.into_iter().filter(|s|!matches!(s.state(),PairingState::Rejected|PairingState::Cancelled|PairingState::Expired|PairingState::Completed)).map(|s|s.id()).collect())}
}
impl<R,C,A,S> PairingDriver for RuntimePairingDriver<R,C,A,S>
where R:PairingRendezvousPort+Send+'static,C:PairingCryptoPort+Send+'static,A:PairingApprovalPort+Send+'static,S:PairingPeerSecretStore+Send+'static {
 fn create(&mut self,session_id:PairingSessionId,now:Timestamp)->Result<PairingInvitationView,RuntimeDriverError>{let context=self.context()?;let invitation=self.runtime.create_invitation(session_id,context,now).map_err(|_|RuntimeDriverError::Pairing)?;Ok(PairingInvitationView{session_id:invitation.session_id,code:invitation.code,uri:invitation.uri,expires_at:invitation.expires_at})}
 fn join(&mut self,session_id:PairingSessionId,code:PairingCode,now:Timestamp)->Result<(),RuntimeDriverError>{let context=self.context()?;self.runtime.join_invitation(session_id,code,context,now).map_err(|_|RuntimeDriverError::Pairing)}
 fn approve(&mut self,session_id:PairingSessionId,now:Timestamp)->Result<(),RuntimeDriverError>{self.runtime.approve(session_id,now).map_err(|_|RuntimeDriverError::Pairing)}
 fn reject(&mut self,session_id:PairingSessionId)->Result<(),RuntimeDriverError>{self.runtime.reject(session_id).map_err(|_|RuntimeDriverError::Pairing)}
 fn cancel(&mut self,session_id:PairingSessionId)->Result<(),RuntimeDriverError>{self.runtime.cancel(session_id).map_err(|_|RuntimeDriverError::Pairing)}
 fn maintenance(&mut self,now:Timestamp)->Result<(),RuntimeDriverError>{self.runtime.maintenance(now).map_err(|_|RuntimeDriverError::Pairing)?;for session_id in self.active_sessions()?{match self.runtime.poll(session_id,now){Ok(report)=>{if let(Some(completed),Some(metadata))=(report.completed_contact,self.contact_metadata.as_mut()){let _=metadata.rename_contact(completed.contact_id,completed.display_name,now);}},Err(torca_pairing_coordinator::PairingRuntimeError::SessionNotFound)=>{},Err(_)=>return Err(RuntimeDriverError::Pairing)}}Ok(())}
 fn shutdown(&mut self){if let Ok(sessions)=self.active_sessions(){for id in sessions{let _=self.runtime.close_transport(id);}}}
}
