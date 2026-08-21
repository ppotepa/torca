use torca_client_application::{
    ApplicationQueryError, ApplicationReadModels, ContactSecuritySnapshot, ContactSecurityState,
    ConversationHistoryPort, ConversationMessagePage, ConversationMessageSummary,
    RuntimeSettingsPort, SecurityProjectionPort,
};
use torca_conversations::ConversationId;
use torca_foundation::Timestamp;
use torca_messaging::{Message, MessageId};
use torca_storage_sqlite::{
    SqlCipherMessageStore, SqlCipherSecurityProjection, SqlCipherSettingsStore,
};

pub(crate) fn build_read_models(
    history: SqlCipherMessageStore,
    security: SqlCipherSecurityProjection,
    settings: SqlCipherSettingsStore,
) -> ApplicationReadModels {
    ApplicationReadModels {
        history: Box::new(SqliteHistory(history)),
        security: Box::new(SqliteSecurity(security)),
        settings: Box::new(SqliteSettings(settings)),
    }
}

struct SqliteHistory(SqlCipherMessageStore);

impl ConversationHistoryPort for SqliteHistory {
    fn page_for_conversation(
        &self,
        id: ConversationId,
        before: Option<(Timestamp, MessageId)>,
        limit: usize,
    ) -> Result<ConversationMessagePage, ApplicationQueryError> {
        self.0
            .page_for_conversation(id, before, limit)
            .map(|page| ConversationMessagePage {
                messages: page.messages,
                has_more: page.has_more,
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn search_conversation(
        &self,
        id: ConversationId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Message>, ApplicationQueryError> {
        self.0.search_conversation(id, query, limit).map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn conversation_summaries(
        &self,
    ) -> Result<
        std::collections::BTreeMap<ConversationId, ConversationMessageSummary>,
        ApplicationQueryError,
    > {
        self.0
            .conversation_summaries()
            .map(|items| {
                items
                    .into_iter()
                    .map(|(id, item)| {
                        (
                            id,
                            ConversationMessageSummary {
                                conversation_id: item.conversation_id,
                                unread_count: item.unread_count,
                                last_activity_at: item.last_activity_at,
                                last_message: item.last_message,
                            },
                        )
                    })
                    .collect()
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
}

struct SqliteSecurity(SqlCipherSecurityProjection);

impl SecurityProjectionPort for SqliteSecurity {
    fn requires_reverification(&self, id: ConversationId) -> Result<bool, ApplicationQueryError> {
        self.0.requires_reverification(id).map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn contact_states(
        &self,
    ) -> Result<
        std::collections::BTreeMap<torca_contacts::ContactId, ContactSecuritySnapshot>,
        ApplicationQueryError,
    > {
        self.0
            .contact_states()
            .map(|states| {
                states
                    .into_iter()
                    .map(|(id, snapshot)| {
                        let state = match snapshot.state {
                            torca_storage_sqlite::ContactSecurityState::Unverified => {
                                ContactSecurityState::Unverified
                            }
                            torca_storage_sqlite::ContactSecurityState::Verified => {
                                ContactSecurityState::Verified
                            }
                            torca_storage_sqlite::ContactSecurityState::IdentityChanged => {
                                ContactSecurityState::IdentityChanged
                            }
                        };
                        (id, ContactSecuritySnapshot { state, verified_at: snapshot.verified_at })
                    })
                    .collect()
            })
            .map_err(|_| ApplicationQueryError::Unavailable)
    }
}

struct SqliteSettings(SqlCipherSettingsStore);

impl RuntimeSettingsPort for SqliteSettings {
    fn notifications_enabled(&self) -> Result<bool, ApplicationQueryError> {
        self.0.notifications_enabled().map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn set_notifications_enabled(
        &self,
        enabled: bool,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_notifications_enabled(enabled, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn read_receipts_enabled(&self) -> Result<bool, ApplicationQueryError> {
        self.0.read_receipts_enabled().map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn set_read_receipts_enabled(
        &self,
        enabled: bool,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_read_receipts_enabled(enabled, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn battery_preferences(
        &self,
    ) -> Result<torca_battery::BatteryPreferences, ApplicationQueryError> {
        self.0.battery_preferences().map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn set_battery_preferences(
        &self,
        preferences: torca_battery::BatteryPreferences,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_battery_preferences(preferences, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn contact_availability(
        &self,
        contact_id: torca_contacts::ContactId,
    ) -> Result<torca_runtime_policy::ContactAvailabilityMode, ApplicationQueryError> {
        self.0.contact_availability(contact_id).map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn set_contact_availability(
        &self,
        contact_id: torca_contacts::ContactId,
        mode: torca_runtime_policy::ContactAvailabilityMode,
        at: i64,
    ) -> Result<(), ApplicationQueryError> {
        self.0
            .set_contact_availability(contact_id, mode, at)
            .map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn new_contacts_acknowledged_at_ms(&self) -> Result<Option<i64>, ApplicationQueryError> {
        self.0.new_contacts_acknowledged_at_ms().map_err(|_| ApplicationQueryError::Unavailable)
    }

    fn acknowledge_new_contacts(&self, at: i64) -> Result<(), ApplicationQueryError> {
        self.0.acknowledge_new_contacts(at).map_err(|_| ApplicationQueryError::Unavailable)
    }
}
