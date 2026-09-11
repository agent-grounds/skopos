//! Offline observations and analyses of context acquisition. §FS-ledger §FS-analysis

pub mod event;
pub mod graph;
pub mod pi;
mod pi_tools;
pub mod report;
pub mod store;
pub mod validation;

use anyhow::{Context, Result};
use event::Event;
use std::io::BufRead;

/// Decode normalized JSONL without retaining an arbitrary payload. §FS-events
pub fn parse_events(reader: impl BufRead) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("reading input line {}", index + 1))?;
        if !line.trim().is_empty() {
            let event = serde_json::from_str(&line)
                .with_context(|| format!("invalid event at line {}", index + 1))?;
            events.push(event);
        }
    }
    Ok(events)
}
