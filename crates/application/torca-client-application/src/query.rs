use std::collections::BTreeMap;

use torca_contacts::ContactId;
use torca_conversations::ConversationId;
use torca_foundation::Timestamp;
use torca_messaging::{Message, MessageId, MessageReaction};
use torca_runtime_policy::{BatteryPreferences, ContactAvailabilityMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationQueryError {
    Unavailable,
}
impl core::fmt::Display for ApplicationQueryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("application query unavailable")
    }
}
impl std::error::Error for ApplicationQueryError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationMessagePage {
    pub messages: Vec<Message>,
    pub reactions: Vec<MessageReaction>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationMessageSummary {
    pub conversation_id: ConversationId,
    pub unread_count: u32,
    pub last_activity_at: Timestamp,
    pub last_message: Option<Message>,
}

pub trait ConversationHistoryPort {
    fn page_for_conversation(
        &self,
        conversation_id: ConversationId,
        before: Option<(Timestamp, MessageId)>,
        limit: usize,
    ) -> Result<ConversationMessagePage, ApplicationQueryError>;
    fn search_conversation(
        &self,
        conversation_id: ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Message>, ApplicationQueryError>;
    fn conversation_summaries(
        &self,
    ) -> Result<BTreeMap<ConversationId, ConversationMessageSummary>, ApplicationQueryError>;
}

pub trait SecurityProjectionPort {
    fn requires_reverification(
        &self,
        conversation_id: ConversationId,
    ) -> Result<bool, ApplicationQueryError>;
    fn contact_states(
        &self,
    ) -> Result<BTreeMap<ContactId, ContactSecuritySnapshot>, ApplicationQueryError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSecurityState {
    Unverified,
    Verified,
    IdentityChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactSecuritySnapshot {
    pub state: ContactSecurityState,
    pub verified_at: Option<Timestamp>,
}

pub trait RuntimeSettingsPort {
    fn notifications_enabled(&self) -> Result<bool, ApplicationQueryError>;
    fn set_notifications_enabled(
        &self,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), ApplicationQueryError>;
    fn read_receipts_enabled(&self) -> Result<bool, ApplicationQueryError>;
    fn set_read_receipts_enabled(
        &self,
        enabled: bool,
        updated_at_ms: i64,
    ) -> Result<(), ApplicationQueryError>;
    fn battery_preferences(&self) -> Result<BatteryPreferences, ApplicationQueryError>;
    fn set_battery_preferences(
        &self,
        preferences: BatteryPreferences,
        updated_at_ms: i64,
    ) -> Result<(), ApplicationQueryError>;
    fn contact_availability(
        &self,
        contact_id: torca_contacts::ContactId,
    ) -> Result<ContactAvailabilityMode, ApplicationQueryError>;
    fn set_contact_availability(
        &self,
        contact_id: torca_contacts::ContactId,
        mode: ContactAvailabilityMode,
        updated_at_ms: i64,
    ) -> Result<(), ApplicationQueryError>;
    fn new_contacts_acknowledged_at_ms(&self) -> Result<Option<i64>, ApplicationQueryError>;
    fn acknowledge_new_contacts(&self, updated_at_ms: i64) -> Result<(), ApplicationQueryError>;
}

pub struct ApplicationReadModels {
    pub history: Box<dyn ConversationHistoryPort>,
    pub security: Box<dyn SecurityProjectionPort>,
    pub settings: Box<dyn RuntimeSettingsPort>,
}
