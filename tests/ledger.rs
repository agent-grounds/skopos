//! Transaction and evidence-contract regressions. §FS-ledger.1 §FS-events
mod common;

use common::*;
use skopos::{event::*, parse_events, store};
use std::io::Cursor;

#[test]
fn atomic_import_idempotency_conflicts_and_sealing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    let first = read("run", 1, "a.rs");
    assert_eq!(
        store::import(&db, &[first.clone(), first.clone()])
            .unwrap()
            .inserted,
        1
    );
    assert_eq!(
        store::import(&db, std::slice::from_ref(&first))
            .unwrap()
            .duplicates,
        1
    );
    let second = read("run", 2, "b.rs");
    let mut conflict = first.clone();
    conflict.task_id = "changed".to_string();
    assert!(store::import(&db, &[second.clone(), conflict]).is_err());
    assert_eq!(store::load(&db).unwrap().len(), 1);
    let batch = vec![second.clone(), end(&second)];
    assert_eq!(store::import(&db, &batch).unwrap().inserted, 2);
    assert_eq!(store::import(&db, &batch).unwrap().duplicates, 2);
    assert!(store::import(&db, &[read("run", 4, "c.rs")]).is_err());
    assert_eq!(store::load(&db).unwrap().len(), 3);
}

#[test]
fn inconsistent_metadata_sequences_and_epochs_roll_back() {
    for change in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("ledger.sqlite3");
        let first = read("run", 1, "a.rs");
        let mut second = read("run", 2, "b.rs");
        match change {
            0 => second.sequence = 1,
            1 => second.revision = Some("different".into()),
            2 => second.epoch = 1,
            _ => second.task_id = "different".into(),
        }
        assert!(store::import(&db, &[first, second]).is_err());
        assert!(store::load(&db).unwrap().is_empty());
    }
}

#[test]
fn reject_unknown_fields_and_false_evidence() {
    let event = read("run", 1, "a.rs");
    for field in ["content", "raw_prompt", "unknown_future_field"] {
        let mut value = serde_json::to_value(&event).unwrap();
        value["data"][field] = serde_json::json!("must not enter ledger");
        assert!(parse_events(Cursor::new(serde_json::to_vec(&value).unwrap())).is_err());
    }
    for path in [
        "../secret",
        "/tmp/file",
        "src/../a",
        "a//b",
        "a\\b",
        "C:/x",
        "",
    ] {
        assert!(skopos::validation::event(&read("run", 1, path)).is_err());
    }
    let mut invalid = event.clone();
    let EventData::Read(ref mut data) = invalid.data else {
        unreachable!()
    };
    data.provenance.method = Method::OsTrace;
    assert!(skopos::validation::event(&invalid).is_err());
    let mut invalid = event;
    let EventData::Read(ref mut data) = invalid.data else {
        unreachable!()
    };
    data.truncated = Some(true);
    data.delivered = Some(LineRange { start: 1, end: 10 });
    assert!(skopos::validation::event(&invalid).is_err());
}

#[test]
fn queries_do_not_create_databases_and_future_schemas_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    assert!(store::load(&db).is_err());
    assert!(!db.exists());
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    assert!(store::import(&db, &[read("run", 1, "a.rs")]).is_err());
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

#[test]
fn concurrent_first_imports_do_not_lose_events() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    let threads: Vec<_> = (0..4)
        .map(|i| {
            let db = db.clone();
            std::thread::spawn(move || store::import(&db, &[read(&format!("run-{i}"), 1, "a.rs")]))
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap().unwrap().inserted, 1);
    }
    assert_eq!(store::load(&db).unwrap().len(), 4);
}
