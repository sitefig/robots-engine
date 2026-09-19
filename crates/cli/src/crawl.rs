//! `susbot crawl`: bulk audit of many domains into a compressed JSON Lines
//! dataset (one record per domain), with a summary. Input is a text or CSV
//! list (Tranco style `rank,domain`, or one domain per line). Fetching runs
//! on a pool of threads sharing one engine.

use crate::net::{fetch_robots, fnv1a, iso_now};
use clap::Args;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use susbot_core::i18n::Locale;
use susbot_core::{Analysis, Engine, Options};

#[derive(Args, Debug)]
pub struct CrawlArgs {
    /// Domain list: one per line, or CSV whose last column is the domain (Tranco format)
    #[arg(short, long)]
    pub input: PathBuf,
    /// Output file (gzip-compressed JSON Lines)
    #[arg(short, long, default_value = "susbot-census.jsonl.gz")]
    pub out: PathBuf,
    /// Summary JSON with counts per crawler verdict, platform and status
    #[arg(long)]
    pub summary: Option<PathBuf>,
    /// Stop after this many domains
    #[arg(long)]
    pub limit: Option<usize>,
    /// Skip the first N domains (to resume)
    #[arg(long, default_value = "0")]
    pub offset: usize,
    /// Parallel fetches
    #[arg(long, default_value = "32")]
    pub concurrency: usize,
    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,
    /// TOML rules merged over the built-in config
    #[arg(long)]
    pub rules: Option<PathBuf>,
}

#[derive(Serialize)]
struct Record {
    domain: String,
    fetched_at: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    status: u16,
    final_url: String,
    redirects: usize,
    bytes: usize,
    hash: String,
    groups: usize,
    rules: usize,
    sitemaps: usize,
    errors: usize,
    warnings: usize,
    security_findings: usize,
    platform: Option<String>,
    /// crawler name -> open | partial | blocked
    crawlers: BTreeMap<String, String>,
    ai_training_blocked: usize,
    ai_training_total: usize,
}

pub fn read_domains(path: &PathBuf) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let domain = line.rsplit(',').next().unwrap_or(line).trim().trim_matches('"').to_lowercase();
        if domain.contains('.') && !domain.contains('/') && !out.contains(&domain) {
            out.push(domain);
        }
    }
    Ok(out)
}

fn audit_one(domain: &str, engine: &Arc<Engine>, locale: &Arc<Locale>, timeout: u64) -> Record {
    let origin = format!("https://{domain}");
    let fetched_at = iso_now();
    let f = match fetch_robots(&origin, &engine.config.crawlers.browser_ua, timeout) {
        Ok(f) => f,
        Err(e) => {
            return Record { domain: domain.into(), fetched_at, ok: false, error: Some(e), status: 0, final_url: String::new(), redirects: 0, bytes: 0, hash: String::new(), groups: 0, rules: 0, sitemaps: 0, errors: 0, warnings: 0, security_findings: 0, platform: None, crawlers: BTreeMap::new(), ai_training_blocked: 0, ai_training_total: 0 };
        }
    };
    let text = f.text.clone();
    let a = Analysis::with_engine(&text, Options { site_url: Some(f.final_url.clone()), fetch: Some(f.clone()), now: Some(fetched_at.clone()), ..Default::default() }, Arc::clone(engine), Arc::clone(locale));
    match a {
        Ok(a) => {
            let r = &a.report;
            Record {
                domain: domain.into(),
                fetched_at,
                ok: true,
                error: None,
                status: f.status,
                final_url: f.final_url.clone(),
                redirects: f.redirects.len(),
                bytes: f.bytes,
                hash: fnv1a(&r.raw),
                groups: r.summary.groups,
                rules: r.summary.rules,
                sitemaps: r.summary.sitemaps,
                errors: r.summary.issues.errors,
                warnings: r.summary.issues.warnings,
                security_findings: r.summary.security_findings,
                platform: r.summary.platform.clone(),
                crawlers: r.crawlers.iter().map(|c| (c.name.clone(), c.verdict.as_str().to_string())).collect(),
                ai_training_blocked: r.summary.ai_training.blocked,
                ai_training_total: r.summary.ai_training.total,
            }
        }
        Err(e) => Record { domain: domain.into(), fetched_at, ok: false, error: Some(e), status: f.status, final_url: f.final_url, redirects: f.redirects.len(), bytes: f.bytes, hash: String::new(), groups: 0, rules: 0, sitemaps: 0, errors: 0, warnings: 0, security_findings: 0, platform: None, crawlers: BTreeMap::new(), ai_training_blocked: 0, ai_training_total: 0 },
    }
}

#[derive(Serialize, Default)]
struct Summary {
    domains: usize,
    fetched: usize,
    unreachable: usize,
    with_robots: usize,
    status: BTreeMap<String, usize>,
    platforms: BTreeMap<String, usize>,
    /// crawler -> verdict -> count
    crawlers: BTreeMap<String, BTreeMap<String, usize>>,
    generated_at: String,
}

pub fn run(cli: &CrawlArgs) -> Result<ExitCode, String> {
    let domains = read_domains(&cli.input)?;
    let domains: Vec<String> = domains.into_iter().skip(cli.offset).take(cli.limit.unwrap_or(usize::MAX)).collect();
    let total = domains.len();
    eprintln!("crawling {total} domain(s) with {} worker(s)", cli.concurrency.max(1));
    let rules = match &cli.rules {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let engine = Arc::new(Engine::from_toml(rules.as_deref())?);
    let locale = Arc::new(Locale::english());
    let queue = Arc::new(Mutex::new(domains.into_iter().collect::<VecDeque<String>>()));
    let (tx, rx) = std::sync::mpsc::channel::<Record>();
    let mut handles = Vec::new();
    for _ in 0..cli.concurrency.max(1) {
        let (queue, tx, engine, locale, timeout) = (Arc::clone(&queue), tx.clone(), Arc::clone(&engine), Arc::clone(&locale), cli.timeout);
        handles.push(std::thread::spawn(move || loop {
            let next = queue.lock().unwrap().pop_front();
            let Some(domain) = next else { break };
            let _ = tx.send(audit_one(&domain, &engine, &locale, timeout));
        }));
    }
    drop(tx);

    let file = std::fs::File::create(&cli.out).map_err(|e| format!("{}: {e}", cli.out.display()))?;
    let mut gz = GzEncoder::new(std::io::BufWriter::new(file), Compression::default());
    let mut summary = Summary { domains: total, generated_at: iso_now(), ..Default::default() };
    let mut done = 0usize;
    for rec in rx {
        done += 1;
        if done % 100 == 0 || done == total {
            eprintln!("  {done}/{total}");
        }
        if rec.ok {
            summary.fetched += 1;
            if (200..300).contains(&rec.status) {
                summary.with_robots += 1;
            }
            *summary.status.entry(rec.status.to_string()).or_default() += 1;
            if let Some(p) = &rec.platform {
                *summary.platforms.entry(p.clone()).or_default() += 1;
            }
            for (name, verdict) in &rec.crawlers {
                *summary.crawlers.entry(name.clone()).or_default().entry(verdict.clone()).or_default() += 1;
            }
        } else {
            summary.unreachable += 1;
        }
        serde_json::to_writer(&mut gz, &rec).map_err(|e| e.to_string())?;
        gz.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    for h in handles {
        let _ = h.join();
    }
    gz.finish().map_err(|e| e.to_string())?;
    if let Some(p) = &cli.summary {
        std::fs::write(p, serde_json::to_string_pretty(&summary).unwrap() + "\n").map_err(|e| format!("{}: {e}", p.display()))?;
    }
    eprintln!("wrote {} ({} fetched, {} unreachable)", cli.out.display(), summary.fetched, summary.unreachable);
    Ok(ExitCode::SUCCESS)
}
