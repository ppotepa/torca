use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct SharedTorEndpoint {
    inner: Arc<RwLock<Option<String>>>,
}

impl SharedTorEndpoint {
    pub fn get(&self) -> Option<String> {
        self.inner.read().ok().and_then(|value| value.clone())
    }

    pub(super) fn set(&self, value: Option<String>) {
        if let Ok(mut endpoint) = self.inner.write() {
            *endpoint = value;
        }
    }
}
