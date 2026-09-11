# Event format

The Rust wire types and their validation implement `skopos.event.v1`.
Fields accepting null preserve missing information; unknown fields are rejected,
including raw content. Each JSONL line contains one event. [§FS-events](../requirements.md#fs-events-versioned-observations)

For example, an adapter that recorded a tool's exact source slice may emit:

```json
{
  "schema": "skopos.event.v1",
  "event_id": "producer-unique-observation-id",
  "run_id": "invocation-42",
  "task_id": "ticket-17",
  "repository": "example/project",
  "worktree": "/workspace/project",
  "revision": "historical-revision",
  "agent_id": "worker-1",
  "session_id": "session-7",
  "model": "provider:model",
  "epoch": 0,
  "sequence": 1,
  "step": 1,
  "timestamp": "2026-09-11T10:00:00Z",
  "data": {
    "kind": "read",
    "path": "docs/spec.md",
    "requested": {"start": 4, "end": 8},
    "delivered": {"start": 4, "end": 8},
    "grund": {"id": "FS-example", "section": "2"},
    "bytes": 120,
    "output_sha256": null,
    "estimated_tokens": null,
    "provenance": {
      "method": "runtime",
      "exposure": "tool_returned",
      "confidence": "recorded"
    },
    "truncated": false
  }
}
```

Flatten the example to one line before importing. Null fingerprints are legal
but cannot establish identical returned-text candidates. Ground references are
opaque historical identities; Skopos does not resolve the illustrative reference
against a live repository. Requested ranges may have a null end; delivered ranges
are one-based and inclusive with a known end. [§FS-events.2](../requirements.md#2-reads-and-edits) [§FS-analysis.1](../requirements.md#1-per-run-reports)

Other `data` variants are:

```json
{"kind":"edit","path":"src/lib.rs"}
{"kind":"context_reset"}
{"kind":"run_end","outcome":"unknown"}
{"kind":"import_summary","selected_entries":10,"recognized_reads":3,"unsupported_tool_results":2,"failed_tool_results":1}
```

These data objects still require the complete outer event envelope. Increase
sequence for every event within an agent/session. Concurrent tool calls share
a step. Increase epoch on `context_reset`; completion comes last and seals the
whole run. Terminal outcomes are caller assertions. [§FS-events.1](../requirements.md#1-identity-and-scope) [§FS-events.3](../requirements.md#3-completion-and-coverage)

The current Pi adapter emits `native_transcript` / `tool_returned` / `recorded`.
It hashes the UTF-8 bytes of a single returned text block, not the underlying
source blob. A custom transformed response may therefore have a different
fingerprint from its source. Unknown truncation and delivered ranges remain null.
No local source file is opened to fill the gaps. [§FS-pi.2](../requirements.md#2-tool-results)
