fn is_terminal(state: PairingState) -> bool {
    matches!(
        state,
        PairingState::Rejected
            | PairingState::Cancelled
            | PairingState::Expired
            | PairingState::Completed
    )
}

fn peer_proposal(offer: &PairingOffer) -> Result<PeerProposal, PairingRuntimeError> {
    let algorithm = match offer.key_algorithm {
        1 => KeyAlgorithm::Ed25519,
        _ => return Err(PairingRuntimeError::UnsupportedAlgorithm),
    };
    let key = IdentityKey::new(
        torca_identity::KeyId::from_opaque(offer.key_id),
        algorithm,
        offer.public_key.clone(),
    )
    .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    let public_identity = PublicIdentity::new(
        torca_identity::IdentityId::from_opaque(offer.identity_id),
        key,
        offer.key_generation,
    );
    let route = ContactRoute::new(offer.onion_address.clone(), offer.capability_id)
        .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    Ok(PeerProposal {
        public_identity,
        display_name: offer.display_name.clone(),
        route,
        avatar: offer.avatar.as_ref().map(|avatar| AvatarGenomeReference {
            schema_version: avatar.schema,
            generator_version: avatar.generator_version.clone(),
            catalog_version: avatar.catalog_version.clone(),
            genome_hash: avatar.genome_hash,
            compressed_genome: avatar.compressed_genome.clone(),
        }),
    })
}

struct PersistedPairingState {
    transport: PairingTransportSnapshot,
    local_offer: Option<PairingEnvelope>,
    remote_offer: Option<PairingEnvelope>,
    completion_sent: bool,
    completion_applied: bool,
    completion_ack_sent: bool,
}

fn encode_persisted_state(
    transport: PairingTransportSnapshot,
    local_offer: Option<&PairingEnvelope>,
    remote_offer: Option<&PairingEnvelope>,
    completion_sent: bool,
    completion_applied: bool,
    completion_ack_sent: bool,
) -> Result<Vec<u8>, PairingRuntimeError> {
    let PairingTransportSnapshot {
        role,
        context,
        mut private_key,
        slot,
        token,
        slot_capability,
        remote_public_key,
    } = transport;
    let local = encode_optional_offer(local_offer)?;
    let remote = encode_optional_offer(remote_offer)?;
    let mut output = Vec::with_capacity(128 + local.len() + remote.len());
    output.push(PAIRING_STATE_VERSION);
    output.push(match role {
        PairingRole::Creator => 1,
        PairingRole::Joiner => 2,
    });
    output.extend_from_slice(&private_key);
    private_key.fill(0);
    output.extend_from_slice(&context.0.into_bytes());
    output.extend_from_slice(&slot.0.into_bytes());
    output.extend_from_slice(&token.0.into_bytes());
    match slot_capability {
        Some(capability) => {
            output.push(1);
            output.extend_from_slice(&capability.0.into_bytes());
        }
        None => output.push(0),
    }
    match remote_public_key {
        Some(key) => {
            output.push(1);
            output.extend_from_slice(&key);
        }
        None => output.push(0),
    }
    output.push(u8::from(completion_sent));
    output.push(u8::from(completion_applied));
    output.push(u8::from(completion_ack_sent));
    output.extend_from_slice(&local);
    output.extend_from_slice(&remote);
    Ok(output)
}

fn encode_optional_offer(offer: Option<&PairingEnvelope>) -> Result<Vec<u8>, PairingRuntimeError> {
    let bytes = match offer {
        Some(offer) => offer.encode().map_err(|_| PairingRuntimeError::InvalidOffer)?,
        None => Vec::new(),
    };
    let length = u16::try_from(bytes.len()).map_err(|_| PairingRuntimeError::InvalidOffer)?;
    let mut output = Vec::with_capacity(2 + bytes.len());
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&bytes);
    Ok(output)
}

fn decode_persisted_state(
    _session_id: PairingSessionId,
    bytes: &[u8],
) -> Result<PersistedPairingState, PairingRuntimeError> {
    let mut input = bytes;
    if take_u8(&mut input)? != PAIRING_STATE_VERSION {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    let role = match take_u8(&mut input)? {
        1 => PairingRole::Creator,
        2 => PairingRole::Joiner,
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let private_key = take_array::<32>(&mut input)?;
    let context = crate::PairingContextId(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let slot = PairingSlotId(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let token = PairingSideToken(OpaqueId::from_bytes(take_array::<16>(&mut input)?));
    let slot_capability = match take_u8(&mut input)? {
        0 => None,
        1 => Some(PairingSlotCapability(OpaqueId::from_bytes(take_array::<16>(&mut input)?))),
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let remote_public_key = match take_u8(&mut input)? {
        0 => None,
        1 => Some(take_array::<32>(&mut input)?),
        _ => return Err(PairingRuntimeError::InvalidOffer),
    };
    let completion_sent = take_bool(&mut input)?;
    let completion_applied = take_bool(&mut input)?;
    let completion_ack_sent = take_bool(&mut input)?;
    let local_offer = decode_optional_offer(context, &mut input)?;
    let remote_offer = decode_optional_offer(context, &mut input)?;
    if !input.is_empty() {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    Ok(PersistedPairingState {
        transport: PairingTransportSnapshot {
            role,
            context,
            private_key,
            slot,
            token,
            slot_capability,
            remote_public_key,
        },
        local_offer,
        remote_offer,
        completion_sent,
        completion_applied,
        completion_ack_sent,
    })
}

fn decode_optional_offer(
    context: crate::PairingContextId,
    input: &mut &[u8],
) -> Result<Option<PairingEnvelope>, PairingRuntimeError> {
    let length = usize::from(u16::from_be_bytes(take_array::<2>(input)?));
    if length == 0 {
        return Ok(None);
    }
    let envelope = PairingEnvelope::decode(take(input, length)?)
        .map_err(|_| PairingRuntimeError::InvalidOffer)?;
    envelope.validate_pairing_id(context.0).map_err(|_| PairingRuntimeError::InvalidOffer)?;
    if !matches!(&envelope.payload, PairingPayload::Offer(_)) {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    Ok(Some(envelope))
}

fn take_bool(input: &mut &[u8]) -> Result<bool, PairingRuntimeError> {
    match take_u8(input)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(PairingRuntimeError::InvalidOffer),
    }
}

fn take_u8(input: &mut &[u8]) -> Result<u8, PairingRuntimeError> {
    Ok(take(input, 1)?[0])
}

fn take_array<const N: usize>(input: &mut &[u8]) -> Result<[u8; N], PairingRuntimeError> {
    take(input, N)?.try_into().map_err(|_| PairingRuntimeError::InvalidOffer)
}

fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8], PairingRuntimeError> {
    if input.len() < length {
        return Err(PairingRuntimeError::InvalidOffer);
    }
    let (head, tail) = input.split_at(length);
    *input = tail;
    Ok(head)
}
