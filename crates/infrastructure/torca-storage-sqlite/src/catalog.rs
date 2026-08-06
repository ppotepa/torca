/// A named SQL statement embedded at compile time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SqlStatement { pub name: &'static str, pub sql: &'static str }

/// Identity SQL catalog.
pub mod identity_sql {
    use super::SqlStatement;
    /// Inserts the local identity row.
    pub const INSERT: SqlStatement = SqlStatement { name: "identity.insert", sql: include_str!("../sql/commands/identity_insert.sql") };
    /// Replaces the local identity row using optimistic generation matching.
    pub const UPDATE: SqlStatement = SqlStatement { name: "identity.update", sql: include_str!("../sql/commands/identity_update.sql") };
    /// Loads the local identity row.
    pub const SELECT: SqlStatement = SqlStatement { name: "identity.select", sql: include_str!("../sql/queries/identity_select.sql") };
}
