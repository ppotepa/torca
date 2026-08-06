# Documentation

Torca documentation is separated by purpose so that current delivery work does not blur long-lived architecture rules.

## Active release documentation

[`0.1`](0.1/README.md) contains the exact scope, ordered roadmap, implementation checklist, and completion criteria for the current release.

## Architecture documentation

[`architecture`](architecture/README.md) contains stable rules for domains, dependencies, commands, events, storage, transport, security, testing, and repository layout.

## Architecture decisions

[`decisions`](decisions/README.md) contains accepted Architecture Decision Records (ADRs). ADRs are immutable after acceptance except for small corrections. A later ADR supersedes an earlier one.

## Authority order

When documents disagree, use this order:

1. accepted ADR relevant to the decision;
2. active version-specific document;
3. long-lived architecture document;
4. package README;
5. historical notes.

An inconsistency should be fixed rather than worked around in code.
