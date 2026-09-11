//! Reread candidates are text equality within a context, not waste. §FS-analysis.1

use crate::event::*;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Totals {
    pub reads: u64,
    pub recorded_exposures: u64,
    pub known_returned_bytes: u64,
    pub missing_byte_counts: u64,
    pub reported_estimated_tokens: u64,
    pub missing_token_estimates: u64,
    pub identical_reread_candidates: u64,
    pub identical_returned_bytes: u64,
    pub known_truncated_reads: u64,
    pub unknown_truncation_reads: u64,
}

fn add(target: &mut u64, value: u64) -> Result<()> {
    *target = target
        .checked_add(value)
        .context("report total exceeds u64")?;
    Ok(())
}

impl Totals {
    fn record(&mut self, read: &Read, repeated: bool) -> Result<()> {
        self.reads += 1;
        self.recorded_exposures += u64::from(read.recorded_exposure());
        if let Some(bytes) = read.bytes {
            add(&mut self.known_returned_bytes, bytes)?;
        } else {
            self.missing_byte_counts += 1;
        }
        if let Some(tokens) = read.estimated_tokens {
            add(&mut self.reported_estimated_tokens, tokens)?;
        } else {
            self.missing_token_estimates += 1;
        }
        if repeated {
            self.identical_reread_candidates += 1;
            if let Some(bytes) = read.bytes {
                add(&mut self.identical_returned_bytes, bytes)?;
            }
        }
        self.known_truncated_reads += u64::from(read.truncated == Some(true));
        self.unknown_truncation_reads += u64::from(read.truncated.is_none());
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ProvenanceCount {
    pub provenance: Provenance,
    pub reads: u64,
}

#[derive(Debug, Serialize)]
pub struct FileReport {
    pub path: String,
    pub totals: Totals,
    pub grund_refs: BTreeSet<GroundRef>,
    pub requested_ranges: BTreeSet<RequestedRange>,
    pub delivered_ranges: BTreeSet<LineRange>,
    pub provenance: Vec<ProvenanceCount>,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub schema: &'static str,
    pub run_id: String,
    pub task_id: String,
    pub repository: String,
    pub revision: Option<String>,
    pub complete: bool,
    pub asserted_outcome: Option<Outcome>,
    pub interpretation: &'static str,
    pub totals: Totals,
    pub files: Vec<FileReport>,
    pub import_coverage: Vec<Coverage>,
}

type Fingerprint = (Episode, String, Option<LineRange>, String, Exposure);

pub fn run(events: &[Event], id: &str) -> Result<RunReport> {
    let selected: Vec<_> = events.iter().filter(|e| e.run_id == id).collect();
    let first = selected
        .first()
        .with_context(|| format!("run {id} not found"))?;
    let mut result = RunReport {
        schema: "skopos.report.v1",
        run_id: id.to_string(),
        task_id: first.task_id.clone(),
        repository: first.repository.clone(),
        revision: first.revision.clone(),
        complete: false,
        asserted_outcome: None,
        interpretation: "Observed tool text, not causal use, waste, or billed cost.",
        totals: Totals::default(),
        files: Vec::new(),
        import_coverage: Vec::new(),
    };
    let mut files: BTreeMap<String, FileReport> = BTreeMap::new();
    let mut seen: BTreeSet<Fingerprint> = BTreeSet::new();
    for event in selected {
        match &event.data {
            EventData::Read(read) => {
                let repeated = if read.recorded_exposure() {
                    read.output_sha256.as_ref().is_some_and(|hash| {
                        !seen.insert((
                            event.episode(),
                            read.path.clone(),
                            read.delivered.clone(),
                            hash.clone(),
                            read.provenance.exposure,
                        ))
                    })
                } else {
                    false
                };
                result.totals.record(read, repeated)?;
                let file = files
                    .entry(read.path.clone())
                    .or_insert_with(|| FileReport {
                        path: read.path.clone(),
                        totals: Totals::default(),
                        grund_refs: BTreeSet::new(),
                        requested_ranges: BTreeSet::new(),
                        delivered_ranges: BTreeSet::new(),
                        provenance: Vec::new(),
                    });
                file.totals.record(read, repeated)?;
                if let Some(ground) = &read.grund {
                    file.grund_refs.insert(ground.clone());
                }
                if let Some(range) = &read.requested {
                    file.requested_ranges.insert(range.clone());
                }
                if let Some(range) = &read.delivered {
                    file.delivered_ranges.insert(range.clone());
                }
                if let Some(count) = file
                    .provenance
                    .iter_mut()
                    .find(|p| p.provenance == read.provenance)
                {
                    count.reads += 1;
                } else {
                    file.provenance.push(ProvenanceCount {
                        provenance: read.provenance.clone(),
                        reads: 1,
                    });
                }
            }
            EventData::Edit { path } => {
                seen.retain(|(episode, seen_path, _, _, _)| {
                    !(episode.agent == event.agent_id
                        && episode.session == event.session_id
                        && episode.epoch == event.epoch
                        && seen_path == path)
                });
            }
            EventData::RunEnd { outcome } => {
                result.complete = true;
                result.asserted_outcome = Some(*outcome);
            }
            EventData::ImportSummary(coverage) => result.import_coverage.push(coverage.clone()),
            EventData::ContextReset => {}
        }
    }
    result.files = files.into_values().collect();
    Ok(result)
}
