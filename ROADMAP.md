# Torca roadmap

## Current: 0.2 validation

Torca 0.2 **source implementation is complete**. Source completion is not release completion. The active work is now validation of the existing Windows/Android product rather than adding broad feature surface.

Canonical status: [`0.2_PROGRESS.md`](0.2_PROGRESS.md). Final source findings: [`docs/0.2/FINAL_AUDIT.md`](docs/0.2/FINAL_AUDIT.md).

Required release gates include:

- actual CI execution for Rust, Flutter, Windows and Android jobs;
- clean Windows and Android builds from fresh workspaces;
- two real clients pairing through the configured relay/Tor path;
- bidirectional text, reply, read/delivery receipts and durable retry;
- restart/reconnect/offline behavior;
- attachment send/resume/open/save paths;
- notification/background/tray lifecycle;
- Safety Number verification and identity-change blocking;
- database/path migration from an existing 0.1 installation;
- source-generated contract/ABI consistency.

## Next architecture/security priority

Before Torca is positioned as a mature security-first messenger, evaluate a reviewed standard ratchet/MLS design. 0.2 intentionally does not claim forward secrecy or post-compromise security for message history.

## 0.3 candidates after 0.2 validation

Candidate work, in approximate priority order:

1. reviewed ratchet/MLS message-key evolution;
2. disappearing-message retention policy;
3. ephemeral P2P typing indicators with a privacy toggle;
4. encrypted persistent drafts and improved conversation search/navigation;
5. archive/pin/mute and richer message actions;
6. additional attachment/media UX;
7. Linux production composition after Windows/Android are stable.

Groups, calls/video, cloud backup and multi-device synchronization remain larger product tracks and should not be mixed into reliability fixes.
