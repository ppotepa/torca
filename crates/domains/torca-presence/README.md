# torca-presence

## Purpose

Derive user-facing availability from multiple independent signals without turning network connectivity into a misleading single boolean.

## Owns

- peer connectivity observation;
- endpoint capability observation;
- last known activity metadata;
- conversation-focus observation where explicitly available;
- derived availability categories and freshness rules.

## Does not own

Peer sockets, contact trust, embedded Tor management, OS background state or UI animation.

## Key rule

`peer_connected`, `endpoint_available`, `conversation_active` and `last_activity_at` remain separate facts. The projection may derive a label, but storage and domain logic must not collapse them.

## 0.1 completion

The contact list can distinguish direct P2P connectivity from generic application activity and stale observations expire predictably.
