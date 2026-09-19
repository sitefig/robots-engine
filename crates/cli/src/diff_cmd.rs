//! `susbot diff`: semantic diff of two robots.txt files.

use clap::{Args, ValueEnum};
use std::path::PathBuf;
use susbot_core::diff::diff_analyses;
use susbot_core::{Analysis, Options};

#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum DiffFormat {
    Markdown,
    Json,
    /// Short text for social media or chat
    Social,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// The older robots.txt
    pub old_file: PathBuf,
    /// The newer robots.txt
    pub new_file: PathBuf,
    /// Domain shown in the output
    #[arg(short, long, default_value = "site")]
    pub domain: String,
    /// Site URL assumed for both files, enabling origin-dependent checks
    #[arg(long)]
    pub site_url: Option<String>,
    /// TOML config merged over the built-in rules
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    #[arg(short, long, value_enum, default_value = "markdown")]
    pub format: DiffFormat,
    /// Link to the commit or diff, included in the social text
    #[arg(long)]
    pub diff_url: Option<String>,
    /// Exit 1 when the files differ (for CI gates)
    #[arg(long)]
    pub fail_on_change: bool,
}

pub fn run(cli: &DiffArgs) -> Result<u8, String> {
    let old_text = std::fs::read_to_string(&cli.old_file).map_err(|e| format!("{}: {e}", cli.old_file.display()))?;
    let new_text = std::fs::read_to_string(&cli.new_file).map_err(|e| format!("{}: {e}", cli.new_file.display()))?;
    let config = match &cli.config {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let options = || Options { site_url: cli.site_url.clone(), config: config.clone(), ..Default::default() };
    let old = Analysis::new(&old_text, options())?;
    let new = Analysis::new(&new_text, options())?;
    let diff = diff_analyses(&old, &new);
    match cli.format {
        DiffFormat::Social => println!("{}", diff.to_social_post(&cli.domain, cli.diff_url.as_deref())),
        DiffFormat::Json => println!("{}", serde_json::to_string_pretty(&diff).map_err(|e| e.to_string())?),
        DiffFormat::Markdown => print!("{}", diff.to_markdown(&cli.domain)),
    }
    Ok(if cli.fail_on_change && diff.is_changed { 1 } else { 0 })
}
