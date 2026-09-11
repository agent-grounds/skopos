//! Atomic, idempotent, versioned local storage. §FS-ledger.1

use crate::{event::Event, validation};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path, time::Duration};

const VERSION: u32 = 1;

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub schema: &'static str,
    pub inserted: usize,
    pub duplicates: usize,
}

fn schema(connection: &Connection, create: bool) -> Result<()> {
    if create {
        connection.execute_batch("BEGIN IMMEDIATE;")?;
    }
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 && create {
        let tables: u64 = connection.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        ensure!(tables == 0, "refusing an unversioned nonempty database");
        connection.execute_batch(
            "CREATE TABLE events (
               ordinal INTEGER PRIMARY KEY,
               event_id TEXT NOT NULL UNIQUE,
               run_id TEXT NOT NULL,
               agent_id TEXT NOT NULL,
               session_id TEXT NOT NULL,
               sequence INTEGER NOT NULL,
               record TEXT NOT NULL,
               UNIQUE(run_id, agent_id, session_id, sequence)
             );
             PRAGMA user_version=1;",
        )?;
    } else {
        ensure!(
            version == VERSION,
            "unsupported database schema version {version}"
        );
    }
    if create {
        connection.execute_batch("COMMIT;")?;
    }
    Ok(())
}

fn records(connection: &Connection) -> Result<Vec<Event>> {
    let mut statement = connection.prepare("SELECT record FROM events ORDER BY ordinal")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| {
        let row = row?;
        serde_json::from_str(&row).context("invalid stored event")
    })
    .collect()
}

pub fn import(path: &Path, incoming: &[Event]) -> Result<ImportResult> {
    for event in incoming {
        validation::event(event)?;
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).context("creating ledger directory")?;
    }
    let mut connection = Connection::open(path).context("opening ledger")?;
    connection.busy_timeout(Duration::from_secs(5))?;
    schema(&connection, true)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut all = records(&tx)?;
    let mut known: BTreeMap<String, Event> = all
        .iter()
        .map(|event| (event.event_id.clone(), event.clone()))
        .collect();
    let mut added = Vec::new();
    let mut duplicates = 0;
    for event in incoming {
        if let Some(previous) = known.get(&event.event_id) {
            ensure!(previous == event, "conflicting event ID {}", event.event_id);
            duplicates += 1;
        } else {
            known.insert(event.event_id.clone(), event.clone());
            all.push(event.clone());
            added.push(event);
        }
    }
    validation::history(&all)?;
    for event in &added {
        tx.execute(
            "INSERT INTO events(event_id,run_id,agent_id,session_id,sequence,record)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                event.event_id,
                event.run_id,
                event.agent_id,
                event.session_id,
                event.sequence,
                serde_json::to_string(event)?
            ],
        )?;
    }
    tx.commit()?;
    Ok(ImportResult {
        schema: "skopos.import.v1",
        inserted: added.len(),
        duplicates,
    })
}

pub fn load(path: &Path) -> Result<Vec<Event>> {
    ensure!(
        path.is_file(),
        "ledger not found at {}; run skopos import first",
        path.display()
    );
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("opening ledger read-only")?;
    schema(&connection, false)?;
    records(&connection)
}
