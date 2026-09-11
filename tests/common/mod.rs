//! Synthetic event builders; no real session data. §FS-events
#![allow(dead_code)]

use skopos::event::*;

pub fn read(run: &str, sequence: u64, path: &str) -> Event {
    Event {
        schema: SCHEMA.to_string(),
        event_id: format!("{run}:{sequence}"),
        run_id: run.to_string(),
        task_id: run.to_string(),
        repository: "demo".to_string(),
        worktree: "/demo/repo".to_string(),
        revision: Some("revision-1".to_string()),
        agent_id: "agent".to_string(),
        session_id: "session".to_string(),
        model: Some("model".to_string()),
        epoch: 0,
        sequence,
        step: sequence,
        timestamp: "2026-09-11T10:00:00Z".to_string(),
        data: EventData::Read(Read {
            path: path.to_string(),
            requested: None,
            delivered: None,
            grund: None,
            bytes: Some(10),
            output_sha256: Some("a".repeat(64)),
            estimated_tokens: None,
            provenance: Provenance {
                method: Method::Runtime,
                exposure: Exposure::ToolReturned,
                confidence: Confidence::Recorded,
            },
            truncated: Some(false),
        }),
    }
}

pub fn end(previous: &Event) -> Event {
    let mut event = previous.clone();
    event.event_id = format!("{}:end", previous.run_id);
    event.sequence += 1;
    event.step += 1;
    event.data = EventData::RunEnd {
        outcome: Outcome::Unknown,
    };
    event
}

pub fn complete(mut events: Vec<Event>) -> Vec<Event> {
    events.push(end(events.last().unwrap()));
    events
}

pub fn pi_options() -> skopos::pi::Options {
    skopos::pi::Options {
        repository: "demo".to_string(),
        run: "native".to_string(),
        task: "task".to_string(),
        revision: "revision-1".to_string(),
        leaf: None,
        complete: true,
    }
}

pub const PI: &str = include_str!("../../examples/pi-session.jsonl");
