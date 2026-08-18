impl<R, C> PairingCoordinator<R, C>
where
    R: PairingRendezvousPort,
    C: PairingCryptoPort,
{
pub const fn new(rendezvous: R, crypto: C) -> Self {
    Self { rendezvous, crypto, sessions: BTreeMap::new() }
}

pub fn open_creator(
    &mut self,
    session_id: PairingSessionId,
    code: &PairingCode,
    expires_at: Timestamp,
    ticket: [u8; 16],
) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
    if self.sessions.contains_key(&session_id) {
        return Err(PairingCoordinatorError::SessionAlreadyExists);
    }
    let key = self.crypto.generate_ephemeral_key()?;
    let setup = (|| {
        let capability = PairingSlotCapability(self.random_id()?);
        let token = PairingSideToken(self.random_id()?);
        let (slot, relay_expires_at) = self.rendezvous.open(
            code,
            expires_at,
            key.public_key.to_vec(),
            capability,
            token,
            ticket,
        )?;
        Ok::<_, PairingCoordinatorError>((slot, relay_expires_at, capability, token))
    })();
    let (slot, relay_expires_at, capability, token) = match setup {
        Ok(value) => value,
        Err(error) => {
            let _ = self.crypto.release_ephemeral_key(key.handle);
            return Err(error);
        }
    };
    self.sessions.insert(
        session_id,
        TransportSession {
            role: LocalRole::Creator,
            context: PairingContextId(slot.0),
            key,
            slot,
            token,
            slot_capability: Some(capability),
            remote_public_key: None,
            acknowledged_through: 0,
        },
    );
    Ok((slot, relay_expires_at))
}

pub fn join(
    &mut self,
    session_id: PairingSessionId,
    code: &PairingCode,
    ticket: Option<[u8; 16]>,
) -> Result<(PairingSlotId, Timestamp), PairingCoordinatorError> {
    if self.sessions.contains_key(&session_id) {
        return Err(PairingCoordinatorError::SessionAlreadyExists);
    }
    let key = self.crypto.generate_ephemeral_key()?;
    let setup = (|| {
        let token = PairingSideToken(self.random_id()?);
        let (slot, relay_expires_at, creator_blob) =
            self.rendezvous.join(code, key.public_key.to_vec(), token, ticket)?;
        let creator_public_key: [u8; 32] =
            creator_blob.try_into().map_err(|_| PairingCoordinatorError::InvalidBlob)?;
        Ok::<_, PairingCoordinatorError>((slot, relay_expires_at, token, creator_public_key))
    })();
    let (slot, relay_expires_at, token, creator_public_key) = match setup {
        Ok(value) => value,
        Err(error) => {
            let _ = self.crypto.release_ephemeral_key(key.handle);
            return Err(error);
        }
    };
    self.sessions.insert(
        session_id,
        TransportSession {
            role: LocalRole::Joiner,
            context: PairingContextId(slot.0),
            key,
            slot,
            token,
            slot_capability: None,
            remote_public_key: Some(creator_public_key),
            acknowledged_through: 0,
        },
    );
    Ok((slot, relay_expires_at))
}

pub fn poll(
    &mut self,
    session_id: PairingSessionId,
) -> Result<PairingPollBatch, PairingCoordinatorError> {
    let session = self
        .sessions
        .get(&session_id)
        .cloned()
        .ok_or(PairingCoordinatorError::SessionNotFound)?;
    let deliveries =
        self.rendezvous.poll(session.slot, session.token, session.acknowledged_through)?;
    let mut envelopes = Vec::with_capacity(deliveries.len());
    let mut remote_public_key = session.remote_public_key;
    let mut received_through = None;
    for delivery in deliveries {
        let blob = delivery.blob;
        if remote_public_key.is_none() && blob.len() == 32 {
            let key = blob.try_into().map_err(|_| PairingCoordinatorError::InvalidBlob)?;
            remote_public_key = Some(key);
            if let Some(stored) = self.sessions.get_mut(&session_id) {
                stored.remote_public_key = Some(key);
            }
            received_through = Some(delivery.sequence);
            continue;
        }
        let encrypted = decode_encrypted(&blob)?;
        let remote = match remote_public_key {
            Some(expected) if expected != encrypted.sender_public_key => {
                return Err(PairingCoordinatorError::InvalidBlob);
            }
            Some(expected) => expected,
            None => encrypted.sender_public_key,
        };
        let plaintext = self.crypto.open_from_peer(
            session.key.handle,
            remote,
            encrypted.nonce,
            &associated_data(session.context),
            &encrypted.ciphertext,
        )?;
        let envelope = PairingEnvelope::decode(&plaintext)
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        envelope
            .validate_pairing_id(session.context.0)
            .map_err(|_| PairingCoordinatorError::Protocol)?;
        envelopes.push(envelope);
        received_through = Some(delivery.sequence);
        if remote_public_key.is_none() {
            remote_public_key = Some(remote);
            if let Some(stored) = self.sessions.get_mut(&session_id) {
                stored.remote_public_key = Some(remote);
            }
        }
    }
    Ok(PairingPollBatch { envelopes, received_through })
}

pub fn ack(
    &mut self,
    session_id: PairingSessionId,
    up_to: u64,
) -> Result<(), PairingCoordinatorError> {
    let session = self
        .sessions
        .get(&session_id)
        .cloned()
        .ok_or(PairingCoordinatorError::SessionNotFound)?;
    self.rendezvous.ack(session.slot, session.token, up_to)?;
    if let Some(stored) = self.sessions.get_mut(&session_id) {
        stored.acknowledged_through = stored.acknowledged_through.max(up_to);
    }
    Ok(())
}

pub fn network_changed(&mut self) {
    self.rendezvous.network_changed();
}

pub fn push(
    &mut self,
    session_id: PairingSessionId,
    envelope: &PairingEnvelope,
) -> Result<(), PairingCoordinatorError> {
    let session = self
        .sessions
        .get(&session_id)
        .cloned()
        .ok_or(PairingCoordinatorError::SessionNotFound)?;
    envelope
        .validate_pairing_id(session.context.0)
        .map_err(|_| PairingCoordinatorError::Protocol)?;
    let remote = session.remote_public_key.ok_or(PairingCoordinatorError::InvalidBlob)?;
    let encrypted = self.encrypt_envelope(session.context, &session.key, remote, envelope)?;
    let message_id = self.random_id()?;
    self.rendezvous.push(message_id, session.slot, session.token, encode_encrypted(&encrypted))
}

pub fn derive_peer_secret(
    &self,
    session_id: PairingSessionId,
    transcript_digest: [u8; 32],
) -> Result<PairingDerivedSecret, PairingCoordinatorError> {
    let session =
        self.sessions.get(&session_id).ok_or(PairingCoordinatorError::SessionNotFound)?;
    let remote = session.remote_public_key.ok_or(PairingCoordinatorError::InvalidBlob)?;
    self.crypto.derive_peer_secret(session.key.handle, remote, transcript_digest)
}

pub fn close(&mut self, session_id: PairingSessionId) -> Result<(), PairingCoordinatorError> {
    let session =
        self.sessions.remove(&session_id).ok_or(PairingCoordinatorError::SessionNotFound)?;
    let relay_result = if session.role == LocalRole::Creator {
        match session.slot_capability {
            Some(capability) => self.rendezvous.close(session.slot, capability),
            None => Err(PairingCoordinatorError::InvalidRole),
        }
    } else {
        Ok(())
    };
    let release_result = self.crypto.release_ephemeral_key(session.key.handle);
    relay_result?;
    release_result
}

pub fn detach(&mut self, session_id: PairingSessionId) -> Result<(), PairingCoordinatorError> {
    let session =
        self.sessions.remove(&session_id).ok_or(PairingCoordinatorError::SessionNotFound)?;
    self.crypto.release_ephemeral_key(session.key.handle)
}

pub fn export_transport(
    &self,
    session_id: PairingSessionId,
) -> Result<PairingTransportSnapshot, PairingCoordinatorError> {
    let session =
        self.sessions.get(&session_id).ok_or(PairingCoordinatorError::SessionNotFound)?;
    Ok(PairingTransportSnapshot {
        role: match session.role {
            LocalRole::Creator => PairingRole::Creator,
            LocalRole::Joiner => PairingRole::Joiner,
        },
        context: session.context,
        private_key: self.crypto.export_ephemeral_key(session.key.handle)?,
        slot: session.slot,
        token: session.token,
        slot_capability: session.slot_capability,
        remote_public_key: session.remote_public_key,
    })
}

pub fn restore_transport(
    &mut self,
    session_id: PairingSessionId,
    mut snapshot: PairingTransportSnapshot,
) -> Result<(), PairingCoordinatorError> {
    if self.sessions.contains_key(&session_id) {
        return Err(PairingCoordinatorError::SessionAlreadyExists);
    }
    let private_key = std::mem::take(&mut snapshot.private_key);
    let key = self.crypto.import_ephemeral_key(private_key)?;
    self.sessions.insert(
        session_id,
        TransportSession {
            role: match snapshot.role {
                PairingRole::Creator => LocalRole::Creator,
                PairingRole::Joiner => LocalRole::Joiner,
            },
            context: snapshot.context,
            key,
            slot: snapshot.slot,
            token: snapshot.token,
            slot_capability: snapshot.slot_capability,
            remote_public_key: snapshot.remote_public_key,
            acknowledged_through: 0,
        },
    );
    Ok(())
}

pub fn into_parts(self) -> (R, C) {
    (self.rendezvous, self.crypto)
}

pub fn context(
    &self,
    session_id: PairingSessionId,
) -> Result<PairingContextId, PairingCoordinatorError> {
    self.sessions
        .get(&session_id)
        .map(|session| session.context)
        .ok_or(PairingCoordinatorError::SessionNotFound)
}

fn encrypt_envelope(
    &mut self,
    context: PairingContextId,
    local_key: &PairingEphemeralKey,
    remote_public_key: [u8; 32],
    envelope: &PairingEnvelope,
) -> Result<EncryptedPairingPayload, PairingCoordinatorError> {
    let plaintext = envelope.encode().map_err(|_| PairingCoordinatorError::Protocol)?;
    let mut nonce = [0_u8; 24];
    self.crypto.fill_random(&mut nonce)?;
    let ciphertext = self.crypto.seal_for_peer(
        local_key.handle,
        remote_public_key,
        nonce,
        &associated_data(context),
        &plaintext,
    )?;
    Ok(EncryptedPairingPayload { sender_public_key: local_key.public_key, nonce, ciphertext })
}

fn random_id(&mut self) -> Result<OpaqueId, PairingCoordinatorError> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 16];
        self.crypto.fill_random(&mut bytes)?;
        let id = OpaqueId::from_bytes(bytes);
        if !id.is_nil() {
            return Ok(id);
        }
    }
    Err(PairingCoordinatorError::Crypto)
}
}
