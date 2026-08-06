# Peer sessions — Batch 13

`torca-peer` owns transport-independent session state:

- disconnected, connecting, handshaking, ready, reconnecting, closed and failed states;
- handshake validation before application data is accepted;
- pending encrypted envelopes retained until protocol acknowledgement;
- resend after reconnect without creating new envelope IDs;
- explicit rejection and transport failure separation;
- in-memory queue transport for integration tests.
