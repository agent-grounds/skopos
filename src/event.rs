//! Closed, content-free observation contract. §FS-events

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "skopos.event.v1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub schema: String,
    pub event_id: String,
    pub run_id: String,
    pub task_id: String,
    pub repository: String,
    pub worktree: String,
    pub revision: Option<String>,
    pub agent_id: String,
    pub session_id: String,
    pub model: Option<String>,
    pub epoch: u64,
    pub sequence: u64,
    pub step: u64,
    pub timestamp: String,
    pub data: EventData,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventData {
    Read(Read),
    Edit { path: String },
    ContextReset,
    ImportSummary(Coverage),
    RunEnd { outcome: Outcome },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub path: String,
    pub requested: Option<RequestedRange>,
    pub delivered: Option<LineRange>,
    pub grund: Option<GroundRef>,
    pub bytes: Option<u64>,
    pub output_sha256: Option<String>,
    pub estimated_tokens: Option<u64>,
    pub provenance: Provenance,
    pub truncated: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct RequestedRange {
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct GroundRef {
    pub id: String,
    pub section: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub method: Method,
    pub exposure: Exposure,
    pub confidence: Confidence,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Runtime,
    NativeTranscript,
    ShellInference,
    OsTrace,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Exposure {
    ToolReturned,
    ModelSubmitted,
    FilesystemOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Recorded,
    Inferred,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Unknown,
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub selected_entries: u64,
    pub recognized_reads: u64,
    pub unsupported_tool_results: u64,
    pub failed_tool_results: u64,
}

/// The context sample excludes workflow siblings. §FS-analysis.2
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Episode {
    pub run: String,
    pub agent: String,
    pub session: String,
    pub model: Option<String>,
    pub epoch: u64,
}

impl Event {
    pub fn episode(&self) -> Episode {
        Episode {
            run: self.run_id.clone(),
            agent: self.agent_id.clone(),
            session: self.session_id.clone(),
            model: self.model.clone(),
            epoch: self.epoch,
        }
    }
}

impl Read {
    pub fn recorded_exposure(&self) -> bool {
        self.provenance.confidence == Confidence::Recorded
            && self.provenance.exposure != Exposure::FilesystemOnly
    }
}
