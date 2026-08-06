/// A named SQL statement embedded at compile time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SqlStatement { pub name: &'static str, pub sql: &'static str }

/// Identity SQL catalog.
pub mod identity_sql {
    use super::SqlStatement;
    /// Inserts identity.
    pub const INSERT: SqlStatement = SqlStatement { name: "identity.insert", sql: include_str!("../sql/commands/identity_insert.sql") };
    /// Updates identity.
    pub const UPDATE: SqlStatement = SqlStatement { name: "identity.update", sql: include_str!("../sql/commands/identity_update.sql") };
    /// Selects identity.
    pub const SELECT: SqlStatement = SqlStatement { name: "identity.select", sql: include_str!("../sql/queries/identity_select.sql") };
}
/// Messaging and durable-work SQL catalog.
pub mod messaging_sql {
    use super::SqlStatement;
    /// Inserts message and outbox work in one transaction.
    pub const INSERT_OUTBOUND: SqlStatement = SqlStatement { name: "messaging.insert_outbound", sql: include_str!("../sql/commands/message_insert_outbound.sql") };
    /// Claims due outbox work.
    pub const CLAIM_DUE: SqlStatement = SqlStatement { name: "outbox.claim_due", sql: include_str!("../sql/queries/outbox_claim_due.sql") };
    /// Records inbound envelope deduplication.
    pub const INSERT_INBOUND_DEDUP: SqlStatement = SqlStatement { name: "inbound_dedup.insert", sql: include_str!("../sql/commands/inbound_dedup_insert.sql") };
}
