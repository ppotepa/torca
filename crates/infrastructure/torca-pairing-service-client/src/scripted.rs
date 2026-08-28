use std::collections::VecDeque;
use std::time::Duration;

use torca_pairing_service_protocol::{PairingServiceRequest, PairingServiceResponse};

use crate::{PairingServiceTransportError, PairingServiceTransportFailureKind};

#[derive(Clone, Debug, Default)]
pub struct ScriptedPairingServiceTransport {
    connected: bool,
    responses: VecDeque<Result<PairingServiceResponse, PairingServiceTransportError>>,
    requests: Vec<PairingServiceRequest>,
}

impl ScriptedPairingServiceTransport {
    pub fn push_response(
        &mut self,
        response: Result<PairingServiceResponse, PairingServiceTransportError>,
    ) {
        self.responses.push_back(response);
    }

    pub fn requests(&self) -> &[PairingServiceRequest] {
        &self.requests
    }
}

impl crate::PairingServiceTransport for ScriptedPairingServiceTransport {
    fn invalidate(&mut self) {
        self.connected = false;
    }

    fn reconnect(&mut self) -> Result<(), PairingServiceTransportError> {
        self.connected = true;
        Ok(())
    }

    fn exchange(
        &mut self,
        request: &PairingServiceRequest,
        _timeout: Duration,
    ) -> Result<PairingServiceResponse, PairingServiceTransportError> {
        if !self.connected {
            return Err(PairingServiceTransportError {
                kind: PairingServiceTransportFailureKind::Disconnected,
                request_was_sent: false,
            });
        }
        self.requests.push(request.clone());
        self.responses.pop_front().unwrap_or_else(|| {
            Err(PairingServiceTransportError {
                kind: PairingServiceTransportFailureKind::Unavailable,
                request_was_sent: false,
            })
        })
    }
}
