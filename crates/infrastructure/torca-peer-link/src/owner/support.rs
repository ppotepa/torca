enum AuthOutcome {
    Waiting,
    Authenticated(ContactId),
}

fn map_state(state: PeerSessionState) -> PeerConnectionState {
    match state {
        PeerSessionState::Disconnected => PeerConnectionState::Disconnected,
        PeerSessionState::Connecting => PeerConnectionState::Connecting,
        PeerSessionState::Handshaking => PeerConnectionState::Handshaking,
        PeerSessionState::Ready => PeerConnectionState::Ready,
        PeerSessionState::Reconnecting => PeerConnectionState::Reconnecting,
        PeerSessionState::Closed | PeerSessionState::Failed => PeerConnectionState::Failed,
    }
}

fn verifier_for(contact: &Contact) -> Result<Ed25519HandshakeVerifier, PeerLinkError> {
    let public: [u8; 32] = contact
        .remote_identity()
        .key()
        .public_key()
        .try_into()
        .map_err(|_| PeerLinkError::Unauthorized)?;
    Ok(Ed25519HandshakeVerifier::from_bytes(public))
}

fn map_contact(_: ContactError) -> PeerLinkError {
    PeerLinkError::Repository
}
fn map_session(_: PeerSessionError) -> PeerLinkError {
    PeerLinkError::Protocol
}
fn map_transport_factory(error: torca_transport_api::TransportFactoryError) -> PeerLinkError {
    match error {
        torca_transport_api::TransportFactoryError::Listener => PeerLinkError::Listener,
        torca_transport_api::TransportFactoryError::ContactNotFound => {
            PeerLinkError::ContactNotFound
        }
        torca_transport_api::TransportFactoryError::Protocol => PeerLinkError::Protocol,
    }
}

fn system_timestamp() -> Result<Timestamp, PeerLinkError> {
    let duration =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| PeerLinkError::Clock)?;
    let millis = i64::try_from(duration.as_millis()).map_err(|_| PeerLinkError::Clock)?;
    Timestamp::from_unix_millis(millis).map_err(|_| PeerLinkError::Clock)
}
