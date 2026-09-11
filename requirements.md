# FS-events: versioned observations

The `skopos.event.v1` JSONL contract records context acquisition without source
contents. Unknown fields and unsupported schemas are errors. [§GOAL-evidence](docs/goals.md#goal-evidence-trustworthy-context-observations)

## 1. Identity and scope

Every event carries a nonempty event ID, run ID, task ID, repository identity,
absolute worktree path, agent ID, session ID, timestamp, positive sequence, and
positive logical step. Revision and model may be unknown. Sequence increases
within each run/agent/session; a step identifies a possibly concurrent tool
batch and never decreases. Epoch is a nonnegative context-generation counter.
All events in a run agree on repository, worktree, revision, and task.

The first epoch may be nonzero. A `context_reset` strictly increases the epoch;
other events retain it. Each agent/session has an independent context. A model
change also separates analytical samples. IDs are caller supplied, permitting
joins to existing Rhei identifiers without reading Rhei's mutable state.

## 2. Reads and edits

A `read` carries a normalized repository-relative path, optional requested and
delivered line ranges (one-based, inclusive), optional Grund ID/section, optional
returned-text byte count, optional SHA-256 fingerprint, optional estimated
tokens, provenance, and a tri-state truncation flag. Unknown is never zero.
Paths must have no leading slash, backslash, empty, `.` or `..` components.
Ranges must be ordered and nonempty. A requested range may have a null end for
an unbounded read; delivered ranges always have a known end. Hashes use 64
lowercase hexadecimal digits.
No content, prompt, raw tool arguments, or arbitrary metadata fields are stored.

Provenance separates method (`runtime`, `native_transcript`, `shell_inference`,
`os_trace`), exposure (`tool_returned`, `model_submitted`, `filesystem_only`),
and confidence (`recorded`, `inferred`). OS traces can only assert inferred
filesystem access; shell inference cannot assert model submission. A truncated
or unknown-truncation read cannot assert an exact delivered range. Filesystem
events cannot assert returned bytes, tokens, fingerprints, or delivered ranges.

An `edit` invalidates previous reread candidates for that path in the same
agent context. Optional Grund identity is supplied by capture, not resolved
against today's potentially changed checkout. Exposure is never called use.

## 3. Completion and coverage

`run_end` seals the run and carries a caller-asserted outcome: `unknown`,
`passed`, `failed`, or `cancelled`. EOF alone does not complete a run. Outcomes
are metadata, not an independently verified assessment of task correctness.
`import_summary` records selected native entries, recognized reads, unsupported
tool results, and failed tool results. Unsupported means unobserved, not unread.

# FS-ledger: local transactional import

SQLite stores normalized events and no raw transcripts. [§GOAL-evidence](docs/goals.md#goal-evidence-trustworthy-context-observations)

## 1. Import and persistence

Validate all input before committing it in one transaction. Malformed input,
conflicting event IDs, inconsistent run metadata, invalid ordering, and events
after a run is sealed reject the whole batch. An identical event ID and record
is an idempotent duplicate. Duplicate sequences within an agent/session are
rejected. Input order supplies sequencing; output follows durable insertion
order. Importing a changed frozen native snapshot under the same run ID is
an error; use a new run ID. An otherwise identical partial snapshot may be
reimported with explicit completion.

The database schema is versioned. Unsupported versions fail without migration
or overwrite. Queries open existing databases read-only, and a missing database
is an actionable error. New databases default to `.skopos/ledger.sqlite3`.

## 2. CLI

`import FILE` accepts normalized JSONL; `import-pi FILE` accepts the native
format below. `-` reads stdin. `reads RUN` emits recorded events;
`files RUN` and `report RUN` produce a run report; `compare A B` returns both
reports and descriptive differences. `status` lists stored runs and completion.
`related PATH` and `graph` emit the associations defined below. All successful
commands emit one JSON object with a versioned `schema`. Errors go to stderr
with nonzero exit. No command changes agent behavior or contacts a network.

# FS-pi: import a frozen Pi session branch

Import Pi version-3 JSONL snapshots with tool-call/result pairing. [§GOAL-evidence](docs/goals.md#goal-evidence-trustworthy-context-observations)

## 1. Tree selection

Require one version-3 session header with an ID and absolute cwd. Select the
ancestry of `--leaf`, or the last tree entry by default. Reject duplicate entry
IDs, missing parents, or cycles. Never union abandoned branches. A referenced
parent session is not opened. A branch summary or compaction starts a new epoch;
this conservative boundary makes no claim that every earlier byte was evicted.
Session control entries can be ignored; they remain part of tree traversal.

## 2. Tool results

Pair successful `read` results with earlier assistant `toolCall` blocks and
their requested path/offset/limit. Only one plain text result block is supported.
Hash and count the actual returned text, including any wrapper or footer. Never
read the current source file to reconstruct an old read. Preserve requested
ranges when bounded; leave delivered ranges unknown. Preserve native truncation
metadata when provided; otherwise truncation is unknown. Capture is recorded
`native_transcript` at `tool_returned` exposure, not confirmed model submission.

Successful paired `edit` and `write` results invalidate prior reads. Failed
results and unsupported tools, lexically external paths, unmatched results, and nontext
reads are counted as coverage gaps and omitted from read associations. In
particular arbitrary `bash` output and images are not parsed as file reads.
Path normalization is lexical; it does not resolve symlinks or verify which
physical file a custom tool opened. Tool calls from the same assistant message share a logical step, so their result
arrival order cannot invent a sequential relationship. Model identity comes
from that assistant message. Compaction resets pending tool calls.

## 3. Import attribution

Require `--repository`, `--run`, and `--revision`, supplied for the historical
snapshot. `--task` defaults to run ID. Use the session header's cwd and session
ID; use `pi` as the agent ID. `--complete` explicitly seals the snapshot with
unknown outcome. Header data, content, summaries, and raw arguments are discarded
after normalization. Store only the fields allowed by the event contract.

# FS-analysis: descriptive context profiles

Analysis generates review candidates, not causal optimization claims.
[§GOAL-evidence](docs/goals.md#goal-evidence-trustworthy-context-observations) [§GOAL-utility](docs/goals.md#goal-utility-useful-decisions-before-a-larger-service)

## 1. Per-run reports

Group files by path. Report total reads, recorded exposures, known returned
bytes, missing byte counts, reported token estimates, missing estimates,
truncated/unknown-truncation reads, and identical reread candidates. A candidate
requires recorded non-filesystem exposure, a fingerprint, and the same path,
delivered-range value, hash, exposure stage, model, agent/session,
and epoch. Edits invalidate matching paths. Unknown delivered range can match
another unknown range with identical returned bytes; this proves text equality,
not source-range equality or completeness. A repeated truncated result is still
identical returned text; its truncation flag remains visible. Store all reads
even when not eligible for analysis.

Expose per-file Grund references, requested/delivered ranges, and provenance
counts. Known-byte totals and estimated-token totals never claim full model
usage, billed cost, or avoidable cost. Import coverage is retained in reports.
Comparisons are descriptive; task difficulty and outcomes may differ.

## 2. Associations

Require explicit repository and revision filters and completed runs. The sample
unit is one run/agent/session/model/epoch containing recorded tool-returned or
model-submitted reads; inferred/filesystem reads are excluded. A file contributes
once per sample. Samples sharing a task are not independent experimental trials;
report distinct task support too. No association implies dependency or necessity.

For each ordered file pair, report sample support, source/target sample counts,
conditional probability, Jaccard, and lift. Require configurable minimum sample
support (default 2). Lift is support * samples / (source count * target count);
it does not establish statistical significance. Next-step support counts samples
where target occurs in the immediately following recorded-read step after source.
Same-step reads never have a direction. Next-step probability divides that support
by source sample count. Steps with no recorded reads are outside this statistic.

`related PATH` selects edges whose source is PATH; `graph` returns all edges.
Sort by support descending, lift descending, then source/target path. Empty
selections return zero samples/edges, without inventing observations. Outcomes
are reported per run but are not used to infer causal effects.

# FS-development: prototype verification

The repository should remain reproducible and grounded as the experiment evolves.
[§GOAL-evidence](docs/goals.md#goal-evidence-trustworthy-context-observations)

## 1. Gates

CI checks formatting, Clippy without warnings, all Rust test targets, Grund, and
Fissile's full-tree audit. Use the checked-in Cargo lockfile. Pin published Grund
0.13.1 and Fissile 0.9.1 for CI; newer local tools do not redefine the gate.
The optional `.githooks/pre-commit` checks formatting, grounding, and staged size
budgets. It does not install tools or modify sources.
