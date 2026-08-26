fn send_with_timeout<T>(
    sender: &SyncSender<T>,
    mut message: T,
    timeout: Duration,
) -> Result<(), ()> {
    let deadline = Instant::now() + timeout;
    loop {
        match sender.try_send(message) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => return Err(()),
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    return Err(());
                }
                message = returned;
                thread::yield_now();
            }
        }
    }
}

fn bridge_command(
    name: &str,
    payload: &Value,
) -> Result<BridgeCommand, (&'static str, &'static str)> {
    let text = |field: &str| {
        payload
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))
    };
    let generated = || secure_id_hex().map_err(|_| ("RUNTIME_ID_FAILED", "runtime.id.failed"));
    let now = || now_ms().map_err(|_| ("CLOCK_UNAVAILABLE", "runtime.clock.unavailable"));
    match name {
        "notifications.set" => Ok(BridgeCommand::SetNotifications {
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
        }),
        "privacy.read_receipts.set" => Ok(BridgeCommand::SetReadReceipts {
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
        }),
        "battery.preferences.set" => Ok(BridgeCommand::SetBatteryPreferences {
            mode: text("mode")?,
            background_sync: text("backgroundSync")?,
            allow_delayed_background_delivery: payload
                .get("allowDelayedBackgroundDelivery")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
            metered_transfers: text("meteredTransfers")?,
            visual_activity: text("visualActivity")?,
        }),
        "contact.availability.set" => Ok(BridgeCommand::SetContactAvailability {
            contact_id_hex: text("contactIdHex")?,
            mode: text("mode")?,
        }),
        "contacts.acknowledge_new" => Ok(BridgeCommand::AcknowledgeNewContacts),
        "diagnostics.observation.start" => Ok(BridgeCommand::StartBatteryObservation),
        "diagnostics.observation.stop" => Ok(BridgeCommand::StopBatteryObservation),
        "diagnostics.observation.reset" => Ok(BridgeCommand::ResetBatteryObservation),
        "diagnostics.incident.mark" => Ok(BridgeCommand::MarkIncident),
        "profile.set" => Ok(BridgeCommand::UpdateProfile {
            display_name: text("displayName")?,
            avatar_envelope_json: payload.get("avatarEnvelope").map(serde_json::Value::to_string),
            at_ms: now()?,
        }),
        "runtime.attention.set" => Ok(BridgeCommand::SetAttention {
            surface: text("surface")?,
            focused_resource_id: payload
                .get("focusedResourceId")
                .and_then(Value::as_str)
                .map(str::to_owned),
            visible_contact_ids: payload
                .get("visibleContactIds")
                .and_then(Value::as_array)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))
                })
                .collect::<Result<Vec<_>, _>>()?,
            generation: payload
                .get("generation")
                .and_then(Value::as_u64)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
        }),
        "pairing.create" => Ok(BridgeCommand::CreatePairing { session_id_hex: generated()? }),
        "pairing.join" => Ok(BridgeCommand::JoinPairing {
            session_id_hex: generated()?,
            code: text("code")?,
            ticket: payload.get("ticket").and_then(Value::as_str).map(str::to_owned),
            bootstrap_json: payload.get("bootstrap").filter(|value| value.is_object()).map(Value::to_string),
        }),
        "pairing.approve" => {
            Ok(BridgeCommand::ApprovePairing { session_id_hex: text("sessionIdHex")? })
        }
        "pairing.reject" => {
            Ok(BridgeCommand::RejectPairing { session_id_hex: text("sessionIdHex")? })
        }
        "pairing.cancel" => {
            Ok(BridgeCommand::CancelPairing { session_id_hex: text("sessionIdHex")? })
        }
        "contact.rename" => Ok(BridgeCommand::RenameContact {
            contact_id_hex: text("contactIdHex")?,
            display_name: text("displayName")?,
        }),
        "contact.verify" => {
            Ok(BridgeCommand::VerifyContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.verification.reset" => {
            Ok(BridgeCommand::ResetContactVerification { contact_id_hex: text("contactIdHex")? })
        }
        "contact.block" => {
            Ok(BridgeCommand::BlockContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.unblock" => {
            Ok(BridgeCommand::UnblockContact { contact_id_hex: text("contactIdHex")? })
        }
        "contact.remove" => {
            Ok(BridgeCommand::RemoveContact { contact_id_hex: text("contactIdHex")? })
        }
        "conversation.start" => {
            Ok(BridgeCommand::StartConversation { contact_id_hex: text("contactIdHex")? })
        }
        "conversation.clear" => Ok(BridgeCommand::ClearConversationHistory {
            conversation_id_hex: text("conversationIdHex")?,
        }),
        "conversation.archive" => Ok(BridgeCommand::ArchiveConversation {
            conversation_id_hex: text("conversationIdHex")?,
            at_ms: now()?,
        }),
        "conversation.restore" => Ok(BridgeCommand::RestoreConversation {
            conversation_id_hex: text("conversationIdHex")?,
            at_ms: now()?,
        }),
        "message.send" => Ok(BridgeCommand::QueueMessage {
            message_id_hex: generated()?,
            conversation_id_hex: text("conversationIdHex")?,
            body: text("body")?,
            reply_to_message_id_hex: payload
                .get("replyToMessageIdHex")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            at_ms: now()?,
        }),
        "message.retry" => {
            Ok(BridgeCommand::RetryMessage { message_id_hex: text("messageIdHex")?, at_ms: now()? })
        }
        "message.cancel" => Ok(BridgeCommand::CancelMessage {
            message_id_hex: text("messageIdHex")?,
            at_ms: now()?,
        }),
        "message.edit" => Ok(BridgeCommand::EditMessage {
            message_id_hex: text("messageIdHex")?,
            body: text("body")?,
            at_ms: now()?,
        }),
        "message.reaction" => Ok(BridgeCommand::SetMessageReaction {
            message_id_hex: text("messageIdHex")?,
            conversation_id_hex: text("conversationIdHex")?,
            actor_id_hex: text("actorIdHex")?,
            emoji: text("emoji")?,
            active: payload.get("active").and_then(Value::as_bool).unwrap_or(true),
            at_ms: now()?,
        }),
        "conversation.read" => {
            let conversation_id_hex = text("conversationIdHex")?;
            Ok(BridgeCommand::MarkConversationRead { conversation_id_hex })
        }
        "attachment.queue" => Ok(BridgeCommand::QueueAttachment {
            attachment_id_hex: generated()?,
            message_id_hex: generated()?,
            conversation_id_hex: text("conversationIdHex")?,
            source_path: text("sourcePath")?,
            preview_source_path: payload
                .get("previewSourcePath")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            name: text("name")?,
            media_type: text("mediaType")?,
            size: payload
                .get("size")
                .and_then(Value::as_u64)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
            at_ms: now()?,
        }),
        "attachment.retry" => {
            Ok(BridgeCommand::RetryAttachment { attachment_id_hex: text("attachmentIdHex")? })
        }
        "attachment.cancel" => {
            Ok(BridgeCommand::CancelAttachment { attachment_id_hex: text("attachmentIdHex")? })
        }
        "attachment.export" => Ok(BridgeCommand::ExportAttachment {
            attachment_id_hex: text("attachmentIdHex")?,
            destination_path: text("destinationPath")?,
        }),
        "attachment.preview.export" => Ok(BridgeCommand::ExportAttachmentPreview {
            attachment_id_hex: text("attachmentIdHex")?,
            destination_path: text("destinationPath")?,
        }),
        "radio.set_enabled" => Ok(BridgeCommand::SetRadioEnabled {
            contact_id_hex: text("contactIdHex")?,
            enabled: payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(("CONTRACT_PAYLOAD_INVALID", "contract.payload.invalid"))?,
            at_ms: now()?,
        }),
        "radio.audio.configure" => Ok(BridgeCommand::ConfigureRadioAudio {
            input_device_id: payload
                .get("inputDeviceId")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            output_device_id: payload
                .get("outputDeviceId")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        }),
        "radio.transmission.begin" => {
            Ok(BridgeCommand::BeginRadioTransmission { contact_id_hex: text("contactIdHex")? })
        }
        "radio.transmission.end" => {
            Ok(BridgeCommand::EndRadioTransmission { contact_id_hex: text("contactIdHex")? })
        }
        "provider.route.refresh" => Ok(BridgeCommand::RefreshProviderRoute),
        _ => Err(("CONTRACT_OPERATION_UNKNOWN", "contract.operation.unknown")),
    }
}

pub(crate) fn secure_id_hex() -> Result<String, ()> {
    let mut provider = RustCryptoProvider;
    let mut bytes = [0_u8; 16];
    provider.fill_random(&mut bytes).map_err(|_| ())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn now_ms() -> Result<i64, ()> {
    let value = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ())?.as_millis();
    i64::try_from(value).map_err(|_| ())
}

fn is_idempotent_command(kind: &str) -> bool {
    kind == "command"
}

fn operation_counts_for_revision(kind: &str, name: &str) -> bool {
    kind == "command" || kind == "lifecycle" || name == "snapshot.get"
}
