//! Half-duplex Radio Mode domain policy.
//!
//! This crate owns consent, session and floor invariants. It deliberately has
//! no knowledge of Tor, codecs, audio devices, persistence or presentation.

use core::fmt;

use torca_contacts::ContactId;
use torca_foundation::{OpaqueId, Timestamp};

/// Product limit for one continuous push-to-talk transmission.
pub const MAX_RADIO_BURST_MS: u32 = 10_000;

/// Stable identifier of one ephemeral mutually accepted radio session.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RadioSessionId(OpaqueId);

impl RadioSessionId {
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }

    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}

impl fmt::Display for RadioSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Stable identifier of one floor request or granted burst.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RadioOperationId(OpaqueId);

impl RadioOperationId {
    pub const fn from_opaque(value: OpaqueId) -> Self {
        Self(value)
    }

    pub const fn to_opaque(self) -> OpaqueId {
        self.0
    }
}

/// Durable local user preference for one contact.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioPreference {
    pub contact_id: ContactId,
    pub enabled: bool,
    pub revision: u64,
    pub changed_at: Timestamp,
}

impl RadioPreference {
    pub const fn disabled(contact_id: ContactId, at: Timestamp) -> Self {
        Self { contact_id, enabled: false, revision: 0, changed_at: at }
    }

    pub fn set(&mut self, enabled: bool, at: Timestamp) -> bool {
        if self.enabled == enabled {
            return false;
        }
        self.enabled = enabled;
        self.revision = self.revision.saturating_add(1);
        self.changed_at = at;
        true
    }
}

/// Last authenticated state observed from the remote installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteRadioState {
    Unknown,
    Disabled,
    Enabled,
}

impl RemoteRadioState {
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Effective user-facing state of one radio relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioState {
    Off,
    Available,
    WaitingForPeer,
    Connecting,
    Ready,
    RequestingFloor,
    StartingCapture,
    Transmitting,
    Receiving,
    Reconnecting,
    Unavailable,
}

/// Current owner of the half-duplex channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioFloor {
    None,
    Local,
    Remote,
}

/// Why a previously usable session became unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioInterruption {
    PeerDisconnected,
    MediaDisconnected,
    NetworkChanged,
    ContactUnavailable,
    AudioUnavailable,
}

/// Reason for ending one bounded burst.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioBurstEndReason {
    Released,
    LimitReached,
    Backgrounded,
    SessionInterrupted,
    ContactDisabled,
    AudioUnavailable,
    NetworkTooSlow,
}

/// Immutable timeline facts emitted by application policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioTimelineEventKind {
    Enabled,
    Disabled,
    Ready,
    Interrupted,
    Restored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioEventActor {
    Local,
    Remote,
    System,
}

/// Domain transition emitted to persistence and presentation adapters.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioTimelineEvent {
    pub kind: RadioTimelineEventKind,
    pub actor: RadioEventActor,
    pub occurred_at: Timestamp,
}

/// Live facts for one established media session.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioSession {
    pub id: RadioSessionId,
    pub floor: RadioFloor,
    pub floor_operation_id: Option<RadioOperationId>,
    pub burst_started_at: Option<Timestamp>,
}

impl RadioSession {
    pub const fn new(id: RadioSessionId) -> Self {
        Self { id, floor: RadioFloor::None, floor_operation_id: None, burst_started_at: None }
    }
}

/// Aggregate for one contact's Radio Mode relationship.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioChannel {
    preference: RadioPreference,
    remote_state: RemoteRadioState,
    remote_boot_epoch: Option<[u8; 16]>,
    remote_revision: u64,
    state: RadioState,
    session: Option<RadioSession>,
    pending_floor_request: Option<RadioOperationId>,
}

impl RadioChannel {
    pub const fn new(preference: RadioPreference) -> Self {
        let state = if preference.enabled { RadioState::WaitingForPeer } else { RadioState::Off };
        Self {
            preference,
            remote_state: RemoteRadioState::Unknown,
            remote_boot_epoch: None,
            remote_revision: 0,
            state,
            session: None,
            pending_floor_request: None,
        }
    }

    pub const fn preference(&self) -> RadioPreference {
        self.preference
    }

    pub const fn remote_state(&self) -> RemoteRadioState {
        self.remote_state
    }

    pub const fn state(&self) -> RadioState {
        self.state
    }

    pub const fn session(&self) -> Option<RadioSession> {
        self.session
    }

    pub const fn pending_floor_request(&self) -> Option<RadioOperationId> {
        self.pending_floor_request
    }

    pub const fn is_mutually_enabled(&self) -> bool {
        self.preference.enabled && self.remote_state.is_enabled()
    }

    pub fn set_local_enabled(
        &mut self,
        enabled: bool,
        at: Timestamp,
    ) -> Option<RadioTimelineEvent> {
        if !self.preference.set(enabled, at) {
            return None;
        }
        self.session = None;
        self.pending_floor_request = None;
        self.state = self.derived_idle_state();
        Some(RadioTimelineEvent {
            kind: if enabled {
                RadioTimelineEventKind::Enabled
            } else {
                RadioTimelineEventKind::Disabled
            },
            actor: RadioEventActor::Local,
            occurred_at: at,
        })
    }

    /// Applies an authenticated remote observation. Returns `None` for stale
    /// or duplicate state, making reconnect retries idempotent.
    pub fn observe_remote(
        &mut self,
        boot_epoch: [u8; 16],
        revision: u64,
        enabled: bool,
        at: Timestamp,
    ) -> Option<RadioTimelineEvent> {
        if self.remote_boot_epoch == Some(boot_epoch) && revision <= self.remote_revision {
            return None;
        }
        let next = if enabled { RemoteRadioState::Enabled } else { RemoteRadioState::Disabled };
        let changed = next != self.remote_state;
        self.remote_boot_epoch = Some(boot_epoch);
        self.remote_revision = revision;
        self.remote_state = next;
        if !self.is_mutually_enabled() {
            self.session = None;
            self.pending_floor_request = None;
        }
        self.state = self.derived_idle_state();
        changed.then_some(RadioTimelineEvent {
            kind: if enabled {
                RadioTimelineEventKind::Enabled
            } else {
                RadioTimelineEventKind::Disabled
            },
            actor: RadioEventActor::Remote,
            occurred_at: at,
        })
    }

    /// Forget remote state when a new authenticated peer connection must
    /// establish a fresh synchronization epoch.
    pub fn peer_disconnected(&mut self, at: Timestamp) -> Option<RadioTimelineEvent> {
        self.remote_state = RemoteRadioState::Unknown;
        self.remote_boot_epoch = None;
        self.remote_revision = 0;
        let was_live = self.session.take().is_some();
        self.pending_floor_request = None;
        self.state =
            if self.preference.enabled { RadioState::Reconnecting } else { RadioState::Off };
        was_live.then_some(RadioTimelineEvent {
            kind: RadioTimelineEventKind::Interrupted,
            actor: RadioEventActor::System,
            occurred_at: at,
        })
    }

    pub fn begin_connecting(&mut self) -> Result<(), RadioDomainError> {
        if !self.is_mutually_enabled() {
            return Err(RadioDomainError::MutualConsentRequired);
        }
        if self.session.is_some() {
            return Err(RadioDomainError::SessionAlreadyActive);
        }
        self.state = RadioState::Connecting;
        Ok(())
    }

    pub fn session_ready(
        &mut self,
        id: RadioSessionId,
        at: Timestamp,
    ) -> Result<RadioTimelineEvent, RadioDomainError> {
        if !self.is_mutually_enabled() {
            return Err(RadioDomainError::MutualConsentRequired);
        }
        let restored = matches!(self.state, RadioState::Reconnecting);
        self.session = Some(RadioSession::new(id));
        self.pending_floor_request = None;
        self.state = RadioState::Ready;
        Ok(RadioTimelineEvent {
            kind: if restored {
                RadioTimelineEventKind::Restored
            } else {
                RadioTimelineEventKind::Ready
            },
            actor: RadioEventActor::System,
            occurred_at: at,
        })
    }

    pub fn request_local_floor(
        &mut self,
        request_id: RadioOperationId,
    ) -> Result<(), RadioDomainError> {
        if self.state != RadioState::Ready || self.session.is_none() {
            return Err(RadioDomainError::SessionNotReady);
        }
        self.pending_floor_request = Some(request_id);
        self.state = RadioState::RequestingFloor;
        Ok(())
    }

    pub fn grant_local_floor(
        &mut self,
        request_id: RadioOperationId,
        burst_id: RadioOperationId,
        at: Timestamp,
    ) -> Result<(), RadioDomainError> {
        if self.pending_floor_request != Some(request_id)
            || self.state != RadioState::RequestingFloor
        {
            return Err(RadioDomainError::UnexpectedFloorGrant);
        }
        let session = self.session.as_mut().ok_or(RadioDomainError::SessionNotReady)?;
        session.floor = RadioFloor::Local;
        session.floor_operation_id = Some(burst_id);
        session.burst_started_at = Some(at);
        self.pending_floor_request = None;
        self.state = RadioState::StartingCapture;
        Ok(())
    }

    pub fn capture_started(&mut self) -> Result<(), RadioDomainError> {
        if self.state != RadioState::StartingCapture {
            return Err(RadioDomainError::UnexpectedFloorGrant);
        }
        self.state = RadioState::Transmitting;
        Ok(())
    }

    pub fn abort_local_capture(&mut self) -> Result<(), RadioDomainError> {
        if self.state != RadioState::StartingCapture {
            return Err(RadioDomainError::UnexpectedFloorGrant);
        }
        let session = self.session.as_mut().ok_or(RadioDomainError::SessionNotReady)?;
        session.floor = RadioFloor::None;
        session.floor_operation_id = None;
        session.burst_started_at = None;
        self.state = RadioState::Ready;
        Ok(())
    }

    pub fn grant_remote_floor(
        &mut self,
        burst_id: RadioOperationId,
        at: Timestamp,
    ) -> Result<(), RadioDomainError> {
        if self.state != RadioState::Ready {
            return Err(RadioDomainError::ChannelBusy);
        }
        let session = self.session.as_mut().ok_or(RadioDomainError::SessionNotReady)?;
        session.floor = RadioFloor::Remote;
        session.floor_operation_id = Some(burst_id);
        session.burst_started_at = Some(at);
        self.state = RadioState::Receiving;
        Ok(())
    }

    pub fn deny_local_floor(
        &mut self,
        request_id: RadioOperationId,
    ) -> Result<(), RadioDomainError> {
        if self.pending_floor_request != Some(request_id) {
            return Err(RadioDomainError::UnexpectedFloorGrant);
        }
        self.pending_floor_request = None;
        self.state = RadioState::Ready;
        Ok(())
    }

    pub fn cancel_local_floor(
        &mut self,
        request_id: RadioOperationId,
    ) -> Result<(), RadioDomainError> {
        if self.pending_floor_request != Some(request_id)
            || self.state != RadioState::RequestingFloor
        {
            return Err(RadioDomainError::UnexpectedFloorGrant);
        }
        self.pending_floor_request = None;
        self.state = RadioState::Ready;
        Ok(())
    }

    pub fn end_burst(&mut self) -> Result<(), RadioDomainError> {
        if !matches!(self.state, RadioState::Transmitting | RadioState::Receiving) {
            return Err(RadioDomainError::NoActiveBurst);
        }
        let session = self.session.as_mut().ok_or(RadioDomainError::SessionNotReady)?;
        session.floor = RadioFloor::None;
        session.floor_operation_id = None;
        session.burst_started_at = None;
        self.state = RadioState::Ready;
        Ok(())
    }

    pub fn burst_limit_reached(&self, now: Timestamp) -> bool {
        self.session.and_then(|session| session.burst_started_at).is_some_and(|started| {
            now.to_unix_millis().saturating_sub(started.to_unix_millis())
                >= i64::from(MAX_RADIO_BURST_MS)
        })
    }

    pub fn interrupt_session(&mut self, at: Timestamp) -> Option<RadioTimelineEvent> {
        let existed = self.session.take().is_some();
        self.pending_floor_request = None;
        self.state = if self.is_mutually_enabled() {
            RadioState::Reconnecting
        } else {
            self.derived_idle_state()
        };
        existed.then_some(RadioTimelineEvent {
            kind: RadioTimelineEventKind::Interrupted,
            actor: RadioEventActor::System,
            occurred_at: at,
        })
    }

    const fn derived_idle_state(&self) -> RadioState {
        match (self.preference.enabled, self.remote_state) {
            (false, RemoteRadioState::Enabled) => RadioState::Available,
            (false, _) => RadioState::Off,
            (true, RemoteRadioState::Enabled) => RadioState::Connecting,
            (true, _) => RadioState::WaitingForPeer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioDomainError {
    MutualConsentRequired,
    SessionAlreadyActive,
    SessionNotReady,
    ChannelBusy,
    UnexpectedFloorGrant,
    NoActiveBurst,
}

impl fmt::Display for RadioDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RadioDomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: i64) -> Timestamp {
        Timestamp::from_unix_millis(value).expect("valid timestamp")
    }

    fn channel(enabled: bool) -> RadioChannel {
        RadioChannel::new(RadioPreference {
            contact_id: ContactId::from_u128(1),
            enabled,
            revision: u64::from(enabled),
            changed_at: at(1),
        })
    }

    #[test]
    fn both_sides_must_enable_radio_before_a_session_can_start() {
        let mut radio = channel(true);
        assert_eq!(radio.begin_connecting(), Err(RadioDomainError::MutualConsentRequired));

        radio.observe_remote([7; 16], 1, true, at(2));
        assert!(radio.is_mutually_enabled());
        assert_eq!(radio.state(), RadioState::Connecting);
    }

    #[test]
    fn remote_retries_are_idempotent_per_boot_epoch_and_revision() {
        let mut radio = channel(false);
        let first = radio.observe_remote([3; 16], 4, true, at(10));
        let duplicate = radio.observe_remote([3; 16], 4, false, at(11));

        assert!(first.is_some());
        assert_eq!(duplicate, None);
        assert_eq!(radio.remote_state(), RemoteRadioState::Enabled);
    }

    #[test]
    fn a_new_remote_boot_epoch_resets_revision_ordering() {
        let mut radio = channel(false);
        radio.observe_remote([3; 16], 99, true, at(10));
        radio.observe_remote([4; 16], 1, false, at(11));
        assert_eq!(radio.remote_state(), RemoteRadioState::Disabled);
    }

    #[test]
    fn floor_is_half_duplex_and_bounded_to_ten_seconds() {
        let mut radio = channel(true);
        radio.observe_remote([1; 16], 1, true, at(2));
        let _ = radio
            .session_ready(RadioSessionId::from_opaque(OpaqueId::from_u128(10)), at(3))
            .expect("session ready");
        let request = RadioOperationId::from_opaque(OpaqueId::from_u128(11));
        let burst = RadioOperationId::from_opaque(OpaqueId::from_u128(12));
        radio.request_local_floor(request).expect("request floor");
        radio.grant_local_floor(request, burst, at(100)).expect("grant floor");
        radio.capture_started().expect("capture started");

        assert_eq!(radio.state(), RadioState::Transmitting);
        assert!(!radio.burst_limit_reached(at(10_099)));
        assert!(radio.burst_limit_reached(at(10_100)));
        assert_eq!(
            radio.grant_remote_floor(RadioOperationId::from_opaque(OpaqueId::from_u128(13)), at(4)),
            Err(RadioDomainError::ChannelBusy)
        );
    }

    #[test]
    fn releasing_ptt_before_a_grant_cancels_the_floor_request() {
        let mut radio = channel(true);
        radio.observe_remote([1; 16], 1, true, at(2));
        let _ = radio
            .session_ready(RadioSessionId::from_opaque(OpaqueId::from_u128(10)), at(3))
            .expect("session ready");
        let request = RadioOperationId::from_opaque(OpaqueId::from_u128(11));
        radio.request_local_floor(request).expect("request floor");

        radio.cancel_local_floor(request).expect("cancel floor");

        assert_eq!(radio.state(), RadioState::Ready);
        assert_eq!(radio.pending_floor_request(), None);
    }

    #[test]
    fn disconnect_keeps_local_preference_but_forgets_remote_consent() {
        let mut radio = channel(true);
        radio.observe_remote([1; 16], 1, true, at(2));
        let _ = radio
            .session_ready(RadioSessionId::from_opaque(OpaqueId::from_u128(10)), at(3))
            .expect("session ready");

        let event = radio.peer_disconnected(at(4));
        assert!(radio.preference().enabled);
        assert_eq!(radio.remote_state(), RemoteRadioState::Unknown);
        assert_eq!(radio.state(), RadioState::Reconnecting);
        assert_eq!(event.expect("interruption").kind, RadioTimelineEventKind::Interrupted);
    }
}
