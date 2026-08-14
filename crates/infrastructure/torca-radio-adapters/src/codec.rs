//! G.711 μ-law codec and small deterministic voice DSP helpers.

/// Encodes signed linear PCM into the G.711 μ-law representation.
pub fn encode_mulaw(sample: i16) -> u8 {
    const BIAS: i32 = 0x84;
    const CLIP: i32 = 32635;
    let sample = i32::from(sample);
    let (mask, magnitude) =
        if sample < 0 { (0x7f_u8, (-sample).min(CLIP)) } else { (0xff_u8, sample.min(CLIP)) };
    let biased = magnitude + BIAS;
    let exponent = exponent(biased);
    let mantissa = u8::try_from((biased >> (exponent + 3)) & 0x0f).unwrap_or_default();
    mask ^ (exponent << 4 | mantissa)
}

/// Decodes one G.711 μ-law byte into signed linear PCM.
pub const fn decode_mulaw(value: u8) -> i16 {
    let value = !value;
    let sign = value & 0x80;
    let exponent = (value >> 4) & 0x07;
    let mantissa = value & 0x0f;
    let magnitude = (((mantissa as i32) << 3) + 0x84) << exponent;
    let sample = magnitude - 0x84;
    if sign != 0 { -(sample as i16) } else { sample as i16 }
}

const fn exponent(value: i32) -> u8 {
    if value >= 0x4000 {
        7
    } else if value >= 0x2000 {
        6
    } else if value >= 0x1000 {
        5
    } else if value >= 0x0800 {
        4
    } else if value >= 0x0400 {
        3
    } else if value >= 0x0200 {
        2
    } else if value >= 0x0100 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_mulaw, encode_mulaw};

    #[test]
    fn silence_matches_the_standard_idle_code() {
        assert_eq!(encode_mulaw(0), 0xff);
        assert_eq!(decode_mulaw(0xff), 0);
    }

    #[test]
    fn codec_is_monotonic_and_bounded_for_voice_samples() {
        for sample in [-30_000_i16, -10_000, -1_000, 0, 1_000, 10_000, 30_000] {
            let decoded = decode_mulaw(encode_mulaw(sample));
            assert_eq!(decoded.signum(), sample.signum());
            assert!(i32::from(decoded).abs() <= 32_124);
        }
    }

    #[test]
    fn all_codewords_decode_without_panicking() {
        for value in 0_u8..=u8::MAX {
            let _ = decode_mulaw(value);
        }
    }
}
