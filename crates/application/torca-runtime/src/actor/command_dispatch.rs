// Responsibility: runtime command dispatch into engine and narrow communication ports.

fn handle_command<P: PairingDriver, C: CommunicationDriver, T: TorDriver>(
    command: RuntimeCommand,
    engine: &EngineHandle,
    pairing: &mut P,
    communication: &mut C,
    tor: &T,
    probes: &ProbeSupervisor,
    relay_info: Option<&Arc<dyn RelayProbe>>,
    relay_health: Option<&RelayHealthHandle>,
    transport_activity: &mut TransportActivityLedger,
    connectivity: &ConnectivityObserver,
    policy: &mut RuntimeGovernor,
    active_attachment_leases: &mut BTreeSet<OpaqueId>,
    diagnostics: &mut DiagnosticBuffer,
    sequence: &mut u128,
    now: Timestamp,
) {
    match command {
        RuntimeCommand::CreatePairing(id, r) => {
            acquire_pairing_lease(policy, id);
            wake_relay(relay_health);
            let result = pairing.create(id, now);
            record_pairing_result(&result, "CREATE", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::JoinPairing(id, code, ticket, r) => {
            acquire_pairing_lease(policy, id);
            wake_relay(relay_health);
            let result = pairing.join(id, code, ticket, now);
            if result.is_ok() {
                communication.prime_connections();
            }
            record_pairing_result(&result, "JOIN", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::ApprovePairing(id, r) => {
            acquire_pairing_lease(policy, id);
            wake_relay(relay_health);
            let result = pairing.approve(id, now);
            if result.is_ok() {
                policy.release_lease(pairing_lease_owner(id));
                communication.prime_connections();
            }
            record_pairing_result(&result, "APPROVE", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::RejectPairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.reject(id);
            if result.is_ok() {
                policy.release_lease(pairing_lease_owner(id));
            }
            record_pairing_result(&result, "REJECT", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::CancelPairing(id, r) => {
            wake_relay(relay_health);
            let result = pairing.cancel(id);
            if result.is_ok() {
                policy.release_lease(pairing_lease_owner(id));
            }
            record_pairing_result(&result, "CANCEL", diagnostics, sequence, now);
            let _ = r.send(result);
        }
        RuntimeCommand::VerifyContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.verify_contact(id, now));
        }
        RuntimeCommand::ResetContactVerification(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.reset_contact_verification(id));
        }
        RuntimeCommand::RenameContact(id, name, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.rename_contact(id, name, now));
        }
        RuntimeCommand::BlockContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.block_contact(id, now));
        }
        RuntimeCommand::UnblockContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.unblock_contact(id, now));
        }
        RuntimeCommand::RemoveContact(id, r) => {
            transport_activity.mark_peer(id, now);
            let _ = r.send(communication.remove_contact(id));
        }
        RuntimeCommand::ClearConversationHistory(id, r) => {
            let _ = r.send(communication.clear_conversation_history(id));
        }
        RuntimeCommand::MarkConversationRead(id, r) => {
            let _ = r.send(communication.mark_conversation_read(id, now));
        }
        RuntimeCommand::QueueAttachment(request_value, r) => {
            let message_id = MessageId::from_opaque(request_value.message_id);
            let body = MessageBody::new(format!("Attachment: {}", request_value.name))
                .map_err(|_| RuntimeDriverError::Communication);
            let result = body.and_then(|body| {
                if let Err(error) = engine.dispatch(EngineCommand::QueueMessage {
                    message_id,
                    conversation_id: ConversationId::from_opaque(request_value.conversation_id),
                    body,
                    reply_to: None,
                    at: now,
                }) {
                    return Err(RuntimeDriverError::from(error));
                }
                if let Err(error) = communication.prepare_attachment(&request_value, now) {
                    let _ = engine.dispatch(EngineCommand::CancelMessage { message_id, at: now });
                    return Err(error);
                }
                Ok(())
            });
            if result.is_ok() {
                acquire_attachment_lease(
                    policy,
                    active_attachment_leases,
                    request_value.attachment_id,
                );
            }
            let _ = r.send(result);
        }
        RuntimeCommand::QueueReaction(contact_id, reaction, at, r) => {
            let _ = r.send(communication.queue_reaction(contact_id, reaction, at));
        }
        RuntimeCommand::RetryAttachment(id, r) => {
            let result = communication.retry_attachment(id, now);
            if result.is_ok() {
                acquire_attachment_lease(policy, active_attachment_leases, id);
            }
            let _ = r.send(result);
        }
        RuntimeCommand::CancelAttachment(id, r) => {
            let result = communication.cancel_attachment(id, now);
            if result.is_ok() {
                active_attachment_leases.remove(&id);
                policy.release_lease(attachment_lease_owner(id));
            }
            let _ = r.send(result);
        }
        RuntimeCommand::ExportAttachment(id, destination, r) => {
            let _ = r.send(communication.export_attachment(id, destination));
        }
        RuntimeCommand::ExportAttachmentPreview(id, destination, r) => {
            let _ = r.send(communication.export_attachment_preview(id, destination));
        }
        RuntimeCommand::AttachmentSnapshot(r) => {
            diagnostics.count(RuntimeCounter::FfiWake);
            let result = communication.attachment_snapshot();
            if let Ok(views) = &result {
                for view in views {
                    if matches!(view.status.as_str(), "available" | "cancelled") {
                        active_attachment_leases.remove(&view.id);
                        policy.release_lease(attachment_lease_owner(view.id));
                    }
                }
            }
            let _ = r.send(result);
        }
        RuntimeCommand::NetworkSnapshot(r) => {
            diagnostics.count(RuntimeCounter::FfiWake);
            let result = (|| {
                let snapshot = engine.overview_snapshot().map_err(RuntimeDriverError::from)?;
                let peers = snapshot
                    .contacts
                    .iter()
                    .map(|c| (c.id(), communication.connection_state(c.id())))
                    .collect();
                let peer_health = snapshot
                    .contacts
                    .iter()
                    .map(|c| (c.id(), communication.peer_health(c.id())))
                    .collect();
                let contact_names = communication.contact_names()?;
                let contact_verifications = communication.contact_verifications()?;
                Ok(NetworkSnapshot {
                    tor: tor.state(),
                    onion_address: tor.onion_address(),
                    peers,
                    peer_health,
                    contact_names,
                    contact_verifications,
                    peer_activity: transport_activity.peers.clone(),
                    probes: probes.latest(),
                    connectivity: connectivity.snapshot(),
                    relay_info: relay_info.and_then(|source| source.service_info()),
                })
            })();
            let _ = r.send(result);
        }
        RuntimeCommand::Diagnostics(r) => {
            diagnostics.count(RuntimeCounter::FfiWake);
            let _ = r.send(diagnostics.export_json());
        }
        RuntimeCommand::Wake => {}
        RuntimeCommand::WakeDelivery(_) | RuntimeCommand::ReleaseDelivery(_) => unreachable!(),
        RuntimeCommand::SetAttention(_) => unreachable!(),
        RuntimeCommand::NetworkChanged => unreachable!(),
        RuntimeCommand::SetRadioDemand(_, _)
        | RuntimeCommand::SetRadioTransmission(_, _)
        | RuntimeCommand::SetInstantContactDemand(_, _) => {
            unreachable!()
        }
        RuntimeCommand::SetBatteryProfile(_) => unreachable!(),
        RuntimeCommand::SetForeground(_) => unreachable!(),
        RuntimeCommand::SetMeteredNetwork(_) | RuntimeCommand::SetMeteredTransferPolicy(_) => {
            unreachable!()
        }
        RuntimeCommand::SetTorDormancy(_) => unreachable!(),
        RuntimeCommand::Shutdown(_) => unreachable!(),
    }
}

fn wake_relay(relay_health: Option<&RelayHealthHandle>) {
    if let Some(relay_health) = relay_health {
        relay_health.wake();
    }
}
