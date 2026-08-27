use crate::domain::{FieldAvailability, FieldCapability};

pub fn describe(field: &FieldCapability) -> String {
    let state = match &field.availability {
        FieldAvailability::Editable => "editable".to_owned(),
        FieldAvailability::ReadOnly { reason } => format!("read-only: {reason}"),
        FieldAvailability::Disabled { reason } => format!("disabled: {reason}"),
        FieldAvailability::Hidden => "hidden".to_owned(),
    };
    format!("{} [{state}] — {}", field.label, field.description)
}
