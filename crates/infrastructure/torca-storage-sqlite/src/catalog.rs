/// A named SQL statement embedded at compile time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SqlStatement {
    pub name: &'static str,
    pub sql: &'static str,
}

pub mod identity_sql {
    use super::SqlStatement;
    pub const INSERT: SqlStatement = SqlStatement {
        name: "identity.insert",
        sql: include_str!("../sql/commands/identity_insert.sql"),
    };
    pub const UPDATE: SqlStatement = SqlStatement {
        name: "identity.update",
        sql: include_str!("../sql/commands/identity_update.sql"),
    };
    pub const SELECT: SqlStatement = SqlStatement {
        name: "identity.select",
        sql: include_str!("../sql/queries/identity_select.sql"),
    };
}

pub mod contact_sql {
    use super::SqlStatement;
    pub const INSERT: SqlStatement = SqlStatement {
        name: "contact.insert",
        sql: include_str!("../sql/commands/contact_insert.sql"),
    };
    pub const UPDATE: SqlStatement = SqlStatement {
        name: "contact.update",
        sql: include_str!("../sql/commands/contact_update.sql"),
    };
    pub const SELECT_BY_ID: SqlStatement = SqlStatement {
        name: "contact.select_by_id",
        sql: include_str!("../sql/queries/contact_select_by_id.sql"),
    };
    pub const LIST: SqlStatement = SqlStatement {
        name: "contact.list",
        sql: include_str!("../sql/queries/contact_list.sql"),
    };
}

pub mod conversation_sql {
    use super::SqlStatement;
    pub const INSERT: SqlStatement = SqlStatement {
        name: "conversation.insert",
        sql: include_str!("../sql/commands/conversation_insert.sql"),
    };
    pub const SELECT_BY_ID: SqlStatement = SqlStatement {
        name: "conversation.select_by_id",
        sql: include_str!("../sql/queries/conversation_select_by_id.sql"),
    };
    pub const SELECT_BY_CONTACT: SqlStatement = SqlStatement {
        name: "conversation.select_by_contact",
        sql: include_str!("../sql/queries/conversation_select_by_contact.sql"),
    };
    pub const LIST: SqlStatement = SqlStatement {
        name: "conversation.list",
        sql: include_str!("../sql/queries/conversation_list.sql"),
    };
}

pub mod messaging_sql {
    use super::SqlStatement;
    pub const INSERT_MESSAGE: SqlStatement = SqlStatement {
        name: "message.insert",
        sql: include_str!("../sql/commands/message_insert_outbound.sql"),
    };
    pub const SELECT_MESSAGE: SqlStatement = SqlStatement {
        name: "message.select",
        sql: include_str!("../sql/queries/message_select_by_id.sql"),
    };
    pub const INSERT_DOMAIN_MESSAGE: SqlStatement = SqlStatement {
        name: "message.domain_insert",
        sql: include_str!("../sql/commands/message_insert_domain.sql"),
    };
    pub const UPDATE_DOMAIN_MESSAGE: SqlStatement = SqlStatement {
        name: "message.domain_update",
        sql: include_str!("../sql/commands/message_update_domain.sql"),
    };
    pub const SELECT_DOMAIN_MESSAGE: SqlStatement = SqlStatement {
        name: "message.domain_select",
        sql: include_str!("../sql/queries/message_domain_select_by_id.sql"),
    };
    pub const SELECT_DOMAIN_FOR_CONVERSATION: SqlStatement = SqlStatement {
        name: "message.domain_for_conversation",
        sql: include_str!("../sql/queries/message_domain_for_conversation.sql"),
    };
    pub const LIST_DOMAIN_MESSAGES: SqlStatement = SqlStatement {
        name: "message.domain_list",
        sql: include_str!("../sql/queries/message_domain_list.sql"),
    };
    pub const INSERT_OUTBOX: SqlStatement = SqlStatement {
        name: "outbox.insert",
        sql: include_str!("../sql/commands/outbox_insert.sql"),
    };
    pub const CLAIM_DUE: SqlStatement = SqlStatement {
        name: "outbox.claim_due",
        sql: include_str!("../sql/queries/outbox_claim_due.sql"),
    };
    pub const EXISTS: SqlStatement = SqlStatement {
        name: "outbox.exists",
        sql: include_str!("../sql/queries/outbox_exists.sql"),
    };
    pub const RESCHEDULE: SqlStatement = SqlStatement {
        name: "outbox.reschedule",
        sql: include_str!("../sql/commands/outbox_reschedule.sql"),
    };
    pub const COMPLETE: SqlStatement = SqlStatement {
        name: "outbox.complete",
        sql: include_str!("../sql/commands/outbox_complete.sql"),
    };
    pub const DEAD_LETTER: SqlStatement = SqlStatement {
        name: "outbox.dead_letter",
        sql: include_str!("../sql/commands/outbox_dead_letter.sql"),
    };
    pub const RECOVER_STALE: SqlStatement = SqlStatement {
        name: "outbox.recover_stale",
        sql: include_str!("../sql/commands/outbox_recover_stale.sql"),
    };
    pub const INSERT_INBOUND_DEDUP: SqlStatement = SqlStatement {
        name: "inbound_dedup.insert",
        sql: include_str!("../sql/commands/inbound_dedup_insert.sql"),
    };
}
