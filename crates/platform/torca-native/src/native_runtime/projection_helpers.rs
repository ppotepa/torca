impl TorcaRuntime {

fn apply_history_summaries(
    &self,
    snapshot: &mut torca_contract::BridgeSnapshot,
) -> Result<(), &'static str> {
    let summaries = self
        .read_models()
        .history
        .conversation_summaries()
        .map_err(|_| "conversation summaries unavailable")?;
    for conversation in &mut snapshot.conversations {
        let id = conversation
            .id
            .parse::<OpaqueId>()
            .map(ConversationId::from_opaque)
            .map_err(|_| "invalid conversation id in snapshot")?;
        let Some(summary) = summaries.get(&id) else { continue };
        conversation.unread_count = summary.unread_count;
        conversation.last_activity_at_ms = summary.last_activity_at.to_unix_millis();
        if let Some(message) = &summary.last_message {
            conversation.last_message_body = Some(message.body().as_str().to_owned());
            conversation.last_message_direction =
                Some(torca_contract::message_direction_name(message.direction()).into());
            conversation.last_message_status =
                Some(torca_contract::message_status_name(message.status()).into());
        }
    }
    Ok(())
}

fn apply_security_states(
    &self,
    snapshot: &mut torca_contract::BridgeSnapshot,
) -> Result<(), &'static str> {
    let states = self
        .read_models()
        .security
        .contact_states()
        .map_err(|_| "contact security state unavailable")?;
    for contact in &mut snapshot.contacts {
        let id = contact
            .id
            .parse::<OpaqueId>()
            .map(torca_contacts::ContactId::from_opaque)
            .map_err(|_| "invalid contact id in snapshot")?;
        let Some(security) = states.get(&id) else { continue };
        contact.verification_status = match security.state {
            ContactSecurityState::Unverified => "unverified",
            ContactSecurityState::Verified => "verified",
            ContactSecurityState::IdentityChanged => "changed",
        }
        .into();
        contact.verified_at_ms = security.verified_at.map(|at| at.to_unix_millis());
    }
    Ok(())
}

fn apply_navigation_badges(&self, snapshot: &mut torca_contract::BridgeSnapshot) {
    snapshot.unread_messages_count = snapshot
        .conversations
        .iter()
        .fold(0_u32, |total, conversation| total.saturating_add(conversation.unread_count));
    let acknowledged_at =
        self.read_models().settings.new_contacts_acknowledged_at_ms().ok().flatten();
    snapshot.new_contacts_count = snapshot
        .contacts
        .iter()
        .filter(|contact| match acknowledged_at {
            Some(at) => contact.created_at_ms > at,
            None => true,
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    snapshot.pairing_attention_count = snapshot
        .pairings
        .iter()
        .filter(|pairing| pairing.role == "creator" && pairing.state == "awaitingapproval")
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
}

fn query_error(&mut self, error: &str) -> i32 {
    self.last_result_json = error_result(error);
    self.query_json = "{\"messages\":[],\"hasMore\":false}".into();
    ABI_ERROR
}

}
