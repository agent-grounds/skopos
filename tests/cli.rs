//! End-to-end native import, persistence, inspection, and JSON errors. §FS-ledger.2 §FS-pi

use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};

fn invoke(db: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_skopos"))
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn import_query_reimport_graph_and_compare_use_the_persistent_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/pi-session.jsonl");
    let args = [
        "import-pi",
        fixture,
        "--repository",
        "demo",
        "--revision",
        "demo-rev",
        "--run",
        "demo",
        "--complete",
    ];
    let imported = success(invoke(&db, &args));
    assert_eq!(imported["inserted"], 9);
    let again = success(invoke(&db, &args));
    assert_eq!(again["inserted"], 0);
    assert_eq!(again["duplicates"], 9);
    let profile = success(invoke(&db, &["report", "demo"]));
    assert_eq!(profile["totals"]["reads"], 6);
    assert_eq!(profile["totals"]["identical_reread_candidates"], 1);
    assert_eq!(success(invoke(&db, &["files", "demo"])), profile);
    let graph = success(invoke(
        &db,
        &[
            "related",
            "docs/spec.md",
            "--repository",
            "demo",
            "--revision",
            "demo-rev",
        ],
    ));
    assert_eq!(graph["samples"], 2);
    assert_eq!(graph["distinct_tasks"], 1);
    assert_eq!(graph["edges"].as_array().unwrap().len(), 1);
    assert_eq!(graph["edges"][0]["target"], "src/lib.rs");
    let comparison = success(invoke(&db, &["compare", "demo", "demo"]));
    assert_eq!(comparison["known_returned_bytes_delta"], 0);
    assert_eq!(
        success(invoke(&db, &["status"]))["runs"][0]["complete"],
        true
    );
    let stored = std::fs::read(&db).unwrap();
    assert!(
        !stored
            .windows(b"SYNTHETIC_".len())
            .any(|w| w == b"SYNTHETIC_")
    );
}

#[test]
fn bad_input_fails_without_stdout_or_partial_records() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    let missing = invoke(&db, &["status"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(!db.exists());
    let file = dir.path().join("invalid.jsonl");
    std::fs::write(&file, "{\"content\":\"do not store me\"}\n").unwrap();
    let invalid = invoke(&db, &["import", file.to_str().unwrap()]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(!db.exists());
}
