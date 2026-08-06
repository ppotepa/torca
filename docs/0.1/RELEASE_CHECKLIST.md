# Torca 0.1 release checklist

A test release may be cut only when every item is evidenced:

- [ ] `./scripts/format.ps1` leaves the tree formatted.
- [ ] `./scripts/validate.ps1` passes on a clean checkout.
- [ ] GAP-001 production crypto is closed with vectors and review.
- [ ] GAP-002 concrete SQLCipher backend is closed with restart/migration tests.
- [ ] Windows native runner and bridge DLL build successfully.
- [ ] Android runner and all target ABI libraries build successfully.
- [ ] Platform test matrix is completed.
- [ ] Direct Tor exchange succeeds without relay participation after pairing.
- [ ] Diagnostic export is reviewed for secret leakage.
- [ ] Threat model and known limitations are accepted.
- [ ] Artifacts are generated with `scripts/package.ps1` and checksums verified.
