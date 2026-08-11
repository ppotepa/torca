use torca_foundation::{OpaqueId, Timestamp};
use torca_relay::{MAX_FAILED_JOINS_PER_MINUTE, PAIRING_SLOT_TTL, RelayBroker};
use torca_relay_protocol::{
    RelayCode, RelayJoinTicket, RelayMessageId, RelayOperationId, RelayProtocolError,
    RelayProtocolVersion, RelayRequest, RelayResponse, RelaySequence, RelaySideToken,
    RelaySlotCapability,
};

fn operation(value: u128) -> RelayOperationId {
    RelayOperationId(OpaqueId::from_u128(value))
}

#[test]
fn info_reports_the_running_relay_build() {
    let mut relay = RelayBroker::default();
    let response = relay.handle(RelayRequest::Info, Timestamp::UNIX_EPOCH).expect("relay info");
    let RelayResponse::Info(info) = response else {
        panic!("unexpected response: {response:?}");
    };
    assert!(!info.product_version.is_empty());
    assert!(!info.build_id.is_empty());
    assert!(!info.source_commit.is_empty());
    assert_eq!(info.protocol_version, RelayProtocolVersion::V4.0);
}

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
                operation_id: operation(1),
                code: code.clone(),
                expires_at,
                creator_blob: vec![1, 2, 3],
                slot_capability: capability,
                creator_token: creator,
                ticket: RelayJoinTicket([0; 16]),
            },
            now,
        )
        .expect("open")
    {
        RelayResponse::Opened { slot_id, .. } => slot_id,
        other => panic!("unexpected response: {other:?}"),
    };

    let _ = relay
        .handle(
            RelayRequest::Join {
                operation_id: operation(2),
                code,
                joiner_blob: vec![4, 5, 6],
                joiner_token: joiner,
                ticket: None,
            },
            now,
        )
        .expect("join");

    assert_eq!(
        relay
            .handle(RelayRequest::Poll { slot_id, token: attacker, after: RelaySequence(0) }, now,),
        Err(RelayProtocolError::Unauthorized)
    );
    assert_eq!(
        relay.handle(RelayRequest::Close { slot_id, capability: wrong_capability }, now,),
        Err(RelayProtocolError::Unauthorized)
    );
    assert!(matches!(
        relay.handle(RelayRequest::Poll { slot_id, token: creator, after: RelaySequence(0) }, now,),
        Ok(RelayResponse::Deliveries(_))
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
    let opened = relay
        .handle(
            RelayRequest::Open {
                operation_id: operation(3),
                code: code.clone(),
                // A client cannot extend a relay-held invitation by supplying
                // a longer local deadline.
                expires_at: Timestamp::from_unix_millis(3_600_000).expect("expiry"),
                creator_blob: vec![1],
                slot_capability: RelaySlotCapability(OpaqueId::from_u128(32)),
                creator_token: creator,
                ticket: RelayJoinTicket([0; 16]),
            },
            now,
        )
        .expect("open");
    assert!(matches!(
        opened,
        RelayResponse::Opened { expires_at, .. } if expires_at == now.checked_add(PAIRING_SLOT_TTL).expect("deadline")
    ));
    let after_ttl = now.checked_add(PAIRING_SLOT_TTL).expect("deadline");
    let after_ttl = after_ttl.checked_add(std::time::Duration::from_millis(1)).expect("later");
    assert_eq!(
        relay.handle(
            RelayRequest::Join {
                operation_id: operation(4),
                code,
                joiner_blob: vec![2],
                joiner_token: RelaySideToken(OpaqueId::from_u128(33)),
                ticket: None,
            },
            after_ttl,
        ),
        Err(RelayProtocolError::SlotNotFound)
    );
}

#[test]
fn failed_code_lookups_are_rate_limited_by_the_relay_clock() {
    let now = Timestamp::UNIX_EPOCH;
    let mut relay = RelayBroker::default();
    for value in 0..MAX_FAILED_JOINS_PER_MINUTE {
        assert_eq!(
            relay.handle(
                RelayRequest::Join {
                    operation_id: operation(u128::from(value) + 100),
                    code: RelayCode::new(format!("A{value:05}")).expect("code"),
                    joiner_blob: vec![1],
                    joiner_token: RelaySideToken(OpaqueId::from_u128(u128::from(value) + 1)),
                    ticket: None,
                },
                now,
            ),
            Err(RelayProtocolError::SlotNotFound)
        );
    }
    assert_eq!(
        relay.handle(
            RelayRequest::Join {
                operation_id: operation(1_999),
                code: RelayCode::new("ZZZZZZ").expect("code"),
                joiner_blob: vec![1],
                joiner_token: RelaySideToken(OpaqueId::from_u128(999)),
                ticket: None,
            },
            now,
        ),
        Err(RelayProtocolError::QueueFull)
    );
    let next_minute = now.checked_add(std::time::Duration::from_secs(60)).expect("later");
    assert_eq!(
        relay.handle(
            RelayRequest::Join {
                operation_id: operation(2_000),
                code: RelayCode::new("ZZZZZZ").expect("code"),
                joiner_blob: vec![1],
                joiner_token: RelaySideToken(OpaqueId::from_u128(1_000)),
                ticket: None,
            },
            next_minute,
        ),
        Err(RelayProtocolError::SlotNotFound)
    );
}

#[test]
fn qr_ticket_is_required_when_present_and_manual_join_remains_fallback() {
    let now = Timestamp::UNIX_EPOCH;
    let code = RelayCode::new("TICKET").expect("code");
    let creator = RelaySideToken(OpaqueId::from_u128(41));
    let ticket = RelayJoinTicket([7; 16]);
    let capability = RelaySlotCapability(OpaqueId::from_u128(42));
    let mut relay = RelayBroker::default();
    let _ = relay
        .handle(
            RelayRequest::Open {
                operation_id: operation(5),
                code: code.clone(),
                expires_at: now.checked_add(PAIRING_SLOT_TTL).expect("expiry"),
                creator_blob: vec![1],
                slot_capability: capability,
                creator_token: creator,
                ticket,
            },
            now,
        )
        .expect("open");
    assert_eq!(
        relay.handle(
            RelayRequest::Join {
                operation_id: operation(6),
                code: code.clone(),
                joiner_blob: vec![2],
                joiner_token: RelaySideToken(OpaqueId::from_u128(43)),
                ticket: Some(RelayJoinTicket([8; 16])),
            },
            now,
        ),
        Err(RelayProtocolError::Unauthorized)
    );
    let _ = relay
        .handle(
            RelayRequest::Join {
                operation_id: operation(7),
                code,
                joiner_blob: vec![2],
                joiner_token: RelaySideToken(OpaqueId::from_u128(44)),
                ticket: None,
            },
            now,
        )
        .expect("manual fallback");
}

#[test]
fn mutation_replay_is_idempotent_and_poll_requires_ack() {
    let now = Timestamp::UNIX_EPOCH;
    let code = RelayCode::new("REPLAY").expect("code");
    let creator = RelaySideToken(OpaqueId::from_u128(101));
    let joiner = RelaySideToken(OpaqueId::from_u128(102));
    let capability = RelaySlotCapability(OpaqueId::from_u128(103));
    let mut relay = RelayBroker::default();
    let open = RelayRequest::Open {
        operation_id: operation(104),
        code: code.clone(),
        expires_at: now.checked_add(PAIRING_SLOT_TTL).expect("expiry"),
        creator_blob: vec![1; 32],
        slot_capability: capability,
        creator_token: creator,
        ticket: RelayJoinTicket([0; 16]),
    };
    let first = relay.handle(open.clone(), now).expect("open");
    assert_eq!(relay.handle(open, now).expect("replay open"), first);
    let slot_id = match first {
        RelayResponse::Opened { slot_id, .. } => slot_id,
        other => panic!("unexpected response: {other:?}"),
    };
    let join = RelayRequest::Join {
        operation_id: operation(105),
        code,
        joiner_blob: vec![2; 32],
        joiner_token: joiner,
        ticket: None,
    };
    let join_response = relay.handle(join.clone(), now).expect("join");
    assert_eq!(relay.handle(join, now).expect("replay join"), join_response);

    let poll = RelayRequest::Poll { slot_id, token: creator, after: RelaySequence(0) };
    let first_poll = relay.handle(poll.clone(), now).expect("poll");
    assert_eq!(relay.handle(poll, now).expect("replayed poll"), first_poll);
    assert!(matches!(&first_poll, RelayResponse::Deliveries(items) if items.len() == 1));
    assert_eq!(
        relay.handle(RelayRequest::Ack { slot_id, token: creator, up_to: RelaySequence(1) }, now,),
        Ok(RelayResponse::Acked(RelaySequence(1)))
    );
    assert_eq!(
        relay.handle(RelayRequest::Poll { slot_id, token: creator, after: RelaySequence(1) }, now,),
        Ok(RelayResponse::Deliveries(Vec::new()))
    );

    let push = RelayRequest::Push {
        operation_id: operation(106),
        message_id: RelayMessageId(OpaqueId::from_u128(107)),
        slot_id,
        token: creator,
        blob: vec![9],
    };
    assert_eq!(relay.handle(push.clone(), now), Ok(RelayResponse::Accepted));
    assert_eq!(relay.handle(push, now), Ok(RelayResponse::Accepted));
    assert!(matches!(
        relay.handle(
            RelayRequest::Poll { slot_id, token: joiner, after: RelaySequence(0) },
            now,
        ),
        Ok(RelayResponse::Deliveries(items)) if items.len() == 1
    ));
}
