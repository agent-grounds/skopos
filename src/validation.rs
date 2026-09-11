//! Reject overclaims and inconsistent event histories. §FS-events §FS-ledger.1

use crate::event::*;
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn relative_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && !path.contains('\\'),
        "path must be repository-relative POSIX text"
    );
    ensure!(
        !path.chars().any(char::is_control)
            && path.split('/').all(|part| !matches!(part, "" | "." | "..")),
        "path must be normalized and repository-relative"
    );
    ensure!(!path.contains(':'), "path must not be a drive path or URI");
    Ok(())
}

fn nonempty(value: &str, name: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{name} must not be empty");
    ensure!(
        !value.chars().any(char::is_control),
        "{name} contains control characters"
    );
    Ok(())
}

pub fn event(event: &Event) -> Result<()> {
    ensure!(
        event.schema == SCHEMA,
        "unsupported event schema: {}",
        event.schema
    );
    for (name, value) in [
        ("event_id", &event.event_id),
        ("run_id", &event.run_id),
        ("task_id", &event.task_id),
        ("repository", &event.repository),
        ("worktree", &event.worktree),
        ("agent_id", &event.agent_id),
        ("session_id", &event.session_id),
        ("timestamp", &event.timestamp),
    ] {
        nonempty(value, name)?;
    }
    ensure!(
        Path::new(&event.worktree).is_absolute(),
        "worktree must be absolute"
    );
    for value in [&event.revision, &event.model].into_iter().flatten() {
        nonempty(value, "revision/model")?;
    }
    ensure!(
        event.sequence > 0 && event.sequence <= i64::MAX as u64,
        "invalid sequence"
    );
    ensure!(event.step > 0, "step must be positive");
    match &event.data {
        EventData::Read(read) => validate_read(read)?,
        EventData::Edit { path } => relative_path(path)?,
        _ => {}
    }
    Ok(())
}

fn validate_read(read: &Read) -> Result<()> {
    relative_path(&read.path)?;
    if let Some(range) = &read.requested {
        ensure!(
            range.start > 0 && range.end.is_none_or(|end| end >= range.start),
            "invalid requested line range"
        );
    }
    if let Some(range) = &read.delivered {
        ensure!(
            range.start > 0 && range.end >= range.start,
            "invalid inclusive line range"
        );
    }
    if let Some(ground) = &read.grund {
        nonempty(&ground.id, "grund.id")?;
        if let Some(section) = &ground.section {
            nonempty(section, "grund.section")?;
        }
    }
    if let Some(hash) = &read.output_sha256 {
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "output_sha256 must contain 64 lowercase hexadecimal digits"
        );
    }
    ensure!(
        read.delivered.is_none() || read.truncated == Some(false),
        "an exact delivered range requires known untruncated output"
    );
    let p = &read.provenance;
    if p.method == Method::OsTrace {
        ensure!(
            p.exposure == Exposure::FilesystemOnly && p.confidence == Confidence::Inferred,
            "OS access cannot establish model-context exposure"
        );
    }
    if p.method == Method::ShellInference {
        ensure!(
            p.confidence == Confidence::Inferred && p.exposure != Exposure::ModelSubmitted,
            "shell inference cannot establish model submission"
        );
    }
    if p.exposure == Exposure::FilesystemOnly {
        ensure!(
            read.bytes.is_none()
                && read.estimated_tokens.is_none()
                && read.output_sha256.is_none()
                && read.delivered.is_none(),
            "filesystem-only access cannot measure returned content"
        );
    }
    Ok(())
}

/// Validate insertion order after exact duplicates have been removed. §FS-ledger.1
pub fn history(events: &[Event]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut runs: BTreeMap<&str, &Event> = BTreeMap::new();
    let mut sessions: BTreeMap<(&str, &str, &str), &Event> = BTreeMap::new();
    let mut sealed = BTreeSet::new();
    for current in events {
        event(current).with_context(|| format!("event {}", current.event_id))?;
        ensure!(
            ids.insert(&current.event_id),
            "duplicate event ID {}",
            current.event_id
        );
        ensure!(
            !sealed.contains(&current.run_id),
            "run {} is sealed",
            current.run_id
        );
        if let Some(first) = runs.get(current.run_id.as_str()) {
            ensure!(
                first.repository == current.repository
                    && first.worktree == current.worktree
                    && first.revision == current.revision
                    && first.task_id == current.task_id,
                "inconsistent metadata for run {}",
                current.run_id
            );
        } else {
            runs.insert(&current.run_id, current);
        }
        let key = (
            current.run_id.as_str(),
            current.agent_id.as_str(),
            current.session_id.as_str(),
        );
        if let Some(previous) = sessions.insert(key, current) {
            ensure!(
                current.sequence > previous.sequence && current.step >= previous.step,
                "sequence/step does not advance in {}",
                current.event_id
            );
            let valid_epoch = if matches!(current.data, EventData::ContextReset) {
                current.epoch > previous.epoch
            } else {
                current.epoch == previous.epoch
            };
            ensure!(
                valid_epoch,
                "epoch changes require context_reset: {}",
                current.event_id
            );
        }
        if matches!(current.data, EventData::RunEnd { .. }) {
            sealed.insert(&current.run_id);
        }
    }
    Ok(())
}
