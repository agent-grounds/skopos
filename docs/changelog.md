# Changelog

## Unreleased

- `clean.sh` at the repository root gives back the disk a checkout's builds
  took: it runs `cargo clean`, then removes every directory holding a valid
  `CACHEDIR.TAG`, such as a plan's scratch build under `panta/`. It is the clean
  verb `ephor clean` runs at the root of a branch checkout no live run holds.
- End-of-task agent feedback prompt and read-only `split-candidates` report for
  ranked unused-span nominations, distinct-task support, and range evidence.
- Initial offline prototype: normalized and Pi session import, transactional
  ledger, provenance-aware reports, and revision-scoped file associations.
