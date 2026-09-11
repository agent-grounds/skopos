//! Empirical associations over separate context samples. §FS-analysis.2

use crate::event::*;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub support: usize,
    pub distinct_tasks: usize,
    pub source_samples: usize,
    pub target_samples: usize,
    pub conditional_probability: f64,
    pub jaccard: f64,
    pub lift: f64,
    pub next_step_support: usize,
    pub next_step_probability: f64,
}

#[derive(Debug, Serialize)]
pub struct Graph {
    pub schema: &'static str,
    pub repository: String,
    pub revision: String,
    pub sample_unit: &'static str,
    pub evidence: &'static str,
    pub samples: usize,
    pub distinct_tasks: usize,
    pub min_support: usize,
    pub edges: Vec<Edge>,
}

#[derive(Default)]
struct Sample {
    task: String,
    files: BTreeSet<String>,
    stages: BTreeMap<u64, BTreeSet<String>>,
}

#[derive(Default)]
struct Pair {
    support: usize,
    tasks: BTreeSet<String>,
    next: usize,
}

pub fn build(
    events: &[Event],
    repository: &str,
    revision: &str,
    min_support: usize,
) -> Result<Graph> {
    ensure!(min_support > 0, "minimum support must be positive");
    let completed: BTreeSet<_> = events
        .iter()
        .filter_map(|e| matches!(e.data, EventData::RunEnd { .. }).then_some(e.run_id.as_str()))
        .collect();
    let mut samples: BTreeMap<Episode, Sample> = BTreeMap::new();
    for event in events {
        if event.repository != repository
            || event.revision.as_deref() != Some(revision)
            || !completed.contains(event.run_id.as_str())
        {
            continue;
        }
        let EventData::Read(read) = &event.data else {
            continue;
        };
        if !read.recorded_exposure() {
            continue;
        }
        let sample = samples.entry(event.episode()).or_default();
        sample.task = event.task_id.clone();
        sample.files.insert(read.path.clone());
        sample
            .stages
            .entry(event.step)
            .or_default()
            .insert(read.path.clone());
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut pairs: BTreeMap<(String, String), Pair> = BTreeMap::new();
    let mut tasks = BTreeSet::new();
    for sample in samples.values() {
        tasks.insert(&sample.task);
        for file in &sample.files {
            *counts.entry(file.clone()).or_default() += 1;
        }
        let stages: Vec<_> = sample.stages.values().collect();
        let mut next = BTreeSet::new();
        for adjacent in stages.windows(2) {
            for source in adjacent[0] {
                for target in adjacent[1] {
                    if source != target {
                        next.insert((source.clone(), target.clone()));
                    }
                }
            }
        }
        for source in &sample.files {
            for target in &sample.files {
                if source == target {
                    continue;
                }
                let key = (source.clone(), target.clone());
                let has_next = next.contains(&key);
                let pair = pairs.entry(key).or_default();
                pair.support += 1;
                pair.tasks.insert(sample.task.clone());
                pair.next += usize::from(has_next);
            }
        }
    }
    let mut edges = Vec::new();
    for ((source, target), pair) in pairs {
        if pair.support < min_support {
            continue;
        }
        let a = counts[&source];
        let b = counts[&target];
        let support = pair.support;
        edges.push(Edge {
            source,
            target,
            support,
            distinct_tasks: pair.tasks.len(),
            source_samples: a,
            target_samples: b,
            conditional_probability: support as f64 / a as f64,
            jaccard: support as f64 / (a + b - support) as f64,
            lift: support as f64 * samples.len() as f64 / (a as f64 * b as f64),
            next_step_support: pair.next,
            next_step_probability: pair.next as f64 / a as f64,
        });
    }
    edges.sort_by(|a, b| {
        b.support
            .cmp(&a.support)
            .then_with(|| b.lift.total_cmp(&a.lift))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.target.cmp(&b.target))
    });
    Ok(Graph {
        schema: "skopos.graph.v1",
        repository: repository.to_string(),
        revision: revision.to_string(),
        sample_unit: "run/agent/session/model/epoch",
        evidence: "recorded tool returns or model submissions; noncausal",
        samples: samples.len(),
        distinct_tasks: tasks.len(),
        min_support,
        edges,
    })
}
