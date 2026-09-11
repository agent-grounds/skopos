//! Import one frozen branch of a Pi v3 session. §FS-pi

use crate::{
    event::*,
    pi_tools::{self, Call},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::BufRead,
};

pub struct Options {
    pub repository: String,
    pub run: String,
    pub task: String,
    pub revision: String,
    pub leaf: Option<String>,
    pub complete: bool,
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .with_context(|| format!("missing or invalid native {field}"))
}

fn parent(entry: &Value) -> Result<Option<&str>> {
    match entry.get("parentId") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if !id.is_empty() => Ok(Some(id)),
        _ => bail!("invalid native parentId"),
    }
}

fn branch<'a>(entries: &'a BTreeMap<String, Value>, leaf: &str) -> Result<Vec<&'a Value>> {
    let mut seen = BTreeSet::new();
    let mut selected = Vec::new();
    let mut cursor = Some(leaf);
    while let Some(id) = cursor {
        ensure!(seen.insert(id), "cycle in native session tree");
        let entry = entries
            .get(id)
            .context("missing parent or leaf in native session tree")?;
        selected.push(entry);
        cursor = parent(entry)?;
    }
    selected.reverse();
    Ok(selected)
}

fn validate_tree(entries: &BTreeMap<String, Value>) -> Result<()> {
    let mut checked = BTreeSet::new();
    for id in entries.keys() {
        let mut path = BTreeSet::new();
        let mut cursor = Some(id.as_str());
        while let Some(id) = cursor {
            if checked.contains(id) {
                break;
            }
            ensure!(path.insert(id), "cycle in native session tree");
            let entry = entries
                .get(id)
                .context("missing parent in native session tree")?;
            cursor = parent(entry)?;
        }
        checked.extend(path);
    }
    Ok(())
}

struct Builder<'a> {
    options: &'a Options,
    cwd: String,
    session: String,
    epoch: u64,
    model: Option<String>,
    events: Vec<Event>,
}

impl Builder<'_> {
    fn push(&mut self, id: &str, timestamp: &str, step: u64, data: EventData) {
        let kind = match &data {
            EventData::Read(_) => "read",
            EventData::Edit { .. } => "edit",
            EventData::ContextReset => "reset",
            EventData::ImportSummary(_) => "summary",
            EventData::RunEnd { .. } => "end",
        };
        let identity =
            serde_json::to_vec(&(self.options.run.as_str(), self.session.as_str(), id, kind))
                .expect("string tuple serialization cannot fail");
        self.events.push(Event {
            schema: SCHEMA.to_string(),
            event_id: format!("pi:{:x}", Sha256::digest(identity)),
            run_id: self.options.run.clone(),
            task_id: self.options.task.clone(),
            repository: self.options.repository.clone(),
            worktree: self.cwd.clone(),
            revision: Some(self.options.revision.clone()),
            agent_id: "pi".to_string(),
            session_id: self.session.clone(),
            model: self.model.clone(),
            epoch: self.epoch,
            sequence: self.events.len() as u64 + 1,
            step,
            timestamp: timestamp.to_string(),
            data,
        });
    }
}

pub fn parse(reader: impl BufRead, options: &Options) -> Result<Vec<Event>> {
    let mut header = None;
    let mut entries = BTreeMap::new();
    let mut last_id = None;
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("invalid Pi JSON at line {}", index + 1))?;
        if string(&value, "type")? == "session" {
            ensure!(
                header.is_none() && entries.is_empty(),
                "expected one initial Pi header"
            );
            ensure!(
                value.get("version").and_then(Value::as_u64) == Some(3),
                "only Pi session version 3 is supported"
            );
            header = Some(value);
        } else {
            ensure!(header.is_some(), "Pi session header must precede entries");
            let id = string(&value, "id")?.to_string();
            parent(&value)?;
            string(&value, "timestamp")?;
            ensure!(!entries.contains_key(&id), "duplicate native entry ID {id}");
            last_id = Some(id.clone());
            entries.insert(id, value);
        }
    }
    let header = header.context("missing Pi session header")?;
    // Validate abandoned ancestry too, without importing its observations. §FS-pi.1
    validate_tree(&entries)?;
    let leaf = options.leaf.as_ref().or(last_id.as_ref());
    let selected = match leaf {
        Some(id) => branch(&entries, id)?,
        None => Vec::new(),
    };
    let mut builder = Builder {
        options,
        cwd: string(&header, "cwd")?.to_string(),
        session: string(&header, "id")?.to_string(),
        epoch: 0,
        model: None,
        events: Vec::new(),
    };
    let mut calls: BTreeMap<String, Call> = BTreeMap::new();
    let mut coverage = Coverage {
        selected_entries: selected.len() as u64,
        ..Coverage::default()
    };
    for (index, entry) in selected.iter().enumerate() {
        let step = index as u64 + 1;
        let id = string(entry, "id")?;
        let timestamp = string(entry, "timestamp")?;
        match string(entry, "type")? {
            "compaction" | "branch_summary" => {
                builder.epoch += 1;
                calls.clear();
                builder.push(id, timestamp, step, EventData::ContextReset);
            }
            "message" => {
                let message = &entry["message"];
                match message.get("role").and_then(Value::as_str) {
                    Some("assistant") => {
                        builder.model =
                            message
                                .get("model")
                                .and_then(Value::as_str)
                                .map(|model| {
                                    match message.get("provider").and_then(Value::as_str) {
                                        Some(provider) => format!("{provider}:{model}"),
                                        None => model.to_string(),
                                    }
                                });
                        if let Some(blocks) = message.get("content").and_then(Value::as_array) {
                            for block in blocks {
                                if block.get("type").and_then(Value::as_str) == Some("toolCall") {
                                    let call_id = string(block, "id")?.to_string();
                                    ensure!(
                                        !calls.contains_key(&call_id),
                                        "duplicate pending tool call"
                                    );
                                    calls.insert(
                                        call_id,
                                        Call {
                                            name: string(block, "name")?.to_string(),
                                            arguments: block["arguments"].clone(),
                                            step,
                                            model: builder.model.clone(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    Some("toolResult") => {
                        let call = message
                            .get("toolCallId")
                            .and_then(Value::as_str)
                            .and_then(|id| calls.remove(id));
                        if message.get("isError").and_then(Value::as_bool) == Some(true) {
                            coverage.failed_tool_results += 1;
                            continue;
                        }
                        let supported = if let Some(call) = call {
                            if message.get("isError").and_then(Value::as_bool) == Some(false) {
                                match pi_tools::result(&call, message, &builder.cwd)? {
                                    Some(data) => {
                                        if matches!(data, EventData::Read(_)) {
                                            coverage.recognized_reads += 1;
                                        }
                                        builder.model = call.model;
                                        builder.push(id, timestamp, call.step, data);
                                        true
                                    }
                                    None => false,
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if !supported {
                            coverage.unsupported_tool_results += 1;
                        }
                    }
                    Some("bashExecution") => coverage.unsupported_tool_results += 1,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let timestamp = match selected.last() {
        Some(entry) => string(entry, "timestamp")?,
        None => string(&header, "timestamp")?,
    };
    let next_step = selected.len() as u64 + 1;
    builder.push(
        "import-summary",
        timestamp,
        next_step,
        EventData::ImportSummary(coverage),
    );
    if options.complete {
        builder.push(
            "run-end",
            timestamp,
            next_step + 1,
            EventData::RunEnd {
                outcome: Outcome::Unknown,
            },
        );
    }
    crate::validation::history(&builder.events)?;
    Ok(builder.events)
}
