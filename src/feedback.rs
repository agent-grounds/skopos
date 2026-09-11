//! Retrospective agent nominations, separate from observed reads. §FS-feedback.1 §FS-feedback.2

use crate::{event::*, validation};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::BufRead,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub schema: String,
    pub feedback_id: String,
    pub run_id: String,
    pub repository: String,
    pub revision: String,
    pub agent_id: String,
    pub session_id: String,
    pub timestamp: String,
    pub nominations: Vec<Nomination>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Nomination {
    pub rank: usize,
    pub read_event_id: String,
    pub path: String,
    pub range: LineRange,
    pub reason: Reason,
    pub confidence: SelfConfidence,
    pub suggestion: Suggestion,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    UnrelatedToTask,
    ExcessDetail,
    DuplicateInformation,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelfConfidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Suggestion {
    ExtractFile,
    ExtractSection,
    NarrowRead,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RangeEvidence {
    ExactDelivered,
    RequestedOnly,
    Unverified,
}

pub struct ValidatedNomination<'a> {
    pub feedback: &'a Feedback,
    pub nomination: &'a Nomination,
    pub event: &'a Event,
    pub read: &'a Read,
    pub range_evidence: RangeEvidence,
}

pub struct Validated<'a> {
    pub reports: Vec<&'a Feedback>,
    pub nominations: Vec<ValidatedNomination<'a>>,
}

pub fn parse(reader: impl BufRead) -> Result<Vec<Feedback>> {
    let mut reports = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("reading feedback line {}", index + 1))?;
        if !line.trim().is_empty() {
            reports.push(
                serde_json::from_str(&line)
                    .with_context(|| format!("invalid feedback at line {}", index + 1))?,
            );
        }
    }
    Ok(reports)
}

fn shape(report: &Feedback) -> Result<()> {
    ensure!(
        report.schema == "skopos.feedback.v1",
        "unsupported feedback schema"
    );
    for value in [
        &report.feedback_id,
        &report.run_id,
        &report.repository,
        &report.revision,
        &report.agent_id,
        &report.session_id,
        &report.timestamp,
    ] {
        ensure!(
            !value.trim().is_empty() && !value.chars().any(char::is_control),
            "feedback identity/timestamp must be nonempty text without controls"
        );
    }
    ensure!(
        report.nominations.len() <= 5,
        "at most five nominations per report"
    );
    let mut spans = BTreeSet::new();
    let mut ranks = BTreeSet::new();
    for n in &report.nominations {
        validation::relative_path(&n.path)?;
        ensure!(
            n.range.start > 0 && n.range.end >= n.range.start,
            "invalid nominated range"
        );
        ensure!(
            spans.insert((&n.path, &n.range)),
            "duplicate nominated span"
        );
        ensure!(
            n.rank > 0 && n.rank <= report.nominations.len() && ranks.insert(n.rank),
            "ranks must be unique and contiguous from 1"
        );
    }
    Ok(())
}

fn range_evidence(read: &Read, range: &LineRange) -> Result<RangeEvidence> {
    if let Some(delivered) = &read.delivered {
        ensure!(
            range.start >= delivered.start && range.end <= delivered.end,
            "nomination lies outside the delivered range"
        );
        Ok(RangeEvidence::ExactDelivered)
    } else if let Some(requested) = &read.requested {
        ensure!(
            range.start >= requested.start && requested.end.is_none_or(|end| range.end <= end),
            "nomination contradicts the requested range"
        );
        Ok(RangeEvidence::RequestedOnly)
    } else {
        Ok(RangeEvidence::Unverified)
    }
}

pub fn validate<'a>(events: &'a [Event], reports: &'a [Feedback]) -> Result<Validated<'a>> {
    validation::history(events)?;
    let mut runs = BTreeMap::new();
    let mut contexts = BTreeSet::new();
    let mut sealed = BTreeSet::new();
    let mut edited = BTreeSet::new();
    let mut post_edit = BTreeSet::new();
    let by_id: BTreeMap<_, _> = events.iter().map(|e| (e.event_id.as_str(), e)).collect();
    for e in events {
        runs.entry(e.run_id.as_str()).or_insert(e);
        contexts.insert((
            e.run_id.as_str(),
            e.agent_id.as_str(),
            e.session_id.as_str(),
        ));
        match &e.data {
            EventData::RunEnd { .. } => {
                sealed.insert(e.run_id.as_str());
            }
            EventData::Edit { path } => {
                edited.insert((e.run_id.as_str(), path.as_str()));
            }
            EventData::Read(read) if edited.contains(&(e.run_id.as_str(), read.path.as_str())) => {
                post_edit.insert(e.event_id.as_str());
            }
            _ => {}
        }
    }
    let mut known = BTreeMap::new();
    let mut reporting = BTreeSet::new();
    let mut result = Validated {
        reports: Vec::new(),
        nominations: Vec::new(),
    };
    for report in reports {
        shape(report).with_context(|| format!("feedback {}", report.feedback_id))?;
        if let Some(previous) = known.insert(&report.feedback_id, report) {
            ensure!(
                previous == report,
                "conflicting feedback ID {}",
                report.feedback_id
            );
            continue;
        }
        let identity = (
            report.run_id.as_str(),
            report.agent_id.as_str(),
            report.session_id.as_str(),
        );
        ensure!(
            reporting.insert(identity),
            "multiple reports for one run/agent/session"
        );
        let run = runs
            .get(report.run_id.as_str())
            .context("feedback run not found")?;
        ensure!(
            sealed.contains(report.run_id.as_str()),
            "feedback requires a completed run"
        );
        ensure!(
            run.repository == report.repository
                && run.revision.as_deref() == Some(&report.revision),
            "feedback repository/revision does not match the run"
        );
        ensure!(
            contexts.contains(&identity),
            "reporting agent/session not found in run"
        );
        for n in &report.nominations {
            let event = *by_id
                .get(n.read_event_id.as_str())
                .context("nominated read event not found")?;
            ensure!(
                (
                    event.run_id.as_str(),
                    event.agent_id.as_str(),
                    event.session_id.as_str()
                ) == identity,
                "nominated read belongs to a different run/agent/session"
            );
            let EventData::Read(read) = &event.data else {
                anyhow::bail!("nominated event is not a read")
            };
            ensure!(
                read.recorded_exposure(),
                "nomination requires a recorded non-filesystem read"
            );
            ensure!(
                read.path == n.path,
                "nominated path does not match the read"
            );
            ensure!(
                !post_edit.contains(event.event_id.as_str()),
                "read follows a recorded edit; base-revision coordinates are uncertain"
            );
            result.nominations.push(ValidatedNomination {
                feedback: report,
                nomination: n,
                event,
                read,
                range_evidence: range_evidence(read, &n.range)?,
            });
        }
        result.reports.push(report);
    }
    Ok(result)
}
