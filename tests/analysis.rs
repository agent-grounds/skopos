//! Context boundaries and hand-computed association fixtures. §FS-analysis
mod common;

use common::*;
use skopos::{event::*, graph, report};

#[test]
fn rereads_respect_edits_compaction_agents_and_ranges() {
    let first = read("run", 1, "a.rs");
    let second = read("run", 2, "a.rs");
    let mut edit = read("run", 3, "a.rs");
    edit.data = EventData::Edit {
        path: "a.rs".into(),
    };
    let after_edit = read("run", 4, "a.rs");
    let mut reset = read("run", 5, "a.rs");
    reset.epoch = 1;
    reset.data = EventData::ContextReset;
    let mut after_reset = read("run", 6, "a.rs");
    after_reset.epoch = 1;
    let mut child = read("run", 1, "a.rs");
    child.event_id = "child".into();
    child.agent_id = "child-agent".into();
    let events = complete(vec![
        first,
        second,
        edit,
        after_edit,
        reset,
        after_reset,
        child,
    ]);
    skopos::validation::history(&events).unwrap();
    let result = report::run(&events, "run").unwrap();
    assert_eq!(result.totals.reads, 5);
    assert_eq!(result.totals.identical_reread_candidates, 1);
    assert_eq!(result.totals.identical_returned_bytes, 10);
    assert_eq!(result.totals.missing_token_estimates, 5);
    let mut ranges = vec![read("ranges", 1, "a.rs"), read("ranges", 2, "a.rs")];
    for (index, event) in ranges.iter_mut().enumerate() {
        let EventData::Read(data) = &mut event.data else {
            unreachable!()
        };
        data.delivered = Some(LineRange {
            start: index as u64 + 1,
            end: index as u64 + 1,
        });
    }
    assert_eq!(
        report::run(&ranges, "ranges")
            .unwrap()
            .totals
            .identical_reread_candidates,
        0
    );
}

#[test]
fn graph_matches_known_probabilities_and_excludes_partial_other_revisions_and_inference() {
    let mut events = complete(vec![
        read("one", 1, "a"),
        read("one", 2, "b"),
        read("one", 3, "a"),
    ]);
    events.extend(complete(vec![read("two", 1, "a"), read("two", 2, "b")]));
    events.extend(complete(vec![read("three", 1, "a")]));
    events.extend(complete(vec![read("four", 1, "c")]));
    events.push(read("partial", 1, "b"));
    let mut other = complete(vec![read("other", 1, "b")]);
    for event in &mut other {
        event.revision = Some("revision-2".into());
    }
    events.extend(other);
    let mut inferred = read("inferred", 1, "b");
    let EventData::Read(data) = &mut inferred.data else {
        unreachable!()
    };
    data.provenance.confidence = Confidence::Inferred;
    data.provenance.method = Method::ShellInference;
    events.extend(complete(vec![inferred]));
    let graph = graph::build(&events, "demo", "revision-1", 2).unwrap();
    assert_eq!(graph.samples, 4);
    assert_eq!(graph.distinct_tasks, 4);
    let edge = graph
        .edges
        .iter()
        .find(|e| e.source == "a" && e.target == "b")
        .unwrap();
    assert_eq!((edge.support, edge.distinct_tasks), (2, 2));
    assert_eq!((edge.source_samples, edge.target_samples), (3, 2));
    assert!((edge.conditional_probability - 2.0 / 3.0).abs() < 1e-12);
    assert!((edge.jaccard - 2.0 / 3.0).abs() < 1e-12);
    assert!((edge.lift - 4.0 / 3.0).abs() < 1e-12);
    assert_eq!(edge.next_step_support, 2);
}

#[test]
fn siblings_never_create_pairs_and_same_batch_has_no_direction() {
    let mut child = read("one", 1, "b");
    child.event_id = "child".into();
    child.agent_id = "child".into();
    let events = complete(vec![read("one", 1, "a"), child]);
    let result = graph::build(&events, "demo", "revision-1", 1).unwrap();
    assert_eq!(result.samples, 2);
    assert!(result.edges.is_empty());
    let mut parallel = read("two", 2, "b");
    parallel.step = 1;
    let events = complete(vec![read("two", 1, "a"), parallel]);
    let result = graph::build(&events, "demo", "revision-1", 1).unwrap();
    assert_eq!(result.edges.len(), 2);
    assert!(result.edges.iter().all(|e| e.next_step_support == 0));
}

#[test]
fn empty_graph_and_overflow_are_explicit() {
    let graph = graph::build(&[], "demo", "revision-1", 1).unwrap();
    assert_eq!(graph.samples, 0);
    assert!(graph.edges.is_empty());
    assert!(graph::build(&[], "demo", "revision-1", 0).is_err());
    let mut large = read("one", 1, "a");
    let EventData::Read(data) = &mut large.data else {
        unreachable!()
    };
    data.bytes = Some(u64::MAX);
    assert!(report::run(&[large, read("one", 2, "b")], "one").is_err());
}
