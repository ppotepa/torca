use torca_foundation::{OpaqueId, Timestamp};
use torca_relay::{PAIRING_SLOT_TTL, RelayBroker};
use torca_relay_protocol::{
    RelayCode, RelayProtocolError, RelayRequest, RelayResponse, RelaySideToken, RelaySlotCapability,
};

#[test]
fn slot_id_alone_does_not_authorize_poll_or_close() {
    let now = Timestamp::UNIX_EPOCH;
    let expires_at = Timestamp::from_unix_millis(60_000).expect("expiry");
    let code = RelayCode::new("ABC123").expect("code");
    let creator = RelaySideToken(OpaqueId::from_u128(11));
    let joiner = RelaySideToken(OpaqueId::from_u128(12));
    let attacker = RelaySideToken(OpaqueId::from_u128(13));
    let capability = RelaySlotCapability(OpaqueId::from_u128(21));
    let wrong_capability = RelaySlotCapability(OpaqueId::from_u128(22));
    let mut relay = RelayBroker::default();

    let slot_id = match relay
        .handle(
            RelayRequest::Open {
                code: code.clone(),
                expires_at,
                creator_blob: vec![1, 2, 3],
                slot_capability: capability,
                creator_token: creator,
            },
            now,
        )
        .expect("open")
    {
        RelayResponse::Opened { slot_id } => slot_id,
        other => panic!("unexpected response: {other:?}"),
    };

    let _ = relay
        .handle(RelayRequest::Join { code, joiner_blob: vec![4, 5, 6], joiner_token: joiner }, now)
        .expect("join");

    assert_eq!(
        relay.handle(RelayRequest::Poll { slot_id, token: attacker }, now),
        Err(RelayProtocolError::Unauthorized)
    );
    assert_eq!(
        relay.handle(RelayRequest::Close { slot_id, capability: wrong_capability }, now,),
        Err(RelayProtocolError::Unauthorized)
    );
    assert!(matches!(
        relay.handle(RelayRequest::Poll { slot_id, token: creator }, now),
        Ok(RelayResponse::Blobs(_))
    ));
    assert_eq!(
        relay.handle(RelayRequest::Close { slot_id, capability }, now),
        Ok(RelayResponse::Closed)
    );
}

#[test]
fn relay_clock_caps_invitation_lifetime_at_five_minutes() {
    let now = Timestamp::UNIX_EPOCH;
    let code = RelayCode::new("ABC123").expect("code");
    let creator = RelaySideToken(OpaqueId::from_u128(31));
    let mut relay = RelayBroker::default();
    let _ = relay
        .handle(
            RelayRequest::Open {
                code: code.clone(),
                // A client cannot extend a relay-held invitation by supplying
                // a longer local deadline.
                expires_at: Timestamp::from_unix_millis(3_600_000).expect("expiry"),
                creator_blob: vec![1],
                slot_capability: RelaySlotCapability(OpaqueId::from_u128(32)),
                creator_token: creator,
            },
            now,
        )
        .expect("open");
    let after_ttl = now.checked_add(PAIRING_SLOT_TTL).expect("deadline");
    let after_ttl = after_ttl.checked_add(std::time::Duration::from_millis(1)).expect("later");
    assert_eq!(
        relay.handle(
            RelayRequest::Join {
                code,
                joiner_blob: vec![2],
                joiner_token: RelaySideToken(OpaqueId::from_u128(33)),
            },
            after_ttl,
        ),
        Err(RelayProtocolError::SlotNotFound)
    );
}
