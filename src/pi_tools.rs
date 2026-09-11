//! Translate supported native tool results, without opening source files. §FS-pi.2

use crate::{event::*, validation};
use anyhow::{Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) struct Call {
    pub name: String,
    pub arguments: Value,
    pub step: u64,
    pub model: Option<String>,
}

fn lexical_absolute(path: &str) -> Option<Vec<&str>> {
    if !path.starts_with('/') || path.contains('\\') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts)
}

fn local_path(cwd: &str, path: &str) -> Option<String> {
    let absolute = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{cwd}/{path}")
    };
    let root = lexical_absolute(cwd)?;
    let target = lexical_absolute(&absolute)?;
    if !target.starts_with(&root) {
        return None;
    }
    let relative = target[root.len()..].join("/");
    validation::relative_path(&relative).ok()?;
    Some(relative)
}

pub(crate) fn result(call: &Call, message: &Value, cwd: &str) -> Result<Option<EventData>> {
    if message.get("toolName").and_then(Value::as_str) != Some(call.name.as_str()) {
        return Ok(None);
    }
    let Some(path) = call
        .arguments
        .get("path")
        .and_then(Value::as_str)
        .and_then(|path| local_path(cwd, path))
    else {
        return Ok(None);
    };
    if matches!(call.name.as_str(), "edit" | "write") {
        return Ok(Some(EventData::Edit { path }));
    }
    if call.name != "read" {
        return Ok(None);
    }
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return Ok(None);
    };
    if content.len() != 1 || content[0].get("type").and_then(Value::as_str) != Some("text") {
        return Ok(None);
    }
    let Some(text) = content[0].get("text").and_then(Value::as_str) else {
        return Ok(None);
    };
    let start = match call.arguments.get("offset") {
        Some(value) => value.as_u64().filter(|n| *n > 0),
        None => Some(1),
    };
    let start = start.ok_or_else(|| anyhow::anyhow!("invalid native read offset"))?;
    let end = match call.arguments.get("limit") {
        Some(value) => {
            let limit = value.as_u64().unwrap_or(0);
            ensure!(limit > 0, "invalid native read limit");
            let end = start
                .checked_add(limit - 1)
                .ok_or_else(|| anyhow::anyhow!("native read range overflow"))?;
            Some(end)
        }
        None => None,
    };
    let truncated = message
        .pointer("/details/truncation/truncated")
        .and_then(Value::as_bool);
    Ok(Some(EventData::Read(Read {
        path,
        requested: Some(RequestedRange { start, end }),
        delivered: None,
        grund: None,
        bytes: Some(text.len() as u64),
        output_sha256: Some(format!("{:x}", Sha256::digest(text.as_bytes()))),
        estimated_tokens: None,
        provenance: Provenance {
            method: Method::NativeTranscript,
            exposure: Exposure::ToolReturned,
            confidence: Confidence::Recorded,
        },
        truncated,
    })))
}
