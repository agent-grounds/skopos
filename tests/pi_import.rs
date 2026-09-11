//! Native pairing, tree selection, and disclosure boundaries. §FS-pi
mod common;

use common::*;
use serde_json::{Value, json};
use skopos::{event::*, pi, report};
use std::io::Cursor;

fn lines(values: &[Value]) -> String {
    values
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn native_import_counts_only_supported_results_and_discards_content() {
    let options = pi_options();
    let events = pi::parse(Cursor::new(PI), &options).unwrap();
    assert_eq!(events, pi::parse(Cursor::new(PI), &options).unwrap());
    let profile = report::run(&events, "native").unwrap();
    assert_eq!(profile.totals.reads, 6);
    assert_eq!(profile.totals.identical_reread_candidates, 1);
    assert_eq!(profile.import_coverage[0].unsupported_tool_results, 1);
    assert_eq!(profile.totals.unknown_truncation_reads, 2);
    assert!(profile.complete);
    let EventData::Read(first) = &events[0].data else {
        panic!("expected read");
    };
    assert_eq!(
        first.requested,
        Some(RequestedRange {
            start: 1,
            end: Some(2)
        })
    );
    assert_eq!(first.delivered, None);
    assert_eq!(
        first.bytes,
        Some("SYNTHETIC_SOURCE_TEXT\nsecond line\n".len() as u64)
    );
    assert_eq!(first.provenance.exposure, Exposure::ToolReturned);
    let serialized = serde_json::to_string(&events).unwrap();
    assert!(!serialized.contains("SYNTHETIC_"));
    assert!(!serialized.contains("cat ignored.txt"));
    assert!(!serialized.contains("pub fn"));
}

#[test]
fn select_one_ancestry_and_never_union_abandoned_paths() {
    let mut options = pi_options();
    options.leaf = Some("b".into());
    let events = pi::parse(Cursor::new(PI), &options).unwrap();
    assert_eq!(report::run(&events, "native").unwrap().totals.reads, 1);
    let mut values: Vec<Value> = PI
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut new_call = values[2].clone();
    new_call["id"] = json!("new-call");
    new_call["message"]["content"][0]["arguments"]["path"] = json!("new-branch.rs");
    let mut new_result = values[3].clone();
    new_result["id"] = json!("new-result");
    new_result["parentId"] = json!("new-call");
    values.extend([new_call, new_result]);
    options.leaf = None;
    let events = pi::parse(Cursor::new(lines(&values)), &options).unwrap();
    let profile = report::run(&events, "native").unwrap();
    assert_eq!(profile.files.len(), 1);
    assert_eq!(profile.files[0].path, "new-branch.rs");
}

#[test]
fn malformed_native_trees_are_rejected() {
    let values: Vec<Value> = PI
        .lines()
        .take(4)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for variant in 0..4 {
        let mut bad = values.clone();
        match variant {
            0 => bad[0]["version"] = json!(4),
            1 => bad[1]["parentId"] = json!("missing"),
            2 => bad[1]["parentId"] = json!("b"),
            _ => bad.push(bad[3].clone()),
        }
        assert!(pi::parse(Cursor::new(lines(&bad)), &pi_options()).is_err());
    }
}

#[test]
fn failed_external_nontext_and_unmatched_results_are_coverage_gaps() {
    let values: Vec<Value> = PI
        .lines()
        .take(4)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for variant in 0..5 {
        let mut changed = values.clone();
        match variant {
            0 => changed[3]["message"]["isError"] = json!(true),
            1 => changed[2]["message"]["content"][0]["arguments"]["path"] = json!("../outside"),
            2 => changed[3]["message"]["content"][0] = json!({"type":"image","data":"fake"}),
            3 => changed[3]["message"]["toolCallId"] = json!("unmatched"),
            _ => {
                changed[3]["message"]
                    .as_object_mut()
                    .unwrap()
                    .remove("isError");
            }
        }
        let events = pi::parse(Cursor::new(lines(&changed)), &pi_options()).unwrap();
        let profile = report::run(&events, "native").unwrap();
        assert_eq!(profile.totals.reads, 0);
        assert_eq!(
            profile.import_coverage[0].failed_tool_results,
            u64::from(variant == 0)
        );
        assert_eq!(
            profile.import_coverage[0].unsupported_tool_results,
            u64::from(variant != 0)
        );
    }
}

#[test]
fn eof_does_not_seal_a_run_and_unbounded_requests_preserve_offsets() {
    let mut options = pi_options();
    options.complete = false;
    let events = pi::parse(Cursor::new(PI), &options).unwrap();
    assert!(!report::run(&events, "native").unwrap().complete);
    let mut values: Vec<Value> = PI
        .lines()
        .take(4)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let arguments = values[2]["message"]["content"][0]["arguments"]
        .as_object_mut()
        .unwrap();
    arguments.remove("limit");
    arguments.insert("offset".into(), json!(17));
    let events = pi::parse(Cursor::new(lines(&values)), &options).unwrap();
    let EventData::Read(read) = &events[0].data else {
        panic!("expected read");
    };
    assert_eq!(
        read.requested,
        Some(RequestedRange {
            start: 17,
            end: None
        })
    );
}
