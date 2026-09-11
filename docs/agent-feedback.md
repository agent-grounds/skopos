# Ask agents about unused reads

Agent recollection complements the acquisition log. It is not ground truth about
what affected a model's decisions. Skopos keeps these nominations in a separate
JSONL sidecar and joins them to recorded reads for split review. [§FS-feedback](../requirements.md#fs-feedback-agent-reported-unused-spans)

## End-of-task prompt

Supply the completed run's identity and read-event metadata to the same agent
that did the work. Collect its response before discarding that agent's context.
Do not relaunch a fresh agent and call its reconstruction first-person feedback.
The surrounding workflow collects the response; Skopos does not call a model.
Seal/import the acquisition log before running the join. [§FS-feedback.1](../requirements.md#1-end-of-task-collection)

> Looking back at this task, nominate up to five spans you read but believe did
> not contribute to implementation, diagnosis, verification, understanding, or
> respecting constraints. Rank the strongest opportunities to avoid incidental
> reading first. We will use these nominations to investigate splitting files
> and chapters, not to delete content automatically.
>
> Use only read-event IDs and historical line coordinates you can identify from
> the supplied read metadata and your existing context. Do not reread files just
> to answer. Do not guess line numbers or fill a quota. An empty nominations
> array is valid. A convention, safety rule, or alternative you considered may
> have helped even if absent from the final answer; do not call that unused.
>
> Return one JSON object in the format below. Include no source text or prompt
> quotes. Use a reason and suggested action from the enumerated values. Your
> confidence is about your recollection, not proof of non-use. Rank from 1 with
> no gaps. Copy the supplied run/repository/revision/agent/session identity.

```json
{
  "schema": "skopos.feedback.v1",
  "feedback_id": "run-42-worker-1-feedback",
  "run_id": "run-42",
  "repository": "example/project",
  "revision": "historical-revision",
  "agent_id": "worker-1",
  "session_id": "session-7",
  "timestamp": "2026-09-11T10:30:00Z",
  "nominations": [
    {
      "rank": 1,
      "read_event_id": "read-17",
      "path": "docs/design.md",
      "range": {"start": 120, "end": 175},
      "reason": "unrelated_to_task",
      "confidence": "medium",
      "suggestion": "extract_section"
    }
  ]
}
```

Reasons: `unrelated_to_task`, `excess_detail`, `duplicate_information`.
Suggestions: `extract_file`, `extract_section`, `narrow_read`.
Confidence: `low`, `medium`, `high`. Use `nominations: []` to abstain.
The example IDs must be replaced with actual recorded identities. [§FS-feedback.1](../requirements.md#1-end-of-task-collection)

## Review candidates

Save one response per line in a local, private sidecar, then run:

```bash
cargo run -- split-candidates /path/to/feedback.jsonl \
  --repository example/project --revision HISTORICAL_REVISION
```

For a one-task pilot only, pass `--min-tasks 1`. The default requires two distinct
tasks, not two runs of the same task. The command checks the referenced reads,
but a requested range alone does not prove which lines were returned; inspect
`range_evidence` independently of the agent's confidence. Pi currently leaves
delivered ranges unknown, so its nominations cannot have `exact_delivered`
evidence. [§FS-feedback.2](../requirements.md#2-evidence-validation) [§FS-feedback.3](../requirements.md#3-split-candidate-report)

Use recurring spans as evidence for separating a coherent module or section.
Similar but unequal ranges remain separate candidates for human review. Inspect
both nominated and useful material before choosing the boundary. Keep necessary
context and conventions accessible; validate retrieval cost and task quality
after any split. There is no automatic splitter or measured-savings claim.
Feedback contains identities and paths; keep real sidecars private and apply
your own retention policy. [§FS-feedback.3](../requirements.md#3-split-candidate-report)
