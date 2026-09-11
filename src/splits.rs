//! Rank review candidates from self-reported unused spans, never measured non-use. §FS-feedback.3

use crate::{
    event::{Event, LineRange},
    feedback::{self, Feedback, ValidatedNomination},
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn report(
    events: &[Event],
    feedback: &[Feedback],
    repository: &str,
    revision: &str,
    min_tasks: usize,
    limit: usize,
) -> Result<Value> {
    ensure!(
        min_tasks > 0 && limit > 0,
        "min-tasks and limit must be positive"
    );
    let valid = feedback::validate(events, feedback)?;
    let selected: Vec<_> = valid
        .reports
        .iter()
        .filter(|r| r.repository == repository && r.revision == revision)
        .collect();
    let run_tasks: BTreeMap<_, _> = events
        .iter()
        .map(|event| (event.run_id.as_str(), event.task_id.as_str()))
        .collect();
    let tasks: BTreeSet<_> = selected
        .iter()
        .map(|r| run_tasks[r.run_id.as_str()])
        .collect();
    let abstentions = selected.iter().filter(|r| r.nominations.is_empty()).count();
    let mut groups: BTreeMap<(&str, &LineRange), Vec<&ValidatedNomination<'_>>> = BTreeMap::new();
    for n in &valid.nominations {
        if n.feedback.repository == repository && n.feedback.revision == revision {
            groups
                .entry((&n.nomination.path, &n.nomination.range))
                .or_default()
                .push(n);
        }
    }
    let nominated_spans = groups.len();
    let mut candidates = Vec::new();
    for ((path, range), mut nominations) in groups {
        let distinct_tasks = nominations
            .iter()
            .map(|n| &n.event.task_id)
            .collect::<BTreeSet<_>>()
            .len();
        if distinct_tasks < min_tasks {
            continue;
        }
        let best_rank = nominations.iter().map(|n| n.nomination.rank).min().unwrap();
        nominations.sort_by_key(|n| (&n.feedback.feedback_id, n.nomination.rank));
        let evidence: Vec<_> = nominations.iter().map(|n| json!({
            "feedback_id": n.feedback.feedback_id, "run_id": n.event.run_id,
            "task_id": n.event.task_id, "agent_id": n.event.agent_id,
            "session_id": n.event.session_id, "model": n.event.model, "epoch": n.event.epoch,
            "read_event_id": n.event.event_id, "grund": n.read.grund,
            "rank": n.nomination.rank, "reason": n.nomination.reason,
            "self_reported_confidence": n.nomination.confidence,
            "suggestion": n.nomination.suggestion, "range_evidence": n.range_evidence
        })).collect();
        candidates.push((
            distinct_tasks,
            best_rank,
            path,
            range,
            json!({
                "path": path, "range": range, "distinct_tasks": distinct_tasks,
                "report_support": nominations.len(), "best_rank": best_rank, "evidence": evidence
            }),
        ));
    }
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(a.1.cmp(&b.1))
            .then(a.2.cmp(b.2))
            .then(a.3.cmp(b.3))
    });
    let eligible_spans = candidates.len();
    let candidates: Vec<_> = candidates.into_iter().take(limit).map(|c| c.4).collect();
    Ok(json!({
        "schema": "skopos.splits.v1", "repository": repository, "revision": revision,
        "min_tasks": min_tasks, "limit": limit, "reports_considered": selected.len(),
        "distinct_tasks_reporting": tasks.len(), "abstaining_reports": abstentions,
        "nominated_spans": nominated_spans, "eligible_spans": eligible_spans, "candidates": candidates,
        "interpretation": "Agent-reported review candidates, not verified non-use, deletion instructions, or measured savings. Missing feedback does not establish use."
    }))
}
