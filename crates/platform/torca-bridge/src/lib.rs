//! Stable typed boundary between Flutter and the process-owned Rust runtime.

use std::collections::BTreeMap;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use torca_attachments::AttachmentId;
use torca_client_engine::{ClientSnapshot, EngineCommand, EngineError, EngineHandle, EngineResult};
use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_identity::{IdentityId, Profile, ProfileName, PublicIdentity};
use torca_messaging::{MessageBody, MessageId, ReplyReference};
use torca_pairing::{PairingCode, PairingSessionId};
use torca_runtime_host::{
    AttachmentSendRequest, AttachmentView, HostTorState, NetworkSnapshot, RuntimeHostHandle,
};

pub const CONTRACT_VERSION: u16 = 10;

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCommand {
    CreateIdentity { identity_id_hex: String, display_name: String, at_ms: i64 },
    CreatePairing { session_id_hex: String }, JoinPairing { session_id_hex: String, code: String }, ApprovePairing { session_id_hex: String }, RejectPairing { session_id_hex: String }, CancelPairing { session_id_hex: String },
    RenameContact { contact_id_hex: String, display_name: String }, BlockContact { contact_id_hex: String }, UnblockContact { contact_id_hex: String }, RemoveContact { contact_id_hex: String }, ClearConversationHistory { conversation_id_hex: String },
    QueueMessage { message_id_hex: String, conversation_id_hex: String, body: String, reply_to_message_id_hex: Option<String>, at_ms: i64 }, RetryMessage { message_id_hex: String, at_ms: i64 }, MarkConversationRead { conversation_id_hex: String },
    QueueAttachment { attachment_id_hex: String, message_id_hex: String, conversation_id_hex: String, source_path: String, name: String, media_type: String, size: u64 }, RetryAttachment { attachment_id_hex: String }, CancelAttachment { attachment_id_hex: String }, ExportAttachment { attachment_id_hex: String, destination_path: String },
    RefreshSnapshot,
}
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeResult { pub ok: bool, pub kind: String, pub error: Option<String> }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeSnapshot { pub contract_version: u16, pub identity_name: Option<String>, pub tor_state: String, pub onion_address: Option<String>, pub pairings: Vec<BridgePairing>, pub contacts: Vec<BridgeContact>, pub conversations: Vec<BridgeConversation>, pub messages: Vec<BridgeMessage>, pub attachments: Vec<BridgeAttachment> }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgePairing { pub id: String, pub code: String, pub role: String, pub state: String, pub expires_at_ms: i64, pub local_approved: bool, pub remote_approved: bool }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgePeerHealth { pub state: String, pub quality: String, pub rtt_ms: Option<u64>, pub last_success_at_ms: Option<i64>, pub consecutive_failures: u32, pub reconnect_attempt: u32 }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeContact { pub id: String, pub display_name: String, pub onion_address: String, pub status: String, pub connection_state: String, pub safety_number: String, pub peer_health: BridgePeerHealth }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeConversation { pub id: String, pub contact_id: String, pub status: String }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeMessage { pub id: String, pub conversation_id: String, pub body: String, pub direction: String, pub status: String, pub reply_to_message_id: Option<String>, pub created_at_ms: i64, pub updated_at_ms: i64, pub attempt_count: u32 }
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)] pub struct BridgeAttachment { pub id: String, pub message_id: String, pub name: String, pub media_type: String, pub size: u64, pub status: String, pub offset: u64 }

pub struct EngineBridge { engine: EngineHandle, runtime: Option<RuntimeHostHandle> }
impl EngineBridge {
    pub const fn new(engine: EngineHandle) -> Self { Self { engine, runtime: None } }
    pub fn attach_runtime(&mut self, runtime: RuntimeHostHandle) { self.runtime = Some(runtime); }
    pub const fn has_runtime(&self) -> bool { self.runtime.is_some() }
    pub fn execute(&self, command: BridgeCommand) -> BridgeResult {
        let result: Result<&'static str, String> = match command {
            BridgeCommand::CreateIdentity { identity_id_hex, display_name, at_ms } => parse_id(&identity_id_hex).and_then(|id| ProfileName::new(display_name).map_err(string_error).map(|n| (id,n))).and_then(|(id,name)| timestamp(at_ms).map(|at| EngineCommand::CreateIdentity { identity_id: IdentityId::from_opaque(id), profile: Profile::new(name,None), at })).and_then(|c| self.engine.dispatch(c).map_err(string_error)).map(|v| result_kind(&v)),
            BridgeCommand::CreatePairing { session_id_hex } => parse_pairing_id(&session_id_hex).and_then(|id| self.runtime()?.create_pairing(id).map_err(string_error)).map(|_| "pairing_started"),
            BridgeCommand::JoinPairing { session_id_hex, code } => parse_pairing_id(&session_id_hex).and_then(|id| PairingCode::new(code).map_err(string_error).map(|c|(id,c))).and_then(|(id,c)| self.runtime()?.join_pairing(id,c).map_err(string_error)).map(|_| "pairing_joined"),
            BridgeCommand::ApprovePairing { session_id_hex } => parse_pairing_id(&session_id_hex).and_then(|id| self.runtime()?.approve_pairing(id).map_err(string_error)).map(|_| "pairing_updated"),
            BridgeCommand::RejectPairing { session_id_hex } => parse_pairing_id(&session_id_hex).and_then(|id| self.runtime()?.reject_pairing(id).map_err(string_error)).map(|_| "pairing_rejected"),
            BridgeCommand::CancelPairing { session_id_hex } => parse_pairing_id(&session_id_hex).and_then(|id| self.runtime()?.cancel_pairing(id).map_err(string_error)).map(|_| "pairing_cancelled"),
            BridgeCommand::RenameContact { contact_id_hex, display_name } => parse_contact_id(&contact_id_hex).and_then(|id| self.runtime()?.rename_contact(id,display_name).map_err(string_error)).map(|_| "contact_renamed"),
            BridgeCommand::BlockContact { contact_id_hex } => parse_contact_id(&contact_id_hex).and_then(|id| self.runtime()?.block_contact(id).map_err(string_error)).map(|_| "contact_blocked"),
            BridgeCommand::UnblockContact { contact_id_hex } => parse_contact_id(&contact_id_hex).and_then(|id| self.runtime()?.unblock_contact(id).map_err(string_error)).map(|_| "contact_unblocked"),
            BridgeCommand::RemoveContact { contact_id_hex } => parse_contact_id(&contact_id_hex).and_then(|id| self.runtime()?.remove_contact(id).map_err(string_error)).map(|_| "contact_removed"),
            BridgeCommand::ClearConversationHistory { conversation_id_hex } => parse_conversation_id(&conversation_id_hex).and_then(|id| self.runtime()?.clear_conversation_history(id).map_err(string_error)).map(|_| "conversation_history_cleared"),
            BridgeCommand::QueueMessage { message_id_hex, conversation_id_hex, body, reply_to_message_id_hex, at_ms } => self.runtime().and_then(|runtime| {
                let message_id = parse_id(&message_id_hex)?; let conversation_id = parse_id(&conversation_id_hex)?; let body = MessageBody::new(body).map_err(string_error)?;
                let reply_to = match reply_to_message_id_hex { Some(v) => Some(ReplyReference { message_id: MessageId::from_opaque(parse_id(&v)?) }), None => None };
                let at = timestamp(at_ms)?;
                self.engine.dispatch(EngineCommand::QueueMessage { message_id: MessageId::from_opaque(message_id), conversation_id: ConversationId::from_opaque(conversation_id), body, reply_to, at }).map_err(string_error).map(|v| { runtime.wake_delivery(); result_kind(&v) })
            }),
            BridgeCommand::RetryMessage { message_id_hex, at_ms } => self.runtime().and_then(|runtime| { let id=MessageId::from_opaque(parse_id(&message_id_hex)?); let at=timestamp(at_ms)?; self.engine.dispatch(EngineCommand::RetryMessage { message_id:id, at }).map_err(string_error).map(|v| { runtime.wake_delivery(); result_kind(&v) }) }),
            BridgeCommand::MarkConversationRead { conversation_id_hex } => parse_id(&conversation_id_hex).and_then(|id| self.runtime()?.mark_conversation_read(id).map_err(string_error)).map(|_| "conversation_read"),
            BridgeCommand::QueueAttachment { attachment_id_hex, message_id_hex, conversation_id_hex, source_path, name, media_type, size } =>
                parse_attachment_request(attachment_id_hex, message_id_hex, conversation_id_hex, source_path, name, media_type, size)
                    .and_then(|request| self.runtime()?.queue_attachment(request).map_err(string_error)).map(|_| "attachment_queued"),
            BridgeCommand::RetryAttachment { attachment_id_hex } => parse_id(&attachment_id_hex).and_then(|id| self.runtime()?.retry_attachment(id).map_err(string_error)).map(|_| "attachment_retried"),
            BridgeCommand::CancelAttachment { attachment_id_hex } => parse_id(&attachment_id_hex).and_then(|id| self.runtime()?.cancel_attachment(id).map_err(string_error)).map(|_| "attachment_cancelled"),
            BridgeCommand::ExportAttachment { attachment_id_hex, destination_path } => parse_id(&attachment_id_hex).and_then(|id| self.runtime()?.export_attachment(AttachmentId::from_opaque(id), PathBuf::from(destination_path)).map_err(string_error)).map(|_| "attachment_exported"),
            BridgeCommand::RefreshSnapshot => self.snapshot().map(|_| "snapshot").map_err(string_error),
        };
        match result { Ok(kind) => BridgeResult { ok:true, kind:kind.into(), error:None }, Err(error) => BridgeResult { ok:false, kind:"error".into(), error:Some(error) } }
    }
    pub fn snapshot(&self) -> Result<BridgeSnapshot, EngineError> {
        let app=self.engine.snapshot()?;
        let (network,attachments)=match &self.runtime { Some(runtime)=>(runtime.network_snapshot().map_err(|_|EngineError("network snapshot unavailable".into()))?, runtime.attachment_snapshot().map_err(|_|EngineError("attachment snapshot unavailable".into()))?), None=>(NetworkSnapshot { tor:HostTorState::Stopped,onion_address:None,peers:BTreeMap::new(),peer_health:BTreeMap::new(),contact_names:BTreeMap::new() },Vec::new()) };
        Ok(map_snapshot(app,network,attachments))
    }
    pub fn diagnostics_json(&self)->Result<String,EngineError>{match &self.runtime{Some(r)=>r.diagnostics_json().map_err(|_|EngineError("diagnostics unavailable".into())),None=>Ok("{\"events\":[]}".into())}}
    fn runtime(&self)->Result<&RuntimeHostHandle,String>{self.runtime.as_ref().ok_or_else(||"secure network runtime is not ready".into())}
}
fn parse_attachment_request(attachment_id_hex:String,message_id_hex:String,conversation_id_hex:String,source_path:String,name:String,media_type:String,size:u64)->Result<AttachmentSendRequest,String>{Ok(AttachmentSendRequest{attachment_id:parse_id(&attachment_id_hex)?,message_id:parse_id(&message_id_hex)?,conversation_id:parse_id(&conversation_id_hex)?,source_path,name,media_type,size})}
fn parse_pairing_id(v:&str)->Result<PairingSessionId,String>{parse_id(v).map(PairingSessionId::from_opaque)} fn parse_contact_id(v:&str)->Result<ContactId,String>{parse_id(v).map(ContactId::from_opaque)} fn parse_conversation_id(v:&str)->Result<ConversationId,String>{parse_id(v).map(ConversationId::from_opaque)} fn parse_id(v:&str)->Result<OpaqueId,String>{v.parse::<OpaqueId>().map_err(string_error)} fn timestamp(v:i64)->Result<Timestamp,String>{Timestamp::from_unix_millis(v).map_err(string_error)} fn string_error(e:impl core::fmt::Display)->String{e.to_string()}
fn result_kind(v:&EngineResult)->&'static str{match v{EngineResult::IdentityCreated=>"identity_created",EngineResult::PairingStarted=>"pairing_started",EngineResult::PairingJoined=>"pairing_joined",EngineResult::PairingUpdated=>"pairing_updated",EngineResult::PairingRejected=>"pairing_rejected",EngineResult::PairingCancelled=>"pairing_cancelled",EngineResult::PairingCompleted{..}=>"pairing_completed",EngineResult::MessageQueued{..}=>"message_queued",EngineResult::MessageUpdated{..}=>"message_updated",EngineResult::ReceiptApplied{..}=>"receipt_applied"}}
fn map_snapshot(snapshot:ClientSnapshot,network:NetworkSnapshot,attachments:Vec<AttachmentView>)->BridgeSnapshot{
    let local_public=snapshot.identity.as_ref().map(|i|i.public().clone()); let identity_name=snapshot.identity.as_ref().map(|i|i.profile().display_name().as_str().to_owned());
    BridgeSnapshot{contract_version:CONTRACT_VERSION,identity_name,tor_state:format!("{:?}",network.tor).to_lowercase(),onion_address:network.onion_address,
        pairings:snapshot.pairings.into_iter().map(|p|BridgePairing{id:p.id().to_string(),code:p.code().as_str().to_owned(),role:format!("{:?}",p.role()).to_lowercase(),state:format!("{:?}",p.state()).to_lowercase(),expires_at_ms:p.expires_at().to_unix_millis(),local_approved:p.local_approved(),remote_approved:p.remote_approved()}).collect(),
        contacts:snapshot.contacts.into_iter().map(|c|{let connection_state=network.peers.get(&c.id()).map_or_else(||"disconnected".to_owned(),|s|format!("{s:?}").to_lowercase());let peer_health=network.peer_health.get(&c.id()).map_or_else(||BridgePeerHealth{state:connection_state.clone(),quality:"unknown".into(),rtt_ms:None,last_success_at_ms:None,consecutive_failures:0,reconnect_attempt:0},|health|BridgePeerHealth{state:format!("{:?}",health.state).to_lowercase(),quality:format!("{:?}",health.quality).to_lowercase(),rtt_ms:health.rtt_ms,last_success_at_ms:health.last_success_at.map(|at|at.to_unix_millis()),consecutive_failures:health.consecutive_failures,reconnect_attempt:health.reconnect_attempt});let connection_state=peer_health.state.clone();let safety_number=local_public.as_ref().map_or_else(String::new,|l|safety_number(l,c.remote_identity()));let display_name=network.contact_names.get(&c.id()).cloned().unwrap_or_else(||fallback_contact_name(c.id()));BridgeContact{id:c.id().to_string(),display_name,onion_address:c.route().onion_address().to_owned(),status:format!("{:?}",c.status()).to_lowercase(),connection_state,safety_number,peer_health}}).collect(),
        conversations:snapshot.conversations.into_iter().map(|c|BridgeConversation{id:c.id().to_string(),contact_id:c.contact_id().to_string(),status:format!("{:?}",c.status()).to_lowercase()}).collect(),
        messages:snapshot.messages.into_iter().map(|m|BridgeMessage{id:m.id().to_string(),conversation_id:m.conversation_id().to_string(),body:m.body().as_str().to_owned(),direction:format!("{:?}",m.direction()).to_lowercase(),status:format!("{:?}",m.status()).to_lowercase(),reply_to_message_id:m.reply_to().map(|r|r.message_id.to_string()),created_at_ms:m.created_at().to_unix_millis(),updated_at_ms:m.updated_at().to_unix_millis(),attempt_count:u32::try_from(m.attempts().len()).unwrap_or(u32::MAX)}).collect(),
        attachments:attachments.into_iter().map(|a|BridgeAttachment{id:a.id.to_string(),message_id:a.message_id.to_string(),name:a.name,media_type:a.media_type,size:a.size,status:a.status,offset:a.offset}).collect()}
}
fn fallback_contact_name(id:ContactId)->String{let value=id.to_string();let short=value.get(..8).unwrap_or(&value);format!("Contact {short}")}
fn safety_number(local:&PublicIdentity,remote:&PublicIdentity)->String{let(first,second)=if local.identity_id().to_opaque()<=remote.identity_id().to_opaque(){(local,remote)}else{(remote,local)};let mut hash=Sha256::new();hash.update(b"TORCA-SAFETY-NUMBER-V1");update_identity_hash(&mut hash,first);update_identity_hash(&mut hash,second);hash.finalize().chunks(4).map(|c|c.iter().map(|b|format!("{b:02X}")).collect::<String>()).collect::<Vec<_>>().join(" ")}
fn update_identity_hash(hash:&mut Sha256,identity:&PublicIdentity){hash.update(identity.identity_id().to_opaque().as_bytes());let key=identity.key().public_key();hash.update(u32::try_from(key.len()).unwrap_or(u32::MAX).to_be_bytes());hash.update(key)}
pub fn dart_contract_source()->&'static str{include_str!("../schema/torca_contract.dart")}
