# AR-ledger: offline events and derived reports

The single Rust package keeps transport, storage, and analysis independent.
The library exposes validated events, atomic ledger ingestion, a Pi adapter,
and pure reporting functions; the CLI formats one JSON response per command.
[§FS-ledger.1](../../requirements.md#1-import-and-persistence) [§FS-ledger.2](../../requirements.md#2-cli)

## 1. Module boundaries

`event` owns the wire types, `validation` enforces their invariants, and `store`
owns SQLite transactions and schema versioning. `pi` handles native tree
selection; `pi_tools` translates supported tool results. `report` and `graph`
derive separate views from the same immutable records. `main` only orchestrates
these components. Test native formats using synthetic data, never real prompts
or local session files. [§FS-pi.1](../../requirements.md#1-tree-selection) [§FS-pi.2](../../requirements.md#2-tool-results) [§FS-analysis.1](../../requirements.md#1-per-run-reports) [§FS-analysis.2](../../requirements.md#2-associations)

`feedback` validates a separate agent-reported JSONL contract and its references
to recorded reads. `splits` derives review candidates without altering the event
ledger. Keeping retrospective annotations outside the acquisition event stream
preserves run sealing and distinguishes agent recollection from observation.
[§FS-feedback.2](../../requirements.md#2-evidence-validation) [§FS-feedback.3](../../requirements.md#3-split-candidate-report)
