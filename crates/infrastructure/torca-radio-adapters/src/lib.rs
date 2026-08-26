//! Infrastructure adapters for Radio Mode media and platform audio.

mod audio;
mod codec;
mod jitter;
mod media;

pub use audio::{AudioPipeline, RadioAudioAdapter};
#[cfg(target_os = "android")]
pub use audio::{install_android_pipeline, push_android_pcm, set_android_native_capture_active};
pub use codec::{decode_mulaw, encode_mulaw};
pub use jitter::{JitterBuffer, JitterStats};
pub use media::{
    RadioMediaAdapter, RadioMediaCipher, RadioMediaConnector, RadioMediaDirectory, RadioMediaRoute,
    RadioMediaStream, RadioMediaSystem, RadioMediaSystemFactory,
    UnsupportedRadioMediaSystemFactory,
};
