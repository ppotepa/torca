//! Radio Mode application coordinator and inward-facing ports.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use torca_contacts::ContactId;
use torca_foundation::{OpaqueId, Timestamp};
use torca_radio::{
    RadioChannel, RadioDomainError, RadioFloor, RadioOperationId, RadioPreference, RadioSessionId,
    RadioState, RadioTimelineEvent,
};
use torca_radio_protocol::{RadioControlFrame, SessionCloseReason};

/// Typed error returned by Radio Mode use cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioApplicationError {
    ContactUnavailable,
    MicrophonePermissionDenied,
    MicrophoneUnavailable,
    AudioOutputUnavailable,
    MutualConsentRequired,
    SessionNotReady,
    ChannelBusy,
    BackgroundTransmissionForbidden,
    Persistence,
    ControlTransport,
    MediaTransport,
    Crypto,
}

impl core::fmt::Display for RadioApplicationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RadioApplicationError {}

impl From<RadioDomainError> for RadioApplicationError {
    fn from(value: RadioDomainError) -> Self {
        match value {
            RadioDomainError::MutualConsentRequired => Self::MutualConsentRequired,
            RadioDomainError::ChannelBusy => Self::ChannelBusy,
            RadioDomainError::SessionAlreadyActive
            | RadioDomainError::SessionNotReady
            | RadioDomainError::UnexpectedFloorGrant
            | RadioDomainError::NoActiveBurst => Self::SessionNotReady,
        }
    }
}

/// Durable timeline record committed together with preference transitions.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioTimelineRecord {
    pub event_id: OpaqueId,
    pub contact_id: ContactId,
    pub correlation_id: OpaqueId,
    pub event: RadioTimelineEvent,
}

/// Atomic persistence boundary. Infrastructure stores preferences and events
/// in one transaction, preserving the single-active-contact invariant.
pub trait RadioStateStore: Send {
    fn load_preferences(&self) -> Result<Vec<RadioPreference>, RadioApplicationError>;

    fn load_recent_events(
        &self,
        _limit: usize,
    ) -> Result<Vec<RadioTimelineRecord>, RadioApplicationError> {
        Ok(Vec::new())
    }

    fn commit(
        &mut self,
        preferences: &[RadioPreference],
        events: &[RadioTimelineRecord],
    ) -> Result<(), RadioApplicationError>;
}

/// Existing authenticated peer lane used only for small state/control frames.
pub trait RadioControlPort: Send {
    fn send(
        &mut self,
        contact_id: ContactId,
        frame: RadioControlFrame,
    ) -> Result<(), RadioApplicationError>;

    /// Advances the bounded durable/in-memory control outbox without
    /// blocking a user command on peer reconnection.
    fn maintain(&mut self, _now: Timestamp) -> Result<(), RadioApplicationError> {
        Ok(())
    }

    /// Returns the next retry deadline for queued control frames.
    fn next_maintenance_delay(&self) -> Option<Duration> {
        None
    }
}

/// Dedicated media lane. Implementations own socket workers but not product
/// state transitions.
pub trait RadioMediaPort: Send {
    fn open(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        media_token: [u8; 32],
        initiate_connection: bool,
    ) -> Result<(), RadioApplicationError>;

    fn close(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
    ) -> Result<(), RadioApplicationError>;

    fn request_floor(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError>;

    fn end_burst(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError>;

    fn cancel_floor(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        request_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError>;

    /// Returns one completed media-worker transition without blocking the
    /// application actor.
    fn take_event(&mut self) -> Option<RadioSessionEvent>;

    /// Number of inbound listener wakeups observed by the media executor.
    /// Implementations that do not expose instrumentation return zero.
    fn wake_count(&self) -> u64 {
        0
    }
}

/// Audio device capability and capture/playback boundary.
pub trait RadioAudioPort: Send {
    fn devices(&self) -> RadioAudioProjection {
        RadioAudioProjection::default()
    }
    fn configure_devices(
        &mut self,
        _input_device_id: Option<&str>,
        _output_device_id: Option<&str>,
    ) -> Result<(), RadioApplicationError> {
        Ok(())
    }
    fn microphone_ready(&self) -> Result<bool, RadioApplicationError>;
    fn begin_capture(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError>;
    fn end_capture(&mut self);
    fn begin_playback(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError>;
    fn end_playback(&mut self);
    /// Returns a platform audio fault observed by a real-time callback.
    fn take_error(&mut self) -> Option<RadioApplicationError> {
        None
    }
}

/// Identity facts required to choose one deterministic coordinator.
pub trait RadioPeerDirectory: Send {
    fn local_identity(&self) -> OpaqueId;
    fn remote_identity(&self, contact_id: ContactId) -> Option<OpaqueId>;
    fn contact_available(&self, contact_id: ContactId) -> bool;
}

/// Cryptographically secure identifier/token source retained behind an
/// application port so deterministic tests do not need platform entropy.
pub trait RadioEntropy: Send {
    fn opaque_id(&mut self) -> Result<OpaqueId, RadioApplicationError>;
    fn bytes_16(&mut self) -> Result<[u8; 16], RadioApplicationError>;
    fn bytes_32(&mut self) -> Result<[u8; 32], RadioApplicationError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRadioLifecycle {
    Foreground,
    Background,
    Terminating,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RadioSessionEvent {
    Ready {
        contact_id: ContactId,
        session_id: RadioSessionId,
        at: Timestamp,
    },
    FloorGranted {
        contact_id: ContactId,
        request_id: RadioOperationId,
        burst_id: RadioOperationId,
        at: Timestamp,
    },
    FloorDenied {
        contact_id: ContactId,
        request_id: RadioOperationId,
    },
    RemoteBurstStarted {
        contact_id: ContactId,
        burst_id: RadioOperationId,
        at: Timestamp,
    },
    /// The first authenticated media frame of a remote burst arrived. Playback
    /// starts here rather than on the floor grant so a control-only burst never
    /// produces a misleading remote audio cue.
    RemoteAudioStarted {
        contact_id: ContactId,
        burst_id: RadioOperationId,
    },
    BurstEnded {
        contact_id: ContactId,
    },
    Interrupted {
        contact_id: ContactId,
        at: Timestamp,
    },
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioContactProjection {
    pub contact_id: ContactId,
    pub local_enabled: bool,
    pub remote_state: torca_radio::RemoteRadioState,
    pub state: RadioState,
    pub changed_at: Timestamp,
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadioSessionProjection {
    pub contact_id: ContactId,
    pub session_id: RadioSessionId,
    pub state: RadioState,
    pub floor: RadioFloor,
    pub burst_elapsed_ms: u32,
    pub max_burst_ms: u32,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioAudioDeviceProjection {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[must_use]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RadioAudioProjection {
    pub input_devices: Vec<RadioAudioDeviceProjection>,
    pub output_devices: Vec<RadioAudioDeviceProjection>,
    pub selected_input_id: Option<String>,
    pub selected_output_id: Option<String>,
}

#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadioProjection {
    pub active_contact_id: Option<ContactId>,
    pub contacts: Vec<RadioContactProjection>,
    pub session: Option<RadioSessionProjection>,
    pub timeline: Vec<RadioTimelineRecord>,
    pub audio: RadioAudioProjection,
}

pub struct RadioCoordinator {
    store: Box<dyn RadioStateStore>,
    control: Box<dyn RadioControlPort>,
    media: Box<dyn RadioMediaPort>,
    audio: Box<dyn RadioAudioPort>,
    peers: Box<dyn RadioPeerDirectory>,
    entropy: Box<dyn RadioEntropy>,
    boot_epoch: [u8; 16],
    channels: BTreeMap<ContactId, RadioChannel>,
    active_contact_id: Option<ContactId>,
    foreground: bool,
    recent_events: VecDeque<RadioTimelineRecord>,
    next_state_sync_at_ms: i64,
}

impl RadioCoordinator {
    pub fn restore(
        store: Box<dyn RadioStateStore>,
        control: Box<dyn RadioControlPort>,
        media: Box<dyn RadioMediaPort>,
        audio: Box<dyn RadioAudioPort>,
        peers: Box<dyn RadioPeerDirectory>,
        mut entropy: Box<dyn RadioEntropy>,
    ) -> Result<Self, RadioApplicationError> {
        let preferences = store.load_preferences()?;
        let recent_events = store.load_recent_events(100)?.into();
        let boot_epoch = entropy.bytes_16()?;
        let active_contact_id =
            preferences.iter().find(|value| value.enabled).map(|value| value.contact_id);
        let channels = preferences
            .into_iter()
            .map(|preference| (preference.contact_id, RadioChannel::new(preference)))
            .collect();
        Ok(Self {
            store,
            control,
            media,
            audio,
            peers,
            entropy,
            boot_epoch,
            channels,
            active_contact_id,
            foreground: true,
            recent_events,
            next_state_sync_at_ms: Timestamp::MIN_UNIX_MILLIS,
        })
    }

    pub fn ensure_contact(&mut self, contact_id: ContactId, at: Timestamp) {
        self.channels
            .entry(contact_id)
            .or_insert_with(|| RadioChannel::new(RadioPreference::disabled(contact_id, at)));
    }

    pub fn configure_audio_devices(
        &mut self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), RadioApplicationError> {
        self.audio.configure_devices(input_device_id, output_device_id)
    }

    pub fn set_enabled(
        &mut self,
        contact_id: ContactId,
        enabled: bool,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        if !self.peers.contact_available(contact_id) {
            return Err(RadioApplicationError::ContactUnavailable);
        }
        self.ensure_contact(contact_id, at);
        if enabled && !self.audio.microphone_ready()? {
            return Err(RadioApplicationError::MicrophoneUnavailable);
        }

        let correlation_id = self.entropy.opaque_id()?;
        let mut changed_preferences = Vec::new();
        let mut records = Vec::new();
        let mut sessions_to_close = Vec::new();

        if enabled {
            if let Some(previous) = self.active_contact_id.filter(|value| *value != contact_id) {
                if let Some(channel) = self.channels.get_mut(&previous) {
                    if let Some(session) = channel.session() {
                        sessions_to_close.push((previous, session.id));
                    }
                    if let Some(event) = channel.set_local_enabled(false, at) {
                        changed_preferences.push(channel.preference());
                        records.push(self.record(previous, correlation_id, event)?);
                    }
                }
            }
        }

        let channel =
            self.channels.get_mut(&contact_id).ok_or(RadioApplicationError::ContactUnavailable)?;
        if !enabled {
            if let Some(session) = channel.session() {
                sessions_to_close.push((contact_id, session.id));
            }
        }
        if let Some(event) = channel.set_local_enabled(enabled, at) {
            changed_preferences.push(channel.preference());
            records.push(RadioTimelineRecord {
                event_id: self.entropy.opaque_id()?,
                contact_id,
                correlation_id,
                event,
            });
        }
        if changed_preferences.is_empty() {
            return Ok(());
        }
        self.store.commit(&changed_preferences, &records)?;
        self.remember(&records);
        self.active_contact_id = if enabled {
            Some(contact_id)
        } else if self.active_contact_id == Some(contact_id) {
            None
        } else {
            self.active_contact_id
        };

        for preference in changed_preferences {
            self.send_state(preference)?;
        }
        for (closing_contact, session_id) in sessions_to_close {
            self.close_session(closing_contact, session_id, true)?;
        }
        if enabled {
            self.maybe_start_session(contact_id, at)?;
        }
        Ok(())
    }

    pub fn synchronize_contact(
        &mut self,
        contact_id: ContactId,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        self.ensure_contact(contact_id, at);
        let preference = self
            .channels
            .get(&contact_id)
            .ok_or(RadioApplicationError::ContactUnavailable)?
            .preference();
        self.send_state(preference)
    }

    pub fn receive_control(
        &mut self,
        contact_id: ContactId,
        frame: RadioControlFrame,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        self.ensure_contact(contact_id, at);
        match frame {
            RadioControlFrame::StateSync { boot_epoch, revision, enabled, changed_at_ms } => {
                let observed_at = Timestamp::from_unix_millis(changed_at_ms).unwrap_or(at);
                let previous_session = self
                    .channels
                    .get(&contact_id)
                    .and_then(|channel| channel.session().map(|session| session.id));
                let event = self.channels.get_mut(&contact_id).and_then(|channel| {
                    channel.observe_remote(boot_epoch, revision, enabled, observed_at)
                });
                if let Some(event) = event {
                    let correlation = self.entropy.opaque_id()?;
                    let record = RadioTimelineRecord {
                        event_id: self.entropy.opaque_id()?,
                        contact_id,
                        correlation_id: correlation,
                        event,
                    };
                    self.store.commit(&[], &[record])?;
                    self.remember(&[record]);
                }
                if enabled {
                    self.maybe_start_session(contact_id, at)?;
                } else if let Some(session_id) = previous_session {
                    self.close_session(contact_id, session_id, false)?;
                }
            }
            RadioControlFrame::SessionOpen { session_id, media_token, coordinator_identity } => {
                let remote = self
                    .peers
                    .remote_identity(contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?;
                let expected = self.peers.local_identity().min(remote);
                let mutually_enabled = self
                    .channels
                    .get(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .is_mutually_enabled();
                if coordinator_identity != expected || !mutually_enabled {
                    return Err(RadioApplicationError::MutualConsentRequired);
                }
                let session_id = RadioSessionId::from_opaque(session_id);
                self.channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .begin_connecting()?;
                self.media.open(contact_id, session_id, media_token, false)?;
            }
            RadioControlFrame::SessionClose { session_id, reason: _ } => {
                if self
                    .channels
                    .get(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .session()
                    .is_some_and(|session| session.id.to_opaque() == session_id)
                {
                    let session_id = RadioSessionId::from_opaque(session_id);
                    let _ = self
                        .channels
                        .get_mut(&contact_id)
                        .ok_or(RadioApplicationError::ContactUnavailable)?
                        .interrupt_session(at);
                    self.audio.end_capture();
                    self.audio.end_playback();
                    self.media.close(contact_id, session_id)?;
                }
            }
        }
        Ok(())
    }

    pub fn handle_session_event(
        &mut self,
        event: RadioSessionEvent,
    ) -> Result<(), RadioApplicationError> {
        match event {
            RadioSessionEvent::Ready { contact_id, session_id, at } => {
                let timeline = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .session_ready(session_id, at)?;
                let correlation = self.entropy.opaque_id()?;
                let record = RadioTimelineRecord {
                    event_id: self.entropy.opaque_id()?,
                    contact_id,
                    correlation_id: correlation,
                    event: timeline,
                };
                self.store.commit(&[], &[record])?;
                self.remember(&[record]);
            }
            RadioSessionEvent::FloorGranted { contact_id, request_id, burst_id, at } => {
                let channel = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?;
                channel.grant_local_floor(request_id, burst_id, at)?;
                let session = channel.session().ok_or(RadioApplicationError::SessionNotReady)?;
                if let Err(error) = self.audio.begin_capture(contact_id, session.id, burst_id) {
                    let _ = channel.abort_local_capture();
                    let _ = self.media.end_burst(contact_id, session.id, burst_id);
                    return Err(error);
                }
                channel.capture_started()?;
            }
            RadioSessionEvent::FloorDenied { contact_id, request_id } => {
                self.channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .deny_local_floor(request_id)?;
            }
            RadioSessionEvent::RemoteBurstStarted { contact_id, burst_id, at } => {
                let channel = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?;
                channel.grant_remote_floor(burst_id, at)?;
            }
            RadioSessionEvent::RemoteAudioStarted { contact_id, burst_id } => {
                let channel = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?;
                let session = channel.session().ok_or(RadioApplicationError::SessionNotReady)?;
                if session.floor_operation_id == Some(burst_id)
                    && channel.state() == RadioState::Receiving
                {
                    self.audio.begin_playback(contact_id, session.id, burst_id)?;
                }
            }
            RadioSessionEvent::BurstEnded { contact_id } => {
                let channel = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?;
                match channel.state() {
                    RadioState::Transmitting => self.audio.end_capture(),
                    RadioState::Receiving => self.audio.end_playback(),
                    _ => {}
                }
                channel.end_burst()?;
            }
            RadioSessionEvent::Interrupted { contact_id, at } => {
                self.audio.end_capture();
                self.audio.end_playback();
                if let Some(event) = self
                    .channels
                    .get_mut(&contact_id)
                    .ok_or(RadioApplicationError::ContactUnavailable)?
                    .interrupt_session(at)
                {
                    let correlation = self.entropy.opaque_id()?;
                    let record = RadioTimelineRecord {
                        event_id: self.entropy.opaque_id()?,
                        contact_id,
                        correlation_id: correlation,
                        event,
                    };
                    self.store.commit(&[], &[record])?;
                    self.remember(&[record]);
                }
            }
        }
        Ok(())
    }

    pub fn begin_transmission(
        &mut self,
        contact_id: ContactId,
    ) -> Result<RadioOperationId, RadioApplicationError> {
        if !self.foreground {
            return Err(RadioApplicationError::BackgroundTransmissionForbidden);
        }
        let request_id = RadioOperationId::from_opaque(self.entropy.opaque_id()?);
        let channel =
            self.channels.get_mut(&contact_id).ok_or(RadioApplicationError::ContactUnavailable)?;
        channel.request_local_floor(request_id)?;
        let session = channel.session().ok_or(RadioApplicationError::SessionNotReady)?;
        if let Err(error) = self.media.request_floor(contact_id, session.id, request_id) {
            let _ = channel.deny_local_floor(request_id);
            return Err(error);
        }
        Ok(request_id)
    }

    pub fn end_transmission(&mut self, contact_id: ContactId) -> Result<(), RadioApplicationError> {
        let channel =
            self.channels.get_mut(&contact_id).ok_or(RadioApplicationError::ContactUnavailable)?;
        match channel.state() {
            RadioState::RequestingFloor => {
                let Some(session) = channel.session() else { return Ok(()) };
                let Some(request_id) = channel.pending_floor_request() else { return Ok(()) };
                // A release is a safety operation. The local state must be
                // cancelled even if a concurrently interrupted media stream
                // cannot receive the best-effort cancellation frame.
                let _ = self.media.cancel_floor(contact_id, session.id, request_id);
                channel.cancel_local_floor(request_id)?;
            }
            RadioState::StartingCapture => {
                let Some(session) = channel.session() else { return Ok(()) };
                let Some(burst_id) = session.floor_operation_id else { return Ok(()) };
                self.audio.end_capture();
                let _ = self.media.end_burst(contact_id, session.id, burst_id);
                channel.abort_local_capture()?;
            }
            RadioState::Transmitting => {
                let Some(session) = channel.session() else { return Ok(()) };
                let Some(burst_id) = session.floor_operation_id else { return Ok(()) };
                self.audio.end_capture();
                let _ = self.media.end_burst(contact_id, session.id, burst_id);
                channel.end_burst()?;
            }
            // Releasing after a remote disconnect, a completed burst or a
            // denied floor is normal. Never surface SessionNotReady to users.
            _ => {}
        }
        Ok(())
    }

    pub fn lifecycle(&mut self, lifecycle: HostRadioLifecycle) {
        self.foreground = lifecycle == HostRadioLifecycle::Foreground;
        if !self.foreground {
            self.end_any_local_burst();
        }
        if lifecycle == HostRadioLifecycle::Terminating {
            self.audio.end_capture();
            self.audio.end_playback();
        }
    }

    pub fn maintain(&mut self, now: Timestamp) -> Result<(), RadioApplicationError> {
        let mut first_error = None;
        if let Some(error) = self.audio.take_error() {
            self.end_any_local_burst();
            first_error = Some(error);
        }
        if let Err(error) = self.control.maintain(now) {
            first_error = Some(error);
        }
        if now.to_unix_millis() >= self.next_state_sync_at_ms {
            let preferences: Vec<_> =
                self.channels.values().map(RadioChannel::preference).collect();
            for preference in preferences {
                if let Err(error) = self.send_state(preference)
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
            self.next_state_sync_at_ms = now.to_unix_millis().saturating_add(30_000);
        }
        while let Some(event) = self.media.take_event() {
            if let Err(error) = self.handle_session_event(event)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        let expired = self.channels.iter().find_map(|(contact, channel)| {
            (channel.state() == RadioState::Transmitting && channel.burst_limit_reached(now))
                .then_some(*contact)
        });
        if let Some(contact) = expired {
            if let Err(error) = self.end_transmission(contact)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Exposes only real radio work to the central runtime scheduler. Disabled
    /// channels with an empty control queue do not create an idle wakeup.
    pub fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        let control = self.control.next_maintenance_delay();
        let state_sync =
            self.channels.values().any(|channel| channel.preference().enabled).then(|| {
                let now_ms = now.to_unix_millis();
                let due_ms = self.next_state_sync_at_ms.saturating_sub(now_ms).max(0);
                Duration::from_millis(u64::try_from(due_ms).unwrap_or(u64::MAX))
            });
        [control, state_sync].into_iter().flatten().min()
    }

    pub fn peer_disconnected(
        &mut self,
        contact_id: ContactId,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        self.audio.end_capture();
        self.audio.end_playback();
        let Some(channel) = self.channels.get_mut(&contact_id) else { return Ok(()) };
        if let Some(event) = channel.peer_disconnected(at) {
            let correlation = self.entropy.opaque_id()?;
            let record = RadioTimelineRecord {
                event_id: self.entropy.opaque_id()?,
                contact_id,
                correlation_id: correlation,
                event,
            };
            self.store.commit(&[], &[record])?;
            self.remember(&[record]);
        }
        Ok(())
    }

    pub fn projection(&self, now: Timestamp) -> RadioProjection {
        let contacts = self
            .channels
            .iter()
            .map(|(contact_id, channel)| RadioContactProjection {
                contact_id: *contact_id,
                local_enabled: channel.preference().enabled,
                remote_state: channel.remote_state(),
                state: channel.state(),
                changed_at: channel.preference().changed_at,
            })
            .collect();
        let session = self.channels.iter().find_map(|(contact_id, channel)| {
            let session = channel.session()?;
            let elapsed = session
                .burst_started_at
                .map_or(0, |started| now.to_unix_millis().saturating_sub(started.to_unix_millis()));
            Some(RadioSessionProjection {
                contact_id: *contact_id,
                session_id: session.id,
                state: channel.state(),
                floor: session.floor,
                burst_elapsed_ms: u32::try_from(elapsed.max(0)).unwrap_or(u32::MAX).min(10_000),
                max_burst_ms: 10_000,
            })
        });
        RadioProjection {
            active_contact_id: self.active_contact_id,
            contacts,
            session,
            timeline: self.recent_events.iter().copied().collect(),
            audio: self.audio.devices(),
        }
    }

    fn media_wake_count(&self) -> u64 {
        self.media.wake_count()
    }

    fn record(
        &mut self,
        contact_id: ContactId,
        correlation_id: OpaqueId,
        event: RadioTimelineEvent,
    ) -> Result<RadioTimelineRecord, RadioApplicationError> {
        Ok(RadioTimelineRecord {
            event_id: self.entropy.opaque_id()?,
            contact_id,
            correlation_id,
            event,
        })
    }

    fn remember(&mut self, records: &[RadioTimelineRecord]) {
        const MAX_RECENT_EVENTS: usize = 100;
        for record in records {
            if self.recent_events.iter().any(|value| value.event_id == record.event_id) {
                continue;
            }
            self.recent_events.push_back(*record);
            while self.recent_events.len() > MAX_RECENT_EVENTS {
                self.recent_events.pop_front();
            }
        }
    }

    fn send_state(&mut self, preference: RadioPreference) -> Result<(), RadioApplicationError> {
        self.control.send(
            preference.contact_id,
            RadioControlFrame::StateSync {
                boot_epoch: self.boot_epoch,
                revision: preference.revision,
                enabled: preference.enabled,
                changed_at_ms: preference.changed_at.to_unix_millis(),
            },
        )
    }

    fn maybe_start_session(
        &mut self,
        contact_id: ContactId,
        _at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        let channel =
            self.channels.get_mut(&contact_id).ok_or(RadioApplicationError::ContactUnavailable)?;
        if !channel.is_mutually_enabled() || channel.session().is_some() {
            return Ok(());
        }
        let local = self.peers.local_identity();
        let remote = self
            .peers
            .remote_identity(contact_id)
            .ok_or(RadioApplicationError::ContactUnavailable)?;
        if local > remote {
            return Ok(());
        }
        channel.begin_connecting()?;
        let session_id = RadioSessionId::from_opaque(self.entropy.opaque_id()?);
        let media_token = self.entropy.bytes_32()?;
        self.control.send(
            contact_id,
            RadioControlFrame::SessionOpen {
                session_id: session_id.to_opaque(),
                media_token,
                coordinator_identity: local,
            },
        )?;
        self.media.open(contact_id, session_id, media_token, true)
    }

    fn close_session(
        &mut self,
        contact_id: ContactId,
        session_id: RadioSessionId,
        notify_peer: bool,
    ) -> Result<(), RadioApplicationError> {
        self.audio.end_capture();
        self.audio.end_playback();
        if notify_peer {
            let _ = self.control.send(
                contact_id,
                RadioControlFrame::SessionClose {
                    session_id: session_id.to_opaque(),
                    reason: SessionCloseReason::Disabled,
                },
            );
        }
        self.media.close(contact_id, session_id)
    }

    fn end_any_local_burst(&mut self) {
        let contact = self.channels.iter().find_map(|(contact, channel)| {
            matches!(channel.state(), RadioState::StartingCapture | RadioState::Transmitting)
                .then_some(*contact)
        });
        if let Some(contact) = contact {
            let _ = self.end_transmission(contact);
        }
    }
}

/// Cloneable application handle shared by native commands and the inbound
/// communication adapter. Product state remains serialized by one mutex and
/// never leaks infrastructure types through the boundary.
#[derive(Clone)]
pub struct SharedRadioCoordinator {
    inner: Arc<Mutex<RadioCoordinator>>,
}

impl SharedRadioCoordinator {
    pub fn new(coordinator: RadioCoordinator) -> Self {
        Self { inner: Arc::new(Mutex::new(coordinator)) }
    }

    /// Exposes a disabled channel for known contacts so the UI can render the
    /// opt-in control before a radio preference has ever been saved.
    pub fn ensure_contact(&self, contact_id: ContactId, at: Timestamp) {
        let _ = self.with_mut(|coordinator| {
            coordinator.ensure_contact(contact_id, at);
            Ok(())
        });
    }

    pub fn configure_audio_devices(
        &self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| {
            coordinator.configure_audio_devices(input_device_id, output_device_id)
        })
    }

    pub fn set_enabled(
        &self,
        contact_id: ContactId,
        enabled: bool,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| coordinator.set_enabled(contact_id, enabled, at))
    }

    pub fn begin_transmission(
        &self,
        contact_id: ContactId,
    ) -> Result<RadioOperationId, RadioApplicationError> {
        self.with_mut(|coordinator| coordinator.begin_transmission(contact_id))
    }

    pub fn end_transmission(&self, contact_id: ContactId) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| coordinator.end_transmission(contact_id))
    }

    pub fn receive_control(
        &self,
        contact_id: ContactId,
        frame: RadioControlFrame,
        at: Timestamp,
    ) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| coordinator.receive_control(contact_id, frame, at))
    }

    pub fn lifecycle(&self, lifecycle: HostRadioLifecycle) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| {
            coordinator.lifecycle(lifecycle);
            Ok(())
        })
    }

    pub fn maintain(&self, now: Timestamp) -> Result<(), RadioApplicationError> {
        self.with_mut(|coordinator| coordinator.maintain(now))
    }

    pub fn next_maintenance_delay(&self, now: Timestamp) -> Option<Duration> {
        self.inner.lock().ok()?.next_maintenance_delay(now)
    }

    pub fn projection(&self, now: Timestamp) -> Result<RadioProjection, RadioApplicationError> {
        self.inner
            .lock()
            .map_err(|_| RadioApplicationError::Persistence)
            .map(|coordinator| coordinator.projection(now))
    }

    pub fn media_wake_count(&self) -> u64 {
        self.inner.lock().map(|coordinator| coordinator.media_wake_count()).unwrap_or(0)
    }

    fn with_mut<T>(
        &self,
        operation: impl FnOnce(&mut RadioCoordinator) -> Result<T, RadioApplicationError>,
    ) -> Result<T, RadioApplicationError> {
        let mut coordinator = self.inner.lock().map_err(|_| RadioApplicationError::Persistence)?;
        operation(&mut coordinator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct StateStore {
        preferences: Vec<RadioPreference>,
        commits: Arc<Mutex<Vec<Vec<RadioPreference>>>>,
    }
    impl RadioStateStore for StateStore {
        fn load_preferences(&self) -> Result<Vec<RadioPreference>, RadioApplicationError> {
            Ok(self.preferences.clone())
        }
        fn commit(
            &mut self,
            preferences: &[RadioPreference],
            _events: &[RadioTimelineRecord],
        ) -> Result<(), RadioApplicationError> {
            self.commits.lock().expect("commits").push(preferences.to_vec());
            Ok(())
        }
    }

    #[derive(Default)]
    struct Control {
        frames: Arc<Mutex<Vec<(ContactId, RadioControlFrame)>>>,
    }
    impl RadioControlPort for Control {
        fn send(
            &mut self,
            contact_id: ContactId,
            frame: RadioControlFrame,
        ) -> Result<(), RadioApplicationError> {
            self.frames.lock().expect("frames").push((contact_id, frame));
            Ok(())
        }
    }

    #[derive(Default)]
    struct Media {
        opened: Arc<Mutex<Vec<ContactId>>>,
        closed: Arc<Mutex<Vec<ContactId>>>,
    }
    impl RadioMediaPort for Media {
        fn open(
            &mut self,
            contact_id: ContactId,
            _: RadioSessionId,
            _: [u8; 32],
            _: bool,
        ) -> Result<(), RadioApplicationError> {
            self.opened.lock().expect("opened").push(contact_id);
            Ok(())
        }
        fn close(
            &mut self,
            contact_id: ContactId,
            _: RadioSessionId,
        ) -> Result<(), RadioApplicationError> {
            self.closed.lock().expect("closed").push(contact_id);
            Ok(())
        }
        fn request_floor(
            &mut self,
            _: ContactId,
            _: RadioSessionId,
            _: RadioOperationId,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }
        fn end_burst(
            &mut self,
            _: ContactId,
            _: RadioSessionId,
            _: RadioOperationId,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }

        fn cancel_floor(
            &mut self,
            _contact_id: ContactId,
            _session_id: RadioSessionId,
            _request_id: RadioOperationId,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }
        fn take_event(&mut self) -> Option<RadioSessionEvent> {
            None
        }
    }

    #[derive(Default)]
    struct Audio;
    impl RadioAudioPort for Audio {
        fn microphone_ready(&self) -> Result<bool, RadioApplicationError> {
            Ok(true)
        }
        fn begin_capture(
            &mut self,
            _: ContactId,
            _: RadioSessionId,
            _: RadioOperationId,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }
        fn end_capture(&mut self) {}
        fn begin_playback(
            &mut self,
            _: ContactId,
            _: RadioSessionId,
            _: RadioOperationId,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }
        fn end_playback(&mut self) {}
    }

    struct Peers;
    impl RadioPeerDirectory for Peers {
        fn local_identity(&self) -> OpaqueId {
            OpaqueId::from_u128(1)
        }
        fn remote_identity(&self, contact_id: ContactId) -> Option<OpaqueId> {
            Some(OpaqueId::from_u128(contact_id.to_opaque().to_u128() + 10))
        }
        fn contact_available(&self, _: ContactId) -> bool {
            true
        }
    }

    struct Entropy(u128);
    impl RadioEntropy for Entropy {
        fn opaque_id(&mut self) -> Result<OpaqueId, RadioApplicationError> {
            self.0 += 1;
            Ok(OpaqueId::from_u128(self.0))
        }
        fn bytes_16(&mut self) -> Result<[u8; 16], RadioApplicationError> {
            Ok([5; 16])
        }
        fn bytes_32(&mut self) -> Result<[u8; 32], RadioApplicationError> {
            Ok([6; 32])
        }
    }

    type RecordedFrames = Arc<Mutex<Vec<(ContactId, RadioControlFrame)>>>;
    type RecordedContacts = Arc<Mutex<Vec<ContactId>>>;

    fn coordinator() -> (RadioCoordinator, RecordedFrames, RecordedContacts, RecordedContacts) {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let opened = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(Mutex::new(Vec::new()));
        let coordinator = RadioCoordinator::restore(
            Box::new(StateStore::default()),
            Box::new(Control { frames: Arc::clone(&frames) }),
            Box::new(Media { opened: Arc::clone(&opened), closed: Arc::clone(&closed) }),
            Box::new(Audio),
            Box::new(Peers),
            Box::new(Entropy(100)),
        )
        .expect("coordinator");
        (coordinator, frames, opened, closed)
    }

    fn at(ms: i64) -> Timestamp {
        Timestamp::from_unix_millis(ms).expect("timestamp")
    }

    #[test]
    fn enabling_a_second_contact_atomically_disables_the_first() {
        let (mut coordinator, frames, _, _) = coordinator();
        let first = ContactId::from_u128(1);
        let second = ContactId::from_u128(2);
        coordinator.set_enabled(first, true, at(1)).expect("first");
        coordinator.set_enabled(second, true, at(2)).expect("second");

        let projection = coordinator.projection(at(2));
        assert_eq!(projection.active_contact_id, Some(second));
        assert!(
            !projection
                .contacts
                .iter()
                .find(|value| value.contact_id == first)
                .expect("first")
                .local_enabled
        );
        assert!(frames.lock().expect("frames").iter().any(|(contact, frame)| {
            *contact == first
                && matches!(frame, RadioControlFrame::StateSync { enabled: false, .. })
        }));
    }

    #[test]
    fn mutual_state_starts_one_media_session_without_a_modal() {
        let (mut coordinator, _, opened, _) = coordinator();
        let contact = ContactId::from_u128(1);
        coordinator.set_enabled(contact, true, at(1)).expect("enable");
        coordinator
            .receive_control(
                contact,
                RadioControlFrame::StateSync {
                    boot_epoch: [9; 16],
                    revision: 1,
                    enabled: true,
                    changed_at_ms: 2,
                },
                at(2),
            )
            .expect("remote state");
        assert_eq!(opened.lock().expect("opened").as_slice(), &[contact]);
    }

    #[test]
    fn disabled_radio_has_no_scheduler_deadline() {
        let (mut coordinator, _, _, _) = coordinator();
        let contact = ContactId::from_u128(9);
        coordinator.ensure_contact(contact, at(1));
        assert_eq!(coordinator.next_maintenance_delay(at(1)), None);

        coordinator.set_enabled(contact, true, at(2)).expect("enable");
        assert_eq!(coordinator.next_maintenance_delay(at(2)), Some(Duration::ZERO));
        coordinator.maintain(at(2)).expect("maintenance");
        assert_eq!(coordinator.next_maintenance_delay(at(2)), Some(Duration::from_secs(30)));
    }

    #[test]
    fn background_lifecycle_forbids_transmission() {
        let (mut coordinator, _, _, _) = coordinator();
        let contact = ContactId::from_u128(1);
        coordinator.lifecycle(HostRadioLifecycle::Background);
        assert_eq!(
            coordinator.begin_transmission(contact),
            Err(RadioApplicationError::BackgroundTransmissionForbidden)
        );
    }

    #[test]
    fn releasing_without_a_live_burst_is_idempotent() {
        let (mut coordinator, _, _, _) = coordinator();
        let contact = ContactId::from_u128(1);
        coordinator.set_enabled(contact, true, at(1)).expect("enable");

        coordinator.end_transmission(contact).expect("safe release");
        coordinator.end_transmission(contact).expect("safe repeated release");
    }

    #[test]
    fn disabling_radio_closes_the_session_retained_before_state_change() {
        let (mut coordinator, frames, _, closed) = coordinator();
        let contact = ContactId::from_u128(1);
        coordinator.set_enabled(contact, true, at(1)).expect("enable");
        coordinator
            .receive_control(
                contact,
                RadioControlFrame::StateSync {
                    boot_epoch: [9; 16],
                    revision: 1,
                    enabled: true,
                    changed_at_ms: 2,
                },
                at(2),
            )
            .expect("remote state");
        let session_id = frames
            .lock()
            .expect("frames")
            .iter()
            .find_map(|(_, frame)| match frame {
                RadioControlFrame::SessionOpen { session_id, .. } => Some(*session_id),
                _ => None,
            })
            .expect("session open");
        coordinator
            .handle_session_event(RadioSessionEvent::Ready {
                contact_id: contact,
                session_id: RadioSessionId::from_opaque(session_id),
                at: at(3),
            })
            .expect("ready");

        coordinator.set_enabled(contact, false, at(4)).expect("disable");

        assert_eq!(closed.lock().expect("closed").as_slice(), &[contact]);
        assert!(coordinator.projection(at(4)).session.is_none());
    }
}
