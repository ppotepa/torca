use std::collections::VecDeque;
use std::time::Duration;

use torca_relay_protocol::{RelayRequest, RelayResponse};

use crate::{RelayTransportError, RelayTransportFailureKind};

#[derive(Clone, Debug, Default)]
pub struct ScriptedRelayTransport {
    connected: bool,
    responses: VecDeque<Result<RelayResponse, RelayTransportError>>,
    requests: Vec<RelayRequest>,
}

impl ScriptedRelayTransport {
    pub fn push_response(&mut self, response: Result<RelayResponse, RelayTransportError>) {
        self.responses.push_back(response);
    }

    pub fn requests(&self) -> &[RelayRequest] {
        &self.requests
    }
}

impl crate::PairingServiceTransport for ScriptedRelayTransport {
    fn invalidate(&mut self) {
        self.connected = false;
    }

    fn reconnect(&mut self) -> Result<(), RelayTransportError> {
        self.connected = true;
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &RelayRequest,
        _timeout: Duration,
    ) -> Result<RelayResponse, RelayTransportError> {
        if !self.connected {
            return Err(RelayTransportError {
                kind: RelayTransportFailureKind::Disconnected,
                request_was_sent: false,
            });
        }
        self.requests.push(request.clone());
        self.responses.pop_front().unwrap_or_else(|| {
            Err(RelayTransportError {
                kind: RelayTransportFailureKind::Unavailable,
                request_was_sent: false,
            })
        })
    }
}
