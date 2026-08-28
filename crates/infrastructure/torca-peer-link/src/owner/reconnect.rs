use std::time::Duration;

use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_foundation::Timestamp;

use crate::PeerLinkError;

const RECONNECT_BASE_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ReconnectReason {
    PreferredDialer,
    Recovery,
    DurableDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReconnectEntry {
    pub(super) failures: u32,
    pub(super) next_attempt_at: Timestamp,
    pub(super) in_progress: bool,
    pub(super) reason: ReconnectReason,
}

impl ReconnectEntry {
    pub(super) const fn new(next_attempt_at: Timestamp, reason: ReconnectReason) -> Self {
        Self { failures: 0, next_attempt_at, in_progress: false, reason }
    }

    pub(super) fn strengthen(&mut self, reason: ReconnectReason) {
        self.reason = self.reason.max(reason);
    }
}

pub(super) fn reconnect_delay(
    random_provider: &mut RustCryptoProvider,
    failures: u32,
) -> Result<Duration, PeerLinkError> {
    let exponent = failures.saturating_sub(1).min(16);
    let base = RECONNECT_BASE_MS.saturating_mul(1_u64 << exponent).min(RECONNECT_MAX_MS);
    let jitter_room = (base / 4).min(RECONNECT_MAX_MS.saturating_sub(base));
    let jitter = if jitter_room == 0 {
        0
    } else {
        let mut random = [0_u8; 8];
        random_provider.fill_random(&mut random).map_err(|_| PeerLinkError::Randomness)?;
        u64::from_le_bytes(random) % (jitter_room + 1)
    };
    Ok(Duration::from_millis(base + jitter))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::peer_recovery_delay;
    use super::super::*;
    use super::*;
    use torca_contacts::{ContactRoute, PeerCredential};
    use torca_identity::{IdentityId, IdentityKey, KeyAlgorithm, KeyId, PublicIdentity};
    use torca_peer_protocol::{HandshakeSigner, HandshakeSigningError};
    use torca_transport_api::{
        EnergyClass, LatencyClass, PeerTransportError, TransportFactoryError,
    };

    #[derive(Clone)]
    struct TestRelationships {
        contact: Contact,
    }

    impl ContactRepository for TestRelationships {
        fn insert(&mut self, _contact: Contact) -> Result<(), ContactError> {
            Err(ContactError::AlreadyExists)
        }

        fn get(&self, id: ContactId) -> Result<Option<Contact>, ContactError> {
            Ok((self.contact.id() == id).then(|| self.contact.clone()))
        }

        fn update(&mut self, contact: Contact) -> Result<(), ContactError> {
            self.contact = contact;
            Ok(())
        }

        fn list(&self) -> Result<Vec<Contact>, ContactError> {
            Ok(vec![self.contact.clone()])
        }
    }

    impl PeerCredentialRepository for TestRelationships {
        fn insert_credential(&mut self, _credential: PeerCredential) -> Result<(), ContactError> {
            Ok(())
        }

        fn credential_for_contact(
            &self,
            _contact_id: ContactId,
        ) -> Result<Option<PeerCredential>, ContactError> {
            Ok(None)
        }
    }

    struct TestSigner;

    impl HandshakeSigner for TestSigner {
        fn sign(&self, _canonical: &[u8]) -> Result<Vec<u8>, HandshakeSigningError> {
            Ok(vec![0; 64])
        }
    }

    struct CountingTransport {
        connected: bool,
    }

    impl PeerTransport for CountingTransport {
        fn connect(&mut self) -> Result<(), PeerTransportError> {
            self.connected = true;
            Ok(())
        }

        fn send(&mut self, _payload: Vec<u8>) -> Result<(), PeerTransportError> {
            if self.connected {
                Ok(())
            } else {
                Err(PeerTransportError("not connected".to_owned()))
            }
        }

        fn try_receive(&mut self) -> Result<Option<Vec<u8>>, PeerTransportError> {
            Ok(None)
        }

        fn close(&mut self) -> Result<(), PeerTransportError> {
            self.connected = false;
            Ok(())
        }
    }

    struct CountingFactory {
        connects: Arc<AtomicUsize>,
    }

    impl PeerTransportFactory for CountingFactory {
        fn provider_id(&self) -> ProviderId {
            ProviderId::new("memory").expect("static provider id")
        }

        fn capabilities(&self) -> TransportCapabilities {
            TransportCapabilities {
                reliable: true,
                ordered: true,
                supports_incoming: true,
                supports_direct_path: true,
                supports_relay_path: false,
                hides_peer_ip: false,
                max_frame_size: 64 * 1024,
                latency: LatencyClass::Interactive,
                energy: EnergyClass::Low,
            }
        }

        fn accept(
            &mut self,
        ) -> Result<Option<Box<dyn PeerTransport + Send>>, TransportFactoryError> {
            Ok(None)
        }

        fn connect(
            &mut self,
            _contact: &Contact,
        ) -> Result<Box<dyn PeerTransport + Send>, TransportFactoryError> {
            self.connects.fetch_add(1, Ordering::Relaxed);
            Ok(Box::new(CountingTransport { connected: false }))
        }

        fn set_waker(
            &self,
            _waker: Arc<dyn Fn() + Send + Sync>,
        ) -> Result<(), TransportFactoryError> {
            Ok(())
        }
    }

    fn test_link(
        local_identity: u128,
        remote_identity: u128,
    ) -> (PeerLink<TestRelationships, TestSigner>, ContactId, Arc<AtomicUsize>) {
        let contact_id = ContactId::from_u128(99);
        let key = IdentityKey::new(KeyId::from_u128(7), KeyAlgorithm::Ed25519, vec![8; 32])
            .expect("valid peer key");
        let contact = Contact::new(
            contact_id,
            PublicIdentity::new(IdentityId::from_u128(remote_identity), key, 0),
            ContactRoute::for_provider_endpoint(OpaqueId::from_u128(10), "memory", vec![1])
                .expect("valid memory route"),
            Timestamp::UNIX_EPOCH,
        );
        let connects = Arc::new(AtomicUsize::new(0));
        let factory = CountingFactory { connects: Arc::clone(&connects) };
        (
            PeerLink::with_transport_factory(
                Box::new(factory),
                TestRelationships { contact },
                TestSigner,
                OpaqueId::from_u128(local_identity),
            ),
            contact_id,
            connects,
        )
    }

    fn due_now() -> Timestamp {
        system_timestamp()
            .expect("current timestamp")
            .checked_add(Duration::from_secs(1))
            .expect("future timestamp")
    }

    #[test]
    fn reconnect_backoff_is_bounded() {
        let mut random = RustCryptoProvider;
        for failures in [1, 2, 3, 8, 32] {
            let delay = reconnect_delay(&mut random, failures).expect("randomness available");
            assert!(delay >= Duration::from_secs(1));
            assert!(delay <= Duration::from_secs(60));
        }
    }

    #[test]
    fn reconnect_reason_can_only_be_strengthened() {
        let mut entry =
            ReconnectEntry::new(Timestamp::UNIX_EPOCH, ReconnectReason::PreferredDialer);
        entry.strengthen(ReconnectReason::Recovery);
        assert_eq!(entry.reason, ReconnectReason::Recovery);
        entry.strengthen(ReconnectReason::PreferredDialer);
        assert_eq!(entry.reason, ReconnectReason::Recovery);
        entry.strengthen(ReconnectReason::DurableDemand);
        entry.strengthen(ReconnectReason::Recovery);
        assert_eq!(entry.reason, ReconnectReason::DurableDemand);
    }

    #[test]
    fn durable_prime_dials_for_both_identity_orders() {
        for (local, remote) in [(1, 2), (2, 1)] {
            let (mut link, contact_id, connects) = test_link(local, remote);
            assert!(link.prime_contact(contact_id).expect("prime durable contact"));
            let report = link.maintenance(&[contact_id], due_now()).expect("run durable reconnect");
            assert_eq!(report.reconnect_started, 1);
            assert_eq!(connects.load(Ordering::Relaxed), 1);
        }
    }

    #[test]
    fn relationship_warmup_only_dials_on_the_preferred_side() {
        let (mut preferred, preferred_contact, preferred_connects) = test_link(1, 2);
        assert_eq!(preferred.prime_connections().expect("prime preferred"), 1);
        let report = preferred
            .maintenance(&[preferred_contact], due_now())
            .expect("run preferred reconnect");
        assert_eq!(report.reconnect_started, 1);
        assert_eq!(preferred_connects.load(Ordering::Relaxed), 1);

        let (mut passive, passive_contact, passive_connects) = test_link(2, 1);
        assert_eq!(passive.prime_connections().expect("prime passive"), 0);
        let report =
            passive.maintenance(&[passive_contact], due_now()).expect("keep passive side idle");
        assert_eq!(report.reconnect_started, 0);
        assert_eq!(passive_connects.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn reconnect_backoff_preserves_durable_demand() {
        let (mut link, contact_id, _) = test_link(2, 1);
        assert!(link.prime_contact(contact_id).expect("prime durable contact"));
        link.schedule_reconnect(contact_id, due_now()).expect("schedule backoff");
        let entry = link.reconnect.get(&contact_id).expect("reconnect entry");
        assert_eq!(entry.reason, ReconnectReason::DurableDemand);
        assert_eq!(entry.failures, 1);

        link.request_reconnect(contact_id, due_now(), ReconnectReason::PreferredDialer);
        assert_eq!(
            link.reconnect.get(&contact_id).expect("strengthened entry").reason,
            ReconnectReason::DurableDemand
        );
    }

    #[test]
    fn peer_recovery_tick_stops_after_bounded_window() {
        let start = Timestamp::UNIX_EPOCH;
        assert_eq!(peer_recovery_delay(Some(start), start), Some(Duration::from_millis(250)));
        let after_window = start.checked_add(Duration::from_secs(31)).expect("valid timestamp");
        assert_eq!(peer_recovery_delay(Some(start), after_window), None);
    }
}
