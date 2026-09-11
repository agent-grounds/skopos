# Skopos

An experimental, local profiler of how software agents acquire project knowledge.
Skopos imports observations, preserves their provenance, and reports the files,
ranges, rereads, and associations worth investigating. It does not yet demonstrate
that any proposed memory optimization improves task outcomes. [§GRUND-skopos](docs/grund.md#grund-skopos-project-memory-needs-empirical-feedback)

## Try the prototype

Requires Rust 1.85 or newer. SQLite is bundled; no database server or API key is
needed. From a checkout:

```bash
cargo run -- import-pi examples/pi-session.jsonl \
  --repository demo --revision demo-rev --run demo --complete
cargo run -- report demo
cargo run -- related docs/spec.md --repository demo --revision demo-rev
```

The synthetic example produces six reads, one identical reread candidate, one
unsupported shell result, and two context epochs. Its two samples belong to
one task: they are not two independent trials. No synthetic source text or
prompt content enters the ledger. [§FS-pi.2](requirements.md#2-tool-results) [§FS-analysis.1](requirements.md#1-per-run-reports) [§FS-analysis.2](requirements.md#2-associations)

All commands print JSON. The default database is `.skopos/ledger.sqlite3`, ignored
by Git. Use global `--db PATH` to choose another ledger. Reimporting an identical
snapshot is safe and adds no duplicate events. [§FS-ledger.1](requirements.md#1-import-and-persistence) [§FS-ledger.2](requirements.md#2-cli)

## What is implemented

- Transactional import of the closed `skopos.event.v1` JSONL contract.
- A Pi v3 transcript adapter with explicit branch selection, tool-call/result
  pairing, conservative compaction boundaries, and coverage-gap counts.
- `reads`, `report` (also `files`), `compare`, and `status` commands.
- Revision-scoped `related` and `graph` reports: support, distinct-task support,
  conditional probability, Jaccard, lift, and next-recorded-step probability.
- Separate agent/session/model/epoch samples, optional requested/delivered ranges
  and Grund section references, and explicit provenance and missing values.

The behavior is specified in [requirements.md](requirements.md).
[§FS-events](requirements.md#fs-events-versioned-observations) [§FS-pi](requirements.md#fs-pi-import-a-frozen-pi-session-branch) [§FS-ledger](requirements.md#fs-ledger-local-transactional-import) [§FS-analysis](requirements.md#fs-analysis-descriptive-context-profiles)

## Import a real snapshot

```bash
cargo run -- import-pi /path/to/frozen-session.jsonl \
  --repository agent-grounds/example --revision COMMIT_AT_RUN_TIME \
  --run rhei-invocation-id --task rhei-task-id --complete
cargo run -- reads rhei-invocation-id
cargo run -- compare first-run second-run
```

Supply the historical revision, not today's checkout revision. `--leaf ENTRY_ID`
selects a particular branch; otherwise the last entry's ancestry is selected.
Use a new run ID for a different snapshot. Omit `--complete` when the snapshot is
partial; EOF does not establish completion. The importer never follows a parent
session path or opens source files from the current checkout. [§FS-pi.1](requirements.md#1-tree-selection) [§FS-pi.3](requirements.md#3-import-attribution)

For other runtimes, produce normalized [event JSONL](docs/events.md) and use
`skopos import FILE` or `skopos import -`. Existing Rhei IDs can be preserved for
later accounting joins. Native Rhei accounting import is planned, not implemented.
[§FS-events.1](requirements.md#1-identity-and-scope) [§FS-ledger.2](requirements.md#2-cli)

## How to interpret results

A native transcript establishes recorded tool output. It does **not** establish
final model submission, retention after compaction, attention, necessity, or
causal use. Pi imports therefore retain requested ranges but leave exact
delivered source ranges unknown. Auto-loaded instructions, summaries, images,
and arbitrary shell/search output are outside this adapter's read coverage.
[§FS-pi.2](requirements.md#2-tool-results) [§GRUND-skopos.1](docs/grund.md#1-evidence-before-optimization)

Identical reread candidates mean repeated returned text in the same agent
context, with edits and context resets accounted for. They may be necessary.
Reported bytes and optional token estimates are not total provider usage,
billing, or proven savings. Co-reading is neither dependency nor proof that
preloading the pair helps. Minimum support defaults to two samples; inspect
distinct task support and validate on held-out tasks. [§FS-analysis](requirements.md#fs-analysis-descriptive-context-profiles)

The prototype loads the ledger into memory and computes pairwise associations;
it is intended for small evaluation corpora. It has no daemon, live capture,
automatic memory edits, or context injection. Normalized records are retained
until the local database is removed; there is no automatic retention policy.
[§GRUND-skopos.2](docs/grund.md#2-a-prototype-earns-further-scope) [§FS-ledger](requirements.md#fs-ledger-local-transactional-import)

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
grund check
fissile audit
git config core.hooksPath .githooks
```

CI pins Grund 0.13.1 and Fissile 0.9.1; these published versions are the gate.
The optional pre-commit hook runs formatting, grounding, and staged file-size
checks using locally installed tools. Work in branch worktrees. [§FS-development.1](requirements.md#1-gates)

See [architecture](docs/architecture/ledger.md) ([§AR-ledger](docs/architecture/ledger.md#ar-ledger-offline-events-and-derived-reports)), the
[validation roadmap](docs/roadmap.md) ([§RM-validation](docs/roadmap.md#rm-validation-earn-the-next-prototype-stage)), and the
[research rationale](docs/research.md). MIT licensed; the crate is not published.
