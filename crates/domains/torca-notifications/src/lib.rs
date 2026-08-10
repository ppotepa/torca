//! Privacy-safe notification intent policy independent of operating-system APIs.

use torca_foundation::OpaqueId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationPrivacy {
    Full,
    Redacted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationEvent {
    IncomingMessage { contact_id: OpaqueId, conversation_id: OpaqueId },
    ContactAdded { contact_id: OpaqueId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationIntent {
    pub id: OpaqueId,
    pub replacement_key: OpaqueId,
    pub title: String,
    pub body: String,
    pub navigation_target: Option<OpaqueId>,
}

/// Produces an intent or suppresses a redundant foreground notification.
pub fn notification_intent(
    event: NotificationEvent,
    privacy: NotificationPrivacy,
    foreground_conversation: Option<OpaqueId>,
) -> Option<NotificationIntent> {
    match event {
        NotificationEvent::IncomingMessage { contact_id: _, conversation_id } => {
            if foreground_conversation == Some(conversation_id) {
                return None;
            }
            let (title, body) = match privacy {
                NotificationPrivacy::Full => {
                    ("New private message".to_owned(), "Open conversation".to_owned())
                }
                NotificationPrivacy::Redacted => {
                    ("New message".to_owned(), "Private message received".to_owned())
                }
            };
            Some(NotificationIntent {
                id: conversation_id,
                replacement_key: conversation_id,
                title,
                body,
                navigation_target: Some(conversation_id),
            })
        }
        NotificationEvent::ContactAdded { contact_id } => Some(NotificationIntent {
            id: contact_id,
            replacement_key: contact_id,
            title: "New contact added".to_owned(),
            body: match privacy {
                NotificationPrivacy::Full => "Your pairing completed".to_owned(),
                NotificationPrivacy::Redacted => "A private contact is ready".to_owned(),
            },
            navigation_target: Some(contact_id),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{NotificationEvent, NotificationPrivacy, notification_intent};
    use torca_foundation::OpaqueId;

    #[test]
    fn foreground_conversation_is_suppressed() {
        let conversation = OpaqueId::from_u128(2);
        assert!(
            notification_intent(
                NotificationEvent::IncomingMessage {
                    contact_id: OpaqueId::from_u128(1),
                    conversation_id: conversation,
                },
                NotificationPrivacy::Redacted,
                Some(conversation),
            )
            .is_none()
        );
    }
}
