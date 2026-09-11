//! JSON CLI for offline ingestion and inspection. §FS-ledger.2

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use skopos::{feedback, graph, parse_events, pi, report, splits, store, validation};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Profile observed context acquisition; experimental, offline, JSON output"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".skopos/ledger.sqlite3")]
    db: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Atomically import normalized skopos.event.v1 JSONL (use - for stdin).
    Import { file: String },
    /// Import one branch of a frozen Pi v3 session (recorded tool returns only).
    ImportPi {
        file: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        run: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        revision: String,
        #[arg(long)]
        leaf: Option<String>,
        #[arg(long)]
        complete: bool,
    },
    /// Inspect recorded events and context boundaries for a run.
    Reads { run: String },
    /// Summarize files, provenance, and identical returned-text candidates.
    #[command(visible_alias = "files")]
    Report { run: String },
    /// Compare observed profiles; does not establish an optimization effect.
    Compare { run_a: String, run_b: String },
    /// List stored runs and completion states.
    Status,
    /// Show directional associations from one repository-relative path.
    Related {
        path: String,
        #[command(flatten)]
        selection: Selection,
    },
    /// Show associations for one historical repository revision.
    Graph(Selection),
    /// Join agent-reported unused spans to reads; output file/chapter split review candidates.
    SplitCandidates {
        file: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        revision: String,
        #[arg(long, default_value_t = 2)]
        min_tasks: usize,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Args)]
struct Selection {
    #[arg(long)]
    repository: String,
    #[arg(long)]
    revision: String,
    #[arg(long, default_value_t = 2)]
    min_support: usize,
}

fn input(file: &str) -> Result<Box<dyn BufRead>> {
    if file == "-" {
        Ok(Box::new(BufReader::new(io::stdin())))
    } else {
        Ok(Box::new(BufReader::new(
            File::open(file).context("opening input")?,
        )))
    }
}

fn execute(cli: Cli) -> Result<Value> {
    match cli.command {
        Command::Import { file } => {
            let events = parse_events(input(&file)?)?;
            return Ok(serde_json::to_value(store::import(&cli.db, &events)?)?);
        }
        Command::ImportPi {
            file,
            repository,
            run,
            task,
            revision,
            leaf,
            complete,
        } => {
            let task = task.unwrap_or_else(|| run.clone());
            let options = pi::Options {
                repository,
                run,
                task,
                revision,
                leaf,
                complete,
            };
            let events = pi::parse(input(&file)?, &options)?;
            return Ok(serde_json::to_value(store::import(&cli.db, &events)?)?);
        }
        _ => {}
    }
    let events = store::load(&cli.db)?;
    Ok(match cli.command {
        Command::Reads { run } => {
            report::run(&events, &run)?;
            json!({"schema":"skopos.reads.v1", "run_id":run,
                "events":events.iter().filter(|event| event.run_id == run).collect::<Vec<_>>()})
        }
        Command::Report { run } => serde_json::to_value(report::run(&events, &run)?)?,
        Command::Compare { run_a, run_b } => {
            let before = report::run(&events, &run_a)?;
            let after = report::run(&events, &run_b)?;
            let delta = i64::try_from(
                i128::from(after.totals.known_returned_bytes)
                    - i128::from(before.totals.known_returned_bytes),
            )
            .context("byte delta exceeds the supported JSON integer range")?;
            json!({"schema":"skopos.compare.v1", "before":before, "after":after,
                "known_returned_bytes_delta":delta,
                "interpretation":"Descriptive comparison; tasks, coverage, and outcomes may differ. Not billed cost."})
        }
        Command::Status => {
            let ids: BTreeSet<_> = events.iter().map(|e| e.run_id.as_str()).collect();
            let runs: Vec<_> = ids.into_iter().map(|id| {
                let profile = report::run(&events, id)?;
                Ok(json!({"run_id":id,"repository":profile.repository,"revision":profile.revision,
                    "complete":profile.complete,"asserted_outcome":profile.asserted_outcome,
                    "reads":profile.totals.reads}))
            }).collect::<Result<_>>()?;
            json!({"schema":"skopos.status.v1","events":events.len(),"runs":runs})
        }
        Command::Related { path, selection } => {
            validation::relative_path(&path)?;
            let mut result = graph::build(
                &events,
                &selection.repository,
                &selection.revision,
                selection.min_support,
            )?;
            result.edges.retain(|edge| edge.source == path);
            serde_json::to_value(result)?
        }
        Command::Graph(selection) => serde_json::to_value(graph::build(
            &events,
            &selection.repository,
            &selection.revision,
            selection.min_support,
        )?)?,
        // §FS-feedback.3: Feedback is a sidecar, never a mutation of sealed observations.
        Command::SplitCandidates {
            file,
            repository,
            revision,
            min_tasks,
            limit,
        } => splits::report(
            &events,
            &feedback::parse(input(&file)?)?,
            &repository,
            &revision,
            min_tasks,
            limit,
        )?,
        Command::Import { .. } | Command::ImportPi { .. } => unreachable!(),
    })
}

fn main() {
    if let Err(error) = execute(Cli::parse()).and_then(|value| {
        serde_json::to_writer_pretty(io::stdout().lock(), &value)?;
        println!();
        Ok(())
    }) {
        eprintln!("skopos: {error:#}");
        std::process::exit(1);
    }
}
