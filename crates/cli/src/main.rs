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

fn main() -> ExitCode {
    // Backwards compatibility: `susbot <target> ...` means `susbot audit <target> ...`.
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(first) = args.get(1) {
        let is_flag = first.starts_with('-');
        if !SUBCOMMANDS.contains(&first.as_str()) && (!is_flag || first == "--print-default-config") {
            args.insert(1, "audit".into());
        }
    }
    let cli = Cli::parse_from(args);
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
            ExitCode::from(2)
        }
    }
}
