/// Cursor-addressed notification projection. Runtime-owned events are filled
/// by the notification supervisor; this empty batch is safe before bootstrap.
#[allow(dead_code)]
pub(crate) fn notification_events_json(after_cursor: u64) -> String {
    format!("{{\"afterCursor\":{after_cursor},\"events\":[]}}")
}
