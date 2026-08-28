use core::{ptr, slice, str};
use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use torca_contract::{BridgeCommand, CONTRACT_VERSION, generated};
use torca_crypto::{CryptoProvider, RustCryptoProvider};
use torca_logging::Level;
use torca_runtime_policy::RuntimeEventHub;

use crate::native_runtime::{ABI_OK, TorcaRuntime};

const NATIVE_ABI: u16 = 1;
const STORAGE_EPOCH: u16 = 3;
const MAILBOX_CAPACITY: usize = 256;
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const IDEMPOTENCY_MAX_ENTRIES: usize = 1024;
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(15 * 60);
const BUILD_ID: &str = match option_env!("TORCA_BUILD_ID") {
    Some(value) => value,
    None => "dev",
};
const PRODUCT_VERSION: &str = match option_env!("TORCA_PRODUCT_VERSION") {
    Some(value) => value,
    None => "0.2.0-alpha.0",
};
const SOURCE_COMMIT: &str = match option_env!("TORCA_SOURCE_COMMIT") {
    Some(value) => value,
    None => "working-tree",
};
const SOURCE_FINGERPRINT: &str = match option_env!("TORCA_SOURCE_FINGERPRINT") {
    Some(value) => value,
    None => "development",
};
const PROVIDER_ENDPOINT_HASH: Option<&str> = option_env!("TORCA_PROVIDER_ENDPOINT_HASH");
const IROH_PROFILE: Option<&str> = option_env!("TORCA_IROH_PROFILE");

pub(crate) const fn compiled_build_id() -> &'static str {
    BUILD_ID
}

// Keep the C/JNI process surface in one namespace while assigning each source
// file one responsibility. This preserves all exported symbols and private
// access without forcing a public-module hierarchy into the ABI layer.
include!("model.rs");
include!("registry.rs");
include!("actor_state.rs");
include!("abi.rs");
include!("safe_client.rs");
include!("bridge.rs");
include!("tests.rs");
