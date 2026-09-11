//! Subjective feedback stays attributed, scoped, and separate from observed exposure. §FS-feedback

mod common;
use common::*;
use skopos::{event::*, feedback::*, splits};
use std::slice::from_ref;

fn source(run: &str) -> Event {
    let mut e = read(run, 1, "docs/spec.md");
    if let EventData::Read(r) = &mut e.data {
        r.delivered = Some(LineRange { start: 1, end: 100 });
        r.grund = Some(GroundRef {
            id: "FS-example".into(),
            section: Some("2".into()),
        });
    }
    e
}

fn feedback(e: &Event) -> Feedback {
    Feedback {
        schema: "skopos.feedback.v1".into(),
        feedback_id: format!("feedback:{}", e.run_id),
        run_id: e.run_id.clone(),
        repository: e.repository.clone(),
        revision: e.revision.clone().unwrap(),
        agent_id: e.agent_id.clone(),
        session_id: e.session_id.clone(),
        timestamp: e.timestamp.clone(),
        nominations: vec![Nomination {
            rank: 1,
            read_event_id: e.event_id.clone(),
            path: "docs/spec.md".into(),
            range: LineRange { start: 20, end: 40 },
            reason: Reason::ExcessDetail,
            confidence: SelfConfidence::High,
            suggestion: Suggestion::ExtractSection,
        }],
    }
}

#[test]
fn range_evidence_does_not_inherit_subjective_confidence() {
    for (delivered, requested, expected) in [
        (
            Some(LineRange { start: 1, end: 100 }),
            None,
            RangeEvidence::ExactDelivered,
        ),
        (
            None,
            Some(RequestedRange {
                start: 10,
                end: Some(50),
            }),
            RangeEvidence::RequestedOnly,
        ),
        (None, None, RangeEvidence::Unverified),
    ] {
        let mut e = source("r");
        if let EventData::Read(r) = &mut e.data {
            r.delivered = delivered;
            r.requested = requested;
        }
        let reports = vec![feedback(&e)];
        let events = complete(vec![e]);
        let valid = validate(&events, &reports).unwrap();
        assert_eq!(valid.nominations[0].range_evidence, expected);
        let result = splits::report(&events, &reports, "demo", "revision-1", 1, 10).unwrap();
        assert_eq!(
            result["candidates"][0]["evidence"][0]["self_reported_confidence"],
            "high"
        );
        assert_eq!(
            result["candidates"][0]["evidence"][0]["grund"]["section"],
            "2"
        );
    }
}

#[test]
fn rejects_wrong_identity_ranges_and_nonread_or_inferred_evidence() {
    let e = source("r");
    let good = feedback(&e);
    let events = complete(vec![e.clone()]);
    assert!(validate(from_ref(&e), from_ref(&good)).is_err());
    for field in [
        "run_id",
        "repository",
        "revision",
        "agent_id",
        "session_id",
        "schema",
    ] {
        let mut value = serde_json::to_value(&good).unwrap();
        value[field] = "wrong".into();
        let invalid = serde_json::from_value(value).unwrap();
        assert!(validate(&events, &[invalid]).is_err(), "{field}");
    }
    for (field, value) in [
        ("read_event_id", "missing"),
        ("read_event_id", "r:end"),
        ("path", "src/no.rs"),
    ] {
        let mut json = serde_json::to_value(&good).unwrap();
        json["nominations"][0][field] = value.into();
        assert!(validate(&events, &[serde_json::from_value(json).unwrap()]).is_err());
    }
    let mut out_of_bounds = good.clone();
    out_of_bounds.nominations[0].range.end = 101;
    assert!(validate(&events, &[out_of_bounds.clone()]).is_err());
    let mut requested = e.clone();
    if let EventData::Read(r) = &mut requested.data {
        r.delivered = None;
        r.requested = Some(RequestedRange {
            start: 1,
            end: Some(100),
        });
    }
    assert!(validate(&complete(vec![requested]), &[out_of_bounds]).is_err());
    let mut inferred = e.clone();
    if let EventData::Read(r) = &mut inferred.data {
        r.provenance.confidence = Confidence::Inferred;
    }
    assert!(validate(&complete(vec![inferred]), from_ref(&good)).is_err());
    let mut sibling = source("sibling");
    sibling.agent_id = "other-agent".into();
    let mut wrong = good.clone();
    wrong.nominations[0].read_event_id = sibling.event_id.clone();
    let all = [events, complete(vec![sibling])].concat();
    assert!(validate(&all, &[wrong]).is_err());
}

#[test]
fn ranks_duplicates_abstention_and_closed_wire_contract() {
    let e = source("r");
    let good = feedback(&e);
    let events = complete(vec![e]);
    assert_eq!(
        validate(&events, &[good.clone(), good.clone()])
            .unwrap()
            .reports
            .len(),
        1
    );
    let mut changed = good.clone();
    changed.nominations.clear();
    assert!(validate(&events, &[good.clone(), changed.clone()]).is_err());
    changed.feedback_id = "second-report".into();
    assert!(validate(&events, &[good.clone(), changed.clone()]).is_err());
    assert!(
        validate(&events, &[changed])
            .unwrap()
            .nominations
            .is_empty()
    );
    let mut bad = good.clone();
    bad.nominations[0].rank = 2;
    assert!(validate(&events, &[bad]).is_err());
    let mut duplicate = good.clone();
    duplicate.nominations.push(duplicate.nominations[0].clone());
    duplicate.nominations[1].rank = 2;
    assert!(validate(&events, &[duplicate]).is_err());
    let mut too_many = good.clone();
    too_many.nominations = (1..=6)
        .map(|rank| {
            let mut n = good.nominations[0].clone();
            n.rank = rank;
            n.range = LineRange {
                start: rank as u64,
                end: rank as u64,
            };
            n
        })
        .collect();
    assert!(validate(&events, &[too_many]).is_err());
    let mut wire = serde_json::to_value(&good).unwrap();
    wire["nominations"][0]["content"] = "do not store source".into();
    assert!(parse(serde_json::to_string(&wire).unwrap().as_bytes()).is_err());
    assert!(parse(b"{invalid}\n".as_slice()).is_err());
}

#[test]
fn recorded_edits_prevent_grouping_post_edit_coordinates() {
    let mut edit = source("r");
    edit.data = EventData::Edit {
        path: "docs/spec.md".into(),
    };
    let mut later = source("r");
    later.event_id = "r:2".into();
    later.sequence = 2;
    later.step = 2;
    let report = feedback(&later);
    assert!(
        validate(
            &complete(vec![edit.clone(), later.clone()]),
            from_ref(&report)
        )
        .is_err()
    );
    edit.data = EventData::Edit {
        path: "src/elsewhere.rs".into(),
    };
    assert!(validate(&complete(vec![edit, later]), &[report]).is_ok());
}

#[test]
fn task_support_revision_filters_abstentions_and_limit_are_explicit() {
    let mut events = Vec::new();
    let mut reports = Vec::new();
    for (run, task, revision, abstain) in [
        ("r1", "task-a", "revision-1", false),
        ("r2", "task-a", "revision-1", false),
        ("r3", "task-b", "revision-1", false),
        ("r4", "task-c", "revision-1", true),
        ("r5", "task-d", "revision-2", false),
    ] {
        let mut e = source(run);
        e.task_id = task.into();
        e.revision = Some(revision.into());
        let mut r = feedback(&e);
        if abstain {
            r.nominations.clear();
        } else {
            let mut next = r.nominations[0].clone();
            next.rank = 2;
            next.range.end = 41;
            r.nominations.push(next);
        }
        events.extend(complete(vec![e]));
        reports.push(r);
    }
    reports.push(reports[0].clone());
    let result = splits::report(&events, &reports, "demo", "revision-1", 2, 1).unwrap();
    assert_eq!(result["reports_considered"], 4);
    assert_eq!(result["distinct_tasks_reporting"], 3);
    assert_eq!(result["abstaining_reports"], 1);
    assert_eq!(result["nominated_spans"], 2);
    assert_eq!(result["eligible_spans"], 2);
    assert_eq!(result["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(result["candidates"][0]["distinct_tasks"], 2);
    assert_eq!(result["candidates"][0]["report_support"], 3);
    assert_eq!(result["candidates"][0]["range"]["end"], 40);
    assert_eq!(
        splits::report(&events, &reports, "demo", "revision-1", 3, 10).unwrap()["eligible_spans"],
        0
    );
    assert_eq!(
        splits::report(&events, &reports, "demo", "missing", 1, 10).unwrap()["reports_considered"],
        0
    );
    assert_eq!(
        splits::report(&events, &[], "demo", "revision-1", 1, 10).unwrap()["nominated_spans"],
        0
    );
    assert!(splits::report(&events, &reports, "demo", "revision-1", 0, 10).is_err());
    assert!(splits::report(&events, &reports, "demo", "revision-1", 1, 0).is_err());
    let mut invalid_unselected = reports[4].clone();
    invalid_unselected.nominations[0].read_event_id = "missing".into();
    assert!(splits::report(&events, &[invalid_unselected], "demo", "revision-1", 1, 10).is_err());
}

#[test]
fn cli_joins_sidecar_without_mutating_ledger_and_errors_without_partial_output() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("ledger.sqlite3");
    let sidecar = dir.path().join("feedback.jsonl");
    let e = source("r");
    let report = feedback(&e);
    skopos::store::import(&db, &complete(vec![e])).unwrap();
    std::fs::write(&sidecar, serde_json::to_string(&report).unwrap()).unwrap();
    let before = std::fs::read(&db).unwrap();
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_skopos"))
            .arg("--db")
            .arg(&db)
            .arg("split-candidates")
            .arg(&sidecar)
            .args([
                "--repository",
                "demo",
                "--revision",
                "revision-1",
                "--min-tasks",
                "1",
            ])
            .output()
            .unwrap()
    };
    let output = invoke();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema"], "skopos.splits.v1");
    assert_eq!(
        result["candidates"][0]["evidence"][0]["range_evidence"],
        "exact_delivered"
    );
    assert_eq!(std::fs::read(&db).unwrap(), before);
    std::fs::write(
        &sidecar,
        format!("{}\n{{invalid}}", serde_json::to_string(&report).unwrap()),
    )
    .unwrap();
    let invalid = invoke();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert_eq!(std::fs::read(&db).unwrap(), before);
}
