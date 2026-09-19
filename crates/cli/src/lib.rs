//! susbot: audit, diff, track and crawl robots.txt from the command line or
//! GitHub Actions.
//!
//! - `audit <url|file|->`: one file, every check, any export format.
//! - `diff <old> <new>`: semantic diff of two files (crawler flips, new
//!   sensitive paths, issues, sitemaps) plus a unified text diff.
//! - `track --config list.json --data-dir dir`: fetch a list of domains,
//!   diff against the stored snapshots, write snapshots, a leaderboard, a
//!   summary and social drafts, and post webhooks on change.
//! - `crawl --input domains.csv --out file.jsonl.gz`: concurrent bulk audit
//!   of many domains into a compressed JSON Lines dataset.
//!
//! `susbot <target>` without a subcommand still works and means `audit`.
//!
//! The library exists so the `susbot` crate and the Python package can ship
//! the same program; it has no API beyond [`run`] and [`run_with`].

mod audit;
mod crawl;
mod diff_cmd;
mod net;
mod track;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "susbot", version = susbot_core::VERSION, about = "robots.txt audit, diff, tracking and bulk crawl (sus.bot)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Audit one robots.txt: lint, crawler access, security exposure, reconnaissance
    Audit(audit::AuditArgs),
    /// Semantic diff of two robots.txt files
    Diff(diff_cmd::DiffArgs),
    /// Fetch a list of domains, diff against stored snapshots, alert and publish a leaderboard
    Track(track::TrackArgs),
    /// Bulk audit of many domains into a JSON Lines dataset
    Crawl(crawl::CrawlArgs),
}

const SUBCOMMANDS: [&str; 5] = ["audit", "diff", "track", "crawl", "help"];

/// Runs the CLI with the process arguments; the `susbot` binaries of
/// `susbot-cli` and of the `susbot` crate are both this function.
pub fn run() -> ExitCode {
    ExitCode::from(run_with(std::env::args()))
}

/// Runs the CLI with the given arguments (the first is the program name) and
/// returns the exit code: 0 on success, 1 when a `--fail-on` threshold is
/// met, 2 on errors. Help and version requests print and return 0 instead of
/// exiting the process, so the Python package can call this in-process.
pub fn run_with<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    // Backwards compatibility: `susbot <target> ...` means `susbot audit <target> ...`.
    let mut args: Vec<String> = args.into_iter().map(Into::into).collect();
    if let Some(first) = args.get(1) {
        let is_flag = first.starts_with('-');
        if !SUBCOMMANDS.contains(&first.as_str()) && (!is_flag || first == "--print-default-config") {
            args.insert(1, "audit".into());
        }
    }
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // Help and version go to stdout with code 0; usage errors to stderr with 2.
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    let result = match &cli.command {
        Command::Audit(a) => audit::run(a),
        Command::Diff(a) => diff_cmd::run(a),
        Command::Track(a) => track::run(a),
        Command::Crawl(a) => crawl::run(a),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("susbot: {e}");
            2
        }
    }
}
