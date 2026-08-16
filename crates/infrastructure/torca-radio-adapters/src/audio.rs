use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(target_os = "android")]
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use torca_contacts::ContactId;
use torca_radio::{RadioOperationId, RadioSessionId};
use torca_radio_coordinator::{
    RadioApplicationError, RadioAudioDeviceProjection, RadioAudioPort, RadioAudioProjection,
};
use torca_radio_protocol::{MAX_RADIO_BURST_FRAMES, RADIO_SAMPLES_PER_FRAME};

use crate::codec::{decode_mulaw, encode_mulaw};

pub type AudioFrame = [u8; RADIO_SAMPLES_PER_FRAME];
const INBOUND_AUDIO_QUEUE_FRAMES: usize = 75;

#[cfg(target_os = "android")]
static ANDROID_PIPELINE: OnceLock<Mutex<Option<AudioPipeline>>> = OnceLock::new();

#[cfg(target_os = "android")]
static ANDROID_NATIVE_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "android")]
static ANDROID_ENCODER: OnceLock<Mutex<AndroidCaptureEncoder>> = OnceLock::new();

#[cfg(target_os = "android")]
struct AndroidCaptureEncoder {
    frame: AudioFrame,
    position: usize,
    envelope: f32,
}

#[cfg(target_os = "android")]
impl Default for AndroidCaptureEncoder {
    fn default() -> Self {
        Self { frame: [0xff; RADIO_SAMPLES_PER_FRAME], position: 0, envelope: 0.0 }
    }
}

/// Installs the single runtime audio lane used by the Android JNI capture
/// bridge. The media system owns the actual pipeline; this only retains a
/// bounded clone for native callback delivery.
#[cfg(target_os = "android")]
pub fn install_android_pipeline(pipeline: AudioPipeline) {
    let slot = ANDROID_PIPELINE.get_or_init(|| Mutex::new(None));
    if let Ok(mut current) = slot.lock() {
        *current = Some(pipeline);
    }
    if let Some(encoder) = ANDROID_ENCODER.get() {
        if let Ok(mut encoder) = encoder.lock() {
            *encoder = AndroidCaptureEncoder::default();
        }
    } else {
        let _ = ANDROID_ENCODER.set(Mutex::new(AndroidCaptureEncoder::default()));
    }
}

#[cfg(target_os = "android")]
pub fn set_android_native_capture_active(active: bool) {
    ANDROID_NATIVE_CAPTURE_ACTIVE.store(active, Ordering::Release);
}

#[cfg(target_os = "android")]
fn android_native_capture_active() -> bool {
    ANDROID_NATIVE_CAPTURE_ACTIVE.load(Ordering::Acquire)
}

/// Accepts little-endian mono PCM from Android's AudioRecord and converts it
/// into the exact μ-law frames consumed by the existing radio media worker.
/// Calls are ignored while the Rust coordinator has not granted the floor.
#[cfg(target_os = "android")]
pub fn push_android_pcm(bytes: &[u8]) {
    let Some(slot) = ANDROID_PIPELINE.get() else { return };
    let Ok(pipeline) = slot.lock().map(|current| current.clone()) else { return };
    let Some(pipeline) = pipeline else { return };
    if !pipeline.capture_enabled() {
        return;
    }
    let Some(encoder) = ANDROID_ENCODER.get() else { return };
    let Ok(mut encoder) = encoder.lock() else { return };
    for chunk in bytes.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        let level = f32::from(sample.unsigned_abs()) / f32::from(i16::MAX as u16);
        encoder.envelope = if level > encoder.envelope {
            encoder.envelope * 0.62 + level * 0.38
        } else {
            encoder.envelope * 0.94 + level * 0.06
        };
        pipeline.set_capture_level(encoder.envelope);
        let position = encoder.position;
        encoder.frame[position] = encode_mulaw(sample);
        encoder.position += 1;
        if encoder.position == RADIO_SAMPLES_PER_FRAME {
            let frame = encoder.frame;
            if pipeline.outbound.push(frame).is_err() {
                let _ = pipeline.outbound.pop();
                let _ = pipeline.outbound.push(frame);
            }
            encoder.position = 0;
        }
    }
}

/// Shared fixed-capacity lane between real-time callbacks and the media
/// worker. Neither side can grow memory under a slow Tor circuit.
#[derive(Clone)]
pub struct AudioPipeline {
    outbound: Arc<ArrayQueue<AudioFrame>>,
    inbound: Arc<ArrayQueue<AudioFrame>>,
    capture_enabled: Arc<AtomicBool>,
    capture_level_milli: Arc<AtomicU32>,
    playback_frame_active: Arc<AtomicBool>,
    start_cue_generation: Arc<AtomicU32>,
    start_cue_finished: Arc<AtomicBool>,
    end_cue_requested: Arc<AtomicBool>,
    end_cue_finished: Arc<AtomicBool>,
}

impl Default for AudioPipeline {
    fn default() -> Self {
        Self {
            // A complete ten-second burst is only about 80 KiB after μ-law
            // encoding. Buffering that bounded payload is preferable to
            // discarding speech whenever a Tor circuit briefly slows down.
            outbound: Arc::new(ArrayQueue::new(MAX_RADIO_BURST_FRAMES)),
            inbound: Arc::new(ArrayQueue::new(INBOUND_AUDIO_QUEUE_FRAMES)),
            capture_enabled: Arc::new(AtomicBool::new(false)),
            capture_level_milli: Arc::new(AtomicU32::new(0)),
            playback_frame_active: Arc::new(AtomicBool::new(false)),
            start_cue_generation: Arc::new(AtomicU32::new(1)),
            start_cue_finished: Arc::new(AtomicBool::new(true)),
            end_cue_requested: Arc::new(AtomicBool::new(false)),
            end_cue_finished: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AudioPipeline {
    pub fn take_outbound(&self) -> Option<AudioFrame> {
        self.outbound.pop()
    }

    pub fn outbound_is_empty(&self) -> bool {
        self.outbound.is_empty()
    }

    pub(crate) fn set_capture_enabled(&self, enabled: bool) {
        self.capture_enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.capture_level_milli.store(0, Ordering::Release);
        }
    }

    fn set_capture_level(&self, level: f32) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let milli = (level.clamp(0.0, 1.0) * 1000.0).round() as u32;
        self.capture_level_milli.store(milli.min(1000), Ordering::Release);
    }

    pub fn capture_level_milli(&self) -> u16 {
        u16::try_from(self.capture_level_milli.load(Ordering::Acquire)).unwrap_or(1000).min(1000)
    }

    fn capture_enabled(&self) -> bool {
        self.capture_enabled.load(Ordering::Acquire)
    }

    pub fn push_inbound(&self, frame: AudioFrame) -> bool {
        if self.inbound.push(frame).is_ok() {
            true
        } else {
            let _ = self.inbound.pop();
            self.inbound.push(frame).is_ok()
        }
    }

    pub fn try_push_inbound(&self, frame: AudioFrame) -> bool {
        self.inbound.push(frame).is_ok()
    }

    pub fn inbound_has_capacity(&self) -> bool {
        !self.inbound.is_full()
    }

    pub fn inbound_is_empty(&self) -> bool {
        self.inbound.is_empty() && !self.playback_frame_active.load(Ordering::Acquire)
    }

    pub(crate) fn set_playback_frame_active(&self, active: bool) {
        self.playback_frame_active.store(active, Ordering::Release);
    }

    pub(crate) fn prepare_playback(&self) {
        self.start_cue_generation.fetch_add(1, Ordering::AcqRel);
        self.start_cue_finished.store(false, Ordering::Release);
        self.end_cue_requested.store(false, Ordering::Release);
        self.end_cue_finished.store(false, Ordering::Release);
        self.set_playback_frame_active(false);
    }

    pub(crate) fn request_end_cue(&self) -> bool {
        !self.end_cue_requested.swap(true, Ordering::AcqRel)
    }

    fn start_cue_generation(&self) -> u32 {
        self.start_cue_generation.load(Ordering::Acquire)
    }

    fn mark_start_cue_finished(&self) {
        self.start_cue_finished.store(true, Ordering::Release);
    }

    fn start_cue_finished(&self) -> bool {
        self.start_cue_finished.load(Ordering::Acquire)
    }

    fn end_cue_requested(&self) -> bool {
        self.end_cue_requested.load(Ordering::Acquire)
    }

    fn mark_end_cue_finished(&self) {
        self.end_cue_finished.store(true, Ordering::Release);
        self.set_playback_frame_active(false);
    }

    pub(crate) fn playback_finished_after_end_cue(&self) -> bool {
        self.end_cue_finished.load(Ordering::Acquire) && self.inbound_is_empty()
    }

    pub fn clear(&self) {
        while self.outbound.pop().is_some() {}
        while self.inbound.pop().is_some() {}
        self.set_playback_frame_active(false);
        self.end_cue_requested.store(false, Ordering::Release);
        self.end_cue_finished.store(false, Ordering::Release);
        self.start_cue_finished.store(true, Ordering::Release);
        self.capture_level_milli.store(0, Ordering::Release);
    }
}

/// CPAL-backed input/output owner. Actual stream callbacks are implemented in
/// a target-specific module; the public adapter stays platform-neutral.
pub struct RadioAudioAdapter {
    pipeline: AudioPipeline,
    platform: platform::PlatformAudio,
    capture_burst: Option<RadioOperationId>,
    playback_burst: Option<RadioOperationId>,
    completed_capture_burst: Option<RadioOperationId>,
    completed_playback_burst: Option<RadioOperationId>,
}

impl RadioAudioAdapter {
    pub fn new(pipeline: AudioPipeline) -> Self {
        Self {
            pipeline: pipeline.clone(),
            platform: platform::PlatformAudio::new(pipeline),
            capture_burst: None,
            playback_burst: None,
            completed_capture_burst: None,
            completed_playback_burst: None,
        }
    }
}

impl RadioAudioPort for RadioAudioAdapter {
    fn devices(&self) -> RadioAudioProjection {
        self.platform.devices()
    }

    fn configure_devices(
        &mut self,
        input_device_id: Option<&str>,
        output_device_id: Option<&str>,
    ) -> Result<(), RadioApplicationError> {
        self.platform.configure_devices(input_device_id, output_device_id)
    }

    fn microphone_ready(&self) -> Result<bool, RadioApplicationError> {
        Ok(self.platform.microphone_ready())
    }

    fn capture_level_milli(&self) -> u16 {
        self.pipeline.capture_level_milli()
    }

    fn begin_capture(
        &mut self,
        _contact_id: ContactId,
        _session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError> {
        // Floor/control retransmissions can deliver the same grant more than
        // once. Starting the CPAL stream again would reset the cue generation
        // and replay the start beep. Capture ownership is per burst, so a
        // duplicate begin is intentionally idempotent.
        if self.capture_burst == Some(burst_id) || self.completed_capture_burst == Some(burst_id) {
            return Ok(());
        }
        self.pipeline.clear();
        self.pipeline.prepare_playback();
        // The local operator should hear the same squelch/beep as the peer.
        // Playback is best-effort here: microphone capture must remain usable
        // when a platform has no output device.
        let _ = self.platform.begin_playback();
        // The cue is deliberately completed before opening the microphone.
        // Otherwise the local beep is guaranteed to be captured on speaker
        // devices and sent to the peer as an apparent echo.
        let deadline = std::time::Instant::now() + Duration::from_millis(180);
        while !self.pipeline.start_cue_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        #[cfg(target_os = "android")]
        let result =
            if android_native_capture_active() { Ok(()) } else { self.platform.begin_capture() };
        #[cfg(not(target_os = "android"))]
        let result = self.platform.begin_capture();
        if result.is_ok() {
            self.pipeline.set_capture_enabled(true);
            self.capture_burst = Some(burst_id);
            self.completed_capture_burst = None;
        }
        result
    }

    fn end_capture(&mut self) {
        if self.capture_burst.is_none() {
            return;
        }
        self.completed_capture_burst = self.capture_burst;
        self.capture_burst = None;
        self.pipeline.set_capture_enabled(false);
        self.pipeline.request_end_cue();
        self.platform.end_capture();
    }

    fn begin_playback(
        &mut self,
        _contact_id: ContactId,
        _session_id: RadioSessionId,
        burst_id: RadioOperationId,
    ) -> Result<(), RadioApplicationError> {
        // A retransmitted first audio frame must not restart playback and the
        // squelch cue for the same burst.
        if self.playback_burst == Some(burst_id) || self.completed_playback_burst == Some(burst_id)
        {
            return Ok(());
        }
        self.pipeline.prepare_playback();
        let result = self.platform.begin_playback();
        if result.is_ok() {
            self.playback_burst = Some(burst_id);
            self.completed_playback_burst = None;
        }
        result
    }

    fn end_playback(&mut self) {
        self.completed_playback_burst = self.playback_burst;
        self.playback_burst = None;
        self.platform.end_playback();
        while self.pipeline.inbound.pop().is_some() {}
    }

    fn take_error(&mut self) -> Option<RadioApplicationError> {
        self.platform.take_error()
    }
}

#[cfg(any(target_os = "windows", target_os = "android"))]
mod platform {
    use super::{
        AudioFrame, AudioPipeline, RADIO_SAMPLES_PER_FRAME, RadioApplicationError,
        RadioAudioDeviceProjection, RadioAudioProjection, decode_mulaw, encode_mulaw,
    };
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    };

    pub struct PlatformAudio {
        pipeline: AudioPipeline,
        input: Option<cpal::Stream>,
        output: Option<cpal::Stream>,
        fault: Arc<AtomicU8>,
        selected_input_id: Option<String>,
        selected_output_id: Option<String>,
    }

    impl PlatformAudio {
        pub fn new(pipeline: AudioPipeline) -> Self {
            Self {
                pipeline,
                input: None,
                output: None,
                fault: Arc::new(AtomicU8::new(0)),
                selected_input_id: None,
                selected_output_id: None,
            }
        }

        pub fn devices(&self) -> RadioAudioProjection {
            let host = cpal::default_host();
            let default_input = host
                .default_input_device()
                .and_then(|device| device.id().ok())
                .map(|id| id.to_string());
            let default_output = host
                .default_output_device()
                .and_then(|device| device.id().ok())
                .map(|id| id.to_string());
            let input_devices = host
                .input_devices()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(device_metadata)
                .map(|(id, name)| RadioAudioDeviceProjection {
                    is_default: default_input.as_deref() == Some(id.as_str()),
                    id,
                    name,
                })
                .collect();
            let output_devices = host
                .output_devices()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(device_metadata)
                .map(|(id, name)| RadioAudioDeviceProjection {
                    is_default: default_output.as_deref() == Some(id.as_str()),
                    id,
                    name,
                })
                .collect();
            RadioAudioProjection {
                input_devices,
                output_devices,
                selected_input_id: self.selected_input_id.clone(),
                selected_output_id: self.selected_output_id.clone(),
            }
        }

        pub fn configure_devices(
            &mut self,
            input_device_id: Option<&str>,
            output_device_id: Option<&str>,
        ) -> Result<(), RadioApplicationError> {
            let devices = self.devices();
            if input_device_id
                .is_some_and(|id| !devices.input_devices.iter().any(|item| item.id == id))
            {
                return Err(RadioApplicationError::MicrophoneUnavailable);
            }
            if output_device_id
                .is_some_and(|id| !devices.output_devices.iter().any(|item| item.id == id))
            {
                return Err(RadioApplicationError::AudioOutputUnavailable);
            }
            self.selected_input_id = input_device_id.map(str::to_owned);
            self.selected_output_id = output_device_id.map(str::to_owned);
            Ok(())
        }

        pub fn microphone_ready(&self) -> bool {
            selected_input_device(self.selected_input_id.as_deref()).is_some()
        }

        pub fn begin_capture(&mut self) -> Result<(), RadioApplicationError> {
            self.end_capture();
            let device = selected_input_device(self.selected_input_id.as_deref())
                .ok_or(RadioApplicationError::MicrophoneUnavailable)?;
            let supported = device
                .default_input_config()
                .map_err(|_| RadioApplicationError::MicrophoneUnavailable)?;
            let config = supported.config();
            if config.sample_rate < 8_000 || config.channels == 0 {
                return Err(RadioApplicationError::MicrophoneUnavailable);
            }
            let pipeline = self.pipeline.clone();
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => {
                    build_input_f32(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                cpal::SampleFormat::I16 => {
                    build_input_i16(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                cpal::SampleFormat::U16 => {
                    build_input_u16(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                _ => return Err(RadioApplicationError::MicrophoneUnavailable),
            }?;
            stream.play().map_err(|_| RadioApplicationError::MicrophoneUnavailable)?;
            self.input = Some(stream);
            Ok(())
        }

        pub fn end_capture(&mut self) {
            self.input.take();
        }

        pub fn begin_playback(&mut self) -> Result<(), RadioApplicationError> {
            // Keep one output stream for the lifetime of the active radio
            // session. Replacing it while the previous stream is still
            // draining can overlap callbacks and play start/end cues twice
            // when the user presses PTT again quickly.
            if self.output.is_some() {
                return Ok(());
            }
            let device = selected_output_device(self.selected_output_id.as_deref())
                .ok_or(RadioApplicationError::AudioOutputUnavailable)?;
            let supported = device
                .default_output_config()
                .map_err(|_| RadioApplicationError::AudioOutputUnavailable)?;
            let config = supported.config();
            if config.sample_rate < 8_000 || config.channels == 0 {
                return Err(RadioApplicationError::AudioOutputUnavailable);
            }
            let pipeline = self.pipeline.clone();
            let stream = match supported.sample_format() {
                cpal::SampleFormat::F32 => {
                    build_output_f32(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                cpal::SampleFormat::I16 => {
                    build_output_i16(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                cpal::SampleFormat::U16 => {
                    build_output_u16(&device, &config, pipeline, Arc::clone(&self.fault))
                }
                _ => return Err(RadioApplicationError::AudioOutputUnavailable),
            }?;
            stream.play().map_err(|_| RadioApplicationError::AudioOutputUnavailable)?;
            self.output = Some(stream);
            Ok(())
        }

        pub fn end_playback(&mut self) {
            self.output.take();
        }

        pub fn take_error(&mut self) -> Option<RadioApplicationError> {
            match self.fault.swap(0, Ordering::AcqRel) {
                1 => Some(RadioApplicationError::MicrophoneUnavailable),
                2 => Some(RadioApplicationError::AudioOutputUnavailable),
                _ => None,
            }
        }
    }

    fn selected_input_device(id: Option<&str>) -> Option<cpal::Device> {
        let host = cpal::default_host();
        match id {
            None => host.default_input_device(),
            Some(id) => host
                .input_devices()
                .ok()?
                .find(|device| device.id().ok().is_some_and(|value| value.to_string() == id)),
        }
    }

    fn selected_output_device(id: Option<&str>) -> Option<cpal::Device> {
        let host = cpal::default_host();
        match id {
            None => host.default_output_device(),
            Some(id) => host
                .output_devices()
                .ok()?
                .find(|device| device.id().ok().is_some_and(|value| value.to_string() == id)),
        }
    }

    fn device_metadata(device: cpal::Device) -> Option<(String, String)> {
        let id = device.id().ok()?.to_string();
        let name = device.description().ok()?.name().to_owned();
        Some((id, name))
    }

    struct CaptureResampler {
        source_rate: u32,
        phase: u32,
        accumulator: f32,
        accumulated_samples: u32,
        frame: AudioFrame,
        position: usize,
        envelope: f32,
    }

    impl CaptureResampler {
        fn new(source_rate: u32) -> Self {
            Self {
                source_rate,
                phase: 0,
                accumulator: 0.0,
                accumulated_samples: 0,
                frame: [0xff; RADIO_SAMPLES_PER_FRAME],
                position: 0,
                envelope: 0.0,
            }
        }

        fn push(&mut self, sample: f32, pipeline: &AudioPipeline) {
            self.accumulator += sample;
            self.accumulated_samples = self.accumulated_samples.saturating_add(1);
            self.phase = self.phase.saturating_add(8_000);
            if self.phase < self.source_rate {
                return;
            }
            self.phase -= self.source_rate;
            #[allow(clippy::cast_precision_loss)]
            let average = self.accumulator / self.accumulated_samples.max(1) as f32;
            self.accumulator = 0.0;
            self.accumulated_samples = 0;
            let pcm = (average.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            let level = average.abs().clamp(0.0, 1.0);
            self.envelope = if level > self.envelope {
                self.envelope * 0.62 + level * 0.38
            } else {
                self.envelope * 0.94 + level * 0.06
            };
            pipeline.set_capture_level(self.envelope);
            self.frame[self.position] = encode_mulaw(pcm);
            self.position += 1;
            if self.position == RADIO_SAMPLES_PER_FRAME {
                let frame = self.frame;
                if pipeline.outbound.push(frame).is_err() {
                    let _ = pipeline.outbound.pop();
                    let _ = pipeline.outbound.push(frame);
                }
                self.position = 0;
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) struct PlaybackResampler {
        output_rate: u32,
        phase: u32,
        frame: Option<AudioFrame>,
        position: usize,
        cue_position: u32,
        cue_length: u32,
        end_cue_position: u32,
        end_cue_length: u32,
        noise: u32,
        last_output: f32,
        cue_generation: u32,
    }

    impl PlaybackResampler {
        pub(crate) fn new(output_rate: u32) -> Self {
            Self {
                output_rate,
                phase: 0,
                frame: None,
                position: 0,
                cue_position: 0,
                cue_length: output_rate.saturating_mul(115) / 1_000,
                end_cue_position: 0,
                end_cue_length: 0,
                noise: 0x52_a9_17_4d,
                last_output: 0.0,
                cue_generation: 0,
            }
        }

        pub(crate) fn next(&mut self, input: &AudioPipeline) -> f32 {
            let generation = input.start_cue_generation();
            if generation != self.cue_generation {
                self.cue_generation = generation;
                self.cue_position = 0;
                self.end_cue_position = 0;
                self.end_cue_length = 0;
                self.last_output = 0.0;
            }
            if self.cue_position < self.cue_length {
                let position = self.cue_position;
                self.cue_position = self.cue_position.saturating_add(1);
                let beep_length = self.output_rate.saturating_mul(55) / 1_000;
                let silence_end = self.output_rate.saturating_mul(75) / 1_000;
                if position < beep_length {
                    #[allow(clippy::cast_precision_loss)]
                    let phase =
                        position as f32 * 880.0 * core::f32::consts::TAU / self.output_rate as f32;
                    return phase.sin() * 0.30;
                }
                if position < silence_end {
                    return 0.0;
                }
                // A short deterministic, low-volume squelch tail makes the
                // start of a remote burst recognizable without storing an
                // audio asset or allocating in the real-time callback.
                self.noise = self.noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let sample = f32::from((self.noise >> 16) as i16) / f32::from(i16::MAX);
                return sample * 0.055;
            }
            input.mark_start_cue_finished();
            // Radio is half-duplex. Once local capture owns the floor, do not
            // render remote frames into the speaker while the microphone is
            // open. This is the first line of echo protection on every host.
            if input.capture_enabled() {
                return 0.0;
            }
            if self.frame.is_none() {
                self.frame = input.inbound.pop();
                self.position = 0;
                input.set_playback_frame_active(self.frame.is_some());
            }
            if self.frame.is_none() && input.end_cue_requested() {
                if self.end_cue_length == 0 {
                    self.end_cue_length = self.output_rate.saturating_mul(155) / 1_000;
                }
                if self.end_cue_position < self.end_cue_length {
                    let position = self.end_cue_position;
                    self.end_cue_position = self.end_cue_position.saturating_add(1);
                    let first_end = self.output_rate.saturating_mul(35) / 1_000;
                    let gap_end = self.output_rate.saturating_mul(70) / 1_000;
                    let second_end = self.output_rate.saturating_mul(125) / 1_000;
                    if position < first_end {
                        #[allow(clippy::cast_precision_loss)]
                        let phase = position as f32 * 950.0 * core::f32::consts::TAU
                            / self.output_rate as f32;
                        return phase.sin() * 0.28;
                    }
                    if position < gap_end {
                        return 0.0;
                    }
                    if position < second_end {
                        #[allow(clippy::cast_precision_loss)]
                        let phase = (position - gap_end) as f32 * 720.0 * core::f32::consts::TAU
                            / self.output_rate as f32;
                        return phase.sin() * 0.26;
                    }
                    self.noise = self.noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let sample = f32::from((self.noise >> 16) as i16) / f32::from(i16::MAX);
                    return sample * 0.035;
                }
                input.mark_end_cue_finished();
            }
            let sample = self.frame.as_ref().map_or(0, |frame| decode_mulaw(frame[self.position]));
            self.phase = self.phase.saturating_add(8_000);
            if self.phase >= self.output_rate {
                self.phase -= self.output_rate;
                self.position += 1;
                if self.position == RADIO_SAMPLES_PER_FRAME {
                    self.frame = input.inbound.pop();
                    self.position = 0;
                    input.set_playback_frame_active(self.frame.is_some());
                }
            }
            let current = f32::from(sample) / f32::from(i16::MAX);
            // A very small one-pole smoother removes the staircase/clicks
            // caused by holding an 8 kHz sample for several hardware frames.
            // It is allocation-free and runs inside the real-time callback.
            let output = self.last_output * 0.18 + current * 0.82;
            self.last_output = output;
            output
        }
    }

    fn input_error(fault: Arc<AtomicU8>, _error: cpal::StreamError) {
        fault.store(1, Ordering::Release);
    }
    fn output_error(fault: Arc<AtomicU8>, _error: cpal::StreamError) {
        fault.store(2, Ordering::Release);
    }

    fn build_input_f32(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let channel_count = f32::from(config.channels);
        let mut state = CaptureResampler::new(config.sample_rate);
        device
            .build_input_stream(
                config,
                move |data: &[f32], _| {
                    if !pipeline.capture_enabled() {
                        return;
                    }
                    for values in data.chunks_exact(channels) {
                        let sample = values.iter().copied().sum::<f32>() / channel_count;
                        state.push(sample, &pipeline);
                    }
                },
                move |error| input_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::MicrophoneUnavailable)
    }

    fn build_input_i16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let channel_count = f32::from(config.channels);
        let mut state = CaptureResampler::new(config.sample_rate);
        device
            .build_input_stream(
                config,
                move |data: &[i16], _| {
                    if !pipeline.capture_enabled() {
                        return;
                    }
                    for values in data.chunks_exact(channels) {
                        let sum = values.iter().map(|value| f32::from(*value)).sum::<f32>();
                        state.push(sum / channel_count / f32::from(i16::MAX), &pipeline);
                    }
                },
                move |error| input_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::MicrophoneUnavailable)
    }

    fn build_input_u16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let channel_count = f32::from(config.channels);
        let mut state = CaptureResampler::new(config.sample_rate);
        device
            .build_input_stream(
                config,
                move |data: &[u16], _| {
                    if !pipeline.capture_enabled() {
                        return;
                    }
                    for values in data.chunks_exact(channels) {
                        let sum = values.iter().map(|value| f32::from(*value)).sum::<f32>();
                        let average = sum / channel_count;
                        state.push((average - 32_768.0) / 32_768.0, &pipeline);
                    }
                },
                move |error| input_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::MicrophoneUnavailable)
    }

    fn build_output_f32(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let mut state = PlaybackResampler::new(config.sample_rate);
        device
            .build_output_stream(
                config,
                move |data: &mut [f32], _| {
                    for values in data.chunks_mut(channels) {
                        let sample = state.next(&pipeline);
                        values.fill(sample);
                    }
                },
                move |error| output_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::AudioOutputUnavailable)
    }

    fn build_output_i16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let mut state = PlaybackResampler::new(config.sample_rate);
        device
            .build_output_stream(
                config,
                move |data: &mut [i16], _| {
                    for values in data.chunks_mut(channels) {
                        let sample = (state.next(&pipeline) * f32::from(i16::MAX)) as i16;
                        values.fill(sample);
                    }
                },
                move |error| output_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::AudioOutputUnavailable)
    }

    fn build_output_u16(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        pipeline: AudioPipeline,
        fault: Arc<AtomicU8>,
    ) -> Result<cpal::Stream, RadioApplicationError> {
        let channels = usize::from(config.channels);
        let mut state = PlaybackResampler::new(config.sample_rate);
        device
            .build_output_stream(
                config,
                move |data: &mut [u16], _| {
                    for values in data.chunks_mut(channels) {
                        #[allow(clippy::cast_sign_loss)]
                        let sample = ((state.next(&pipeline) + 1.0) * 32_767.5)
                            .clamp(0.0, f32::from(u16::MAX))
                            as u16;
                        values.fill(sample);
                    }
                },
                move |error| output_error(Arc::clone(&fault), error),
                None,
            )
            .map_err(|_| RadioApplicationError::AudioOutputUnavailable)
    }
}

#[cfg(not(any(target_os = "windows", target_os = "android")))]
mod platform {
    use super::{AudioPipeline, RadioApplicationError, RadioAudioProjection};

    pub struct PlatformAudio;

    impl PlatformAudio {
        pub const fn new(_pipeline: AudioPipeline) -> Self {
            Self
        }
        pub fn devices(&self) -> RadioAudioProjection {
            RadioAudioProjection::default()
        }
        pub fn configure_devices(
            &mut self,
            _input_device_id: Option<&str>,
            _output_device_id: Option<&str>,
        ) -> Result<(), RadioApplicationError> {
            Ok(())
        }
        pub const fn microphone_ready(&self) -> bool {
            false
        }
        pub fn begin_capture(&mut self) -> Result<(), RadioApplicationError> {
            Err(RadioApplicationError::MicrophoneUnavailable)
        }
        pub fn end_capture(&mut self) {}
        pub fn begin_playback(&mut self) -> Result<(), RadioApplicationError> {
            Err(RadioApplicationError::AudioOutputUnavailable)
        }
        pub fn end_playback(&mut self) {}
        pub fn take_error(&mut self) -> Option<RadioApplicationError> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_queues_drop_old_audio_and_remain_bounded() {
        let pipeline = AudioPipeline::default();
        for value in 0..100_u8 {
            assert!(pipeline.push_inbound([value; RADIO_SAMPLES_PER_FRAME]));
        }
        assert_eq!(pipeline.inbound.len(), INBOUND_AUDIO_QUEUE_FRAMES);
        assert_eq!(pipeline.inbound.pop().expect("oldest retained")[0], 25);
    }

    #[test]
    fn outbound_queue_holds_one_complete_burst() {
        let pipeline = AudioPipeline::default();
        for _ in 0..MAX_RADIO_BURST_FRAMES {
            pipeline.outbound.push([0; RADIO_SAMPLES_PER_FRAME]).expect("bounded burst frame");
        }
        assert_eq!(pipeline.outbound.len(), MAX_RADIO_BURST_FRAMES);
        assert!(pipeline.outbound.push([0; RADIO_SAMPLES_PER_FRAME]).is_err());
    }

    #[test]
    fn end_cue_request_is_idempotent() {
        let pipeline = AudioPipeline::default();
        assert!(pipeline.request_end_cue());
        assert!(!pipeline.request_end_cue());
    }

    #[cfg(any(target_os = "windows", target_os = "android"))]
    #[test]
    fn playback_resampler_renders_start_and_end_cues() {
        let pipeline = AudioPipeline::default();
        let mut resampler = platform::PlaybackResampler::new(48_000);

        let start_samples = (0..6_000).map(|_| resampler.next(&pipeline)).collect::<Vec<_>>();
        assert!(start_samples.iter().any(|sample| sample.abs() > 0.1));

        pipeline.request_end_cue();
        let end_samples = (0..8_000).map(|_| resampler.next(&pipeline)).collect::<Vec<_>>();
        assert!(end_samples.iter().any(|sample| sample.abs() > 0.1));
        assert!(pipeline.playback_finished_after_end_cue());
    }

    #[cfg(any(target_os = "windows", target_os = "android"))]
    #[test]
    fn playback_is_silent_while_local_capture_owns_the_floor() {
        let pipeline = AudioPipeline::default();
        let mut resampler = platform::PlaybackResampler::new(48_000);
        pipeline.prepare_playback();

        // Drain the deterministic start cue first; it is intentionally still
        // audible before capture begins.
        for _ in 0..6_000 {
            let _ = resampler.next(&pipeline);
        }
        pipeline.push_inbound([0x7f; RADIO_SAMPLES_PER_FRAME]);
        pipeline.set_capture_enabled(true);

        assert!((0..200).all(|_| resampler.next(&pipeline) == 0.0));
    }
}
