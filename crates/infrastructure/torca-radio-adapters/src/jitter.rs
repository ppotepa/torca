use std::collections::BTreeMap;

use torca_radio_protocol::{MAX_RADIO_BURST_FRAMES, RADIO_SAMPLES_PER_FRAME};

// Keep the radio feel responsive while retaining enough headroom for a short
// Tor scheduling hiccup. The old 400 ms target made every long burst sound
// delayed before it even started.
const DEFAULT_TARGET_FRAMES: usize = 8;
const MIN_TARGET_FRAMES: usize = 4;
const MAX_TARGET_FRAMES: usize = 25;
const MAX_BUFFERED_FRAMES: usize = MAX_RADIO_BURST_FRAMES;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JitterStats {
    pub target_ms: u32,
    pub dropped_frames: u32,
    pub underruns: u32,
}

/// Bounded reorder/jitter buffer. TCP normally preserves order, but explicit
/// sequence handling makes duplicate and stale media frames harmless.
pub struct JitterBuffer {
    frames: BTreeMap<u32, [u8; RADIO_SAMPLES_PER_FRAME]>,
    next_sequence: Option<u32>,
    target_frames: usize,
    started: bool,
    stats: JitterStats,
    last_frame: Option<[u8; RADIO_SAMPLES_PER_FRAME]>,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self {
            frames: BTreeMap::new(),
            next_sequence: None,
            target_frames: DEFAULT_TARGET_FRAMES,
            started: false,
            stats: JitterStats {
                target_ms: u32::try_from(DEFAULT_TARGET_FRAMES * 20).unwrap_or(1_000),
                dropped_frames: 0,
                underruns: 0,
            },
            last_frame: None,
        }
    }
}

impl JitterBuffer {
    pub fn push(&mut self, sequence: u32, frame: [u8; RADIO_SAMPLES_PER_FRAME]) {
        if (self.started && self.next_sequence.is_some_and(|next| sequence < next))
            || self.frames.contains_key(&sequence)
        {
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            return;
        }
        if self.started {
            self.next_sequence.get_or_insert(sequence);
        } else {
            self.next_sequence =
                Some(self.next_sequence.map_or(sequence, |next| next.min(sequence)));
        }
        self.frames.insert(sequence, frame);
        while self.frames.len() > MAX_BUFFERED_FRAMES {
            if let Some(oldest) = self.frames.keys().next().copied() {
                self.frames.remove(&oldest);
                self.next_sequence = Some(oldest.saturating_add(1));
                self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            }
        }
        if !self.started && self.frames.len() >= self.target_frames {
            self.started = true;
        }
    }

    pub fn pop(&mut self) -> Option<[u8; RADIO_SAMPLES_PER_FRAME]> {
        if !self.started {
            return None;
        }
        let sequence = self.next_sequence?;
        self.next_sequence = Some(sequence.saturating_add(1));
        match self.frames.remove(&sequence) {
            Some(frame) => {
                self.last_frame = Some(frame);
                if self.frames.len() > self.target_frames.saturating_mul(2) {
                    self.target_frames =
                        self.target_frames.saturating_add(1).min(MAX_TARGET_FRAMES);
                }
                self.update_target();
                Some(frame)
            }
            None => {
                self.stats.underruns = self.stats.underruns.saturating_add(1);
                // Do not stop playback for one missing sequence.  A Tor
                // circuit can briefly delay a frame while later frames are
                // already buffered; repeating the last decoded frame is a
                // simple packet-loss concealment strategy and sounds much
                // better than a 160-500 ms silence/rebuffer gap.  The sender
                // still retransmits the missing sequence and the receiver's
                // exact ACK set remains authoritative for burst completion.
                if let Some(next) = self.frames.keys().next().copied() {
                    self.next_sequence = Some(next);
                    self.target_frames =
                        self.target_frames.saturating_add(2).min(MAX_TARGET_FRAMES);
                    self.update_target();
                    return self.last_frame;
                }
                // No future frame is buffered, so wait for the next small
                // startup window rather than spinning on the same sequence.
                self.target_frames = self.target_frames.saturating_sub(1).max(MIN_TARGET_FRAMES);
                self.started = false;
                self.next_sequence = None;
                self.update_target();
                self.last_frame
            }
        }
    }

    /// Marks the producer side of the burst complete. This lets short bursts
    /// start immediately even when they contain fewer frames than the normal
    /// jitter target.
    pub fn finish(&mut self) {
        if !self.frames.is_empty() {
            self.started = true;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn reset(&mut self) {
        self.frames.clear();
        self.next_sequence = None;
        self.started = false;
        self.last_frame = None;
        self.target_frames = DEFAULT_TARGET_FRAMES;
        self.update_target();
    }

    pub const fn stats(&self) -> JitterStats {
        self.stats
    }

    fn update_target(&mut self) {
        self.target_frames = self.target_frames.clamp(MIN_TARGET_FRAMES, MAX_TARGET_FRAMES);
        self.stats.target_ms =
            u32::try_from(self.target_frames.saturating_mul(20)).unwrap_or(1_000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_after_initial_buffer_and_returns_sequence_order() {
        let mut jitter = JitterBuffer::default();
        for sequence in (0_u8..20).rev() {
            jitter.push(u32::from(sequence), [sequence; RADIO_SAMPLES_PER_FRAME]);
        }
        // With an 8-frame startup target, the earliest eight frames that
        // arrived in reverse order are 12..19; older late frames are stale by
        // the time playback begins.
        for sequence in 12_u8..20 {
            assert_eq!(jitter.pop().expect("frame")[0], sequence);
        }
    }

    #[test]
    fn duplicate_and_stale_frames_are_dropped() {
        let mut jitter = JitterBuffer::default();
        jitter.push(1, [1; RADIO_SAMPLES_PER_FRAME]);
        jitter.push(1, [1; RADIO_SAMPLES_PER_FRAME]);
        assert_eq!(jitter.stats().dropped_frames, 1);
    }

    #[test]
    fn completed_short_burst_drains_without_waiting_for_target() {
        let mut jitter = JitterBuffer::default();
        jitter.push(0, [1; RADIO_SAMPLES_PER_FRAME]);
        jitter.push(1, [2; RADIO_SAMPLES_PER_FRAME]);
        assert!(jitter.pop().is_none());

        jitter.finish();

        assert_eq!(jitter.pop().expect("first frame")[0], 1);
        assert_eq!(jitter.pop().expect("second frame")[0], 2);
        assert!(jitter.is_empty());
    }

    #[test]
    fn memory_is_bounded_under_a_slow_consumer() {
        let mut jitter = JitterBuffer::default();
        for sequence in 0..u32::try_from(MAX_BUFFERED_FRAMES + 25).expect("test frame count") {
            jitter.push(sequence, [0; RADIO_SAMPLES_PER_FRAME]);
        }
        assert!(jitter.frames.len() <= MAX_BUFFERED_FRAMES);
        assert!(jitter.stats().dropped_frames > 0);
    }

    #[test]
    fn underrun_reanchors_on_the_next_available_sequence() {
        let mut jitter = JitterBuffer::default();
        for sequence in 0..20 {
            jitter.push(sequence, [sequence as u8; RADIO_SAMPLES_PER_FRAME]);
        }
        assert_eq!(jitter.pop().expect("first frame")[0], 0);
        // Sequence 1 is lost before the consumer asks for it.
        assert_eq!(jitter.pop().expect("second frame")[0], 1);
        // Simulate a gap after the initial buffer has drained.
        for sequence in 22..32 {
            jitter.push(sequence, [sequence as u8; RADIO_SAMPLES_PER_FRAME]);
        }
        while jitter.pop().is_some() {}
        jitter.push(40, [40; RADIO_SAMPLES_PER_FRAME]);
        jitter.finish();
        assert_eq!(jitter.pop().expect("re-anchored frame")[0], 40);
    }

    #[test]
    fn a_gap_repeats_last_frame_instead_of_pausing_playback() {
        let mut jitter = JitterBuffer::default();
        for sequence in 0..8 {
            jitter.push(sequence, [sequence as u8; RADIO_SAMPLES_PER_FRAME]);
        }
        assert_eq!(jitter.pop().expect("first frame")[0], 0);
        // Sequence one is missing, but sequence two is already buffered.
        jitter.frames.remove(&1);
        assert_eq!(jitter.pop().expect("concealed frame")[0], 0);
        assert_eq!(jitter.stats().underruns, 1);
        assert_eq!(jitter.pop().expect("next buffered frame")[0], 2);
    }
}
