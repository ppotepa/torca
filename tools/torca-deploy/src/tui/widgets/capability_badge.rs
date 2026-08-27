use crate::domain::FieldAvailability;

pub fn availability_label(availability: &FieldAvailability) -> &'static str {
    match availability {
        FieldAvailability::Editable => "editable",
        FieldAvailability::ReadOnly { .. } => "read-only",
        FieldAvailability::Disabled { .. } => "disabled",
        FieldAvailability::Hidden => "hidden",
    }
}
