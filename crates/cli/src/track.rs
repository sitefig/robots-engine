//! `susbot track`: fetch a list of domains, diff each against its stored
//! snapshot, update the snapshots, write a leaderboard, a Markdown summary
//! and social drafts, and post webhooks on change. Snapshots live in
//! `<data-dir>/<domain>/robots.txt` plus `meta.json`, so git history is the
//! change log.

use crate::net::{fetch_robots, iso_now, send_webhook, today};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use susbot_core::analyser::Verdict;
use susbot_core::diff::{diff_analyses, SemanticDiff};
use susbot_core::fetch::FetchInfo;
use susbot_core::i18n::Locale;
use susbot_core::{Analysis, Engine, Options};

#[derive(Args, Debug)]
pub struct TrackArgs {
    /// JSON list of sites: [{ "domain": "example.com", "name": "...", "category": "...", "enabled": true, "webhook_url": "..." }]
    #[arg(short, long, default_value = "config/famous-100.json")]
    pub config: PathBuf,
    /// Where snapshots are stored (one directory per domain)
    #[arg(short, long, default_value = "data/famous-100")]
    pub data_dir: PathBuf,
    /// TOML rules merged over the built-in config
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// Write social drafts for changed sites (Markdown)
    #[arg(long)]
    pub social_out: Option<PathBuf>,
    /// Write a Markdown digest of the run (for $GITHUB_STEP_SUMMARY)
    #[arg(long)]
    pub summary_out: Option<PathBuf>,
    /// Write the changed domains, one per line (for a commit message or a gate)
    #[arg(long)]
    pub changed_out: Option<PathBuf>,
    /// Slack or Discord incoming webhook for every change (a site's own webhook_url wins)
    #[arg(long)]
    pub webhook: Option<String>,
    /// Only post webhooks for high-impact changes (crawler flips, new high-severity paths, new errors)
    #[arg(long)]
    pub webhook_high_impact_only: bool,
    /// URL prefix for the snapshot directory, used in links (e.g. https://github.com/org/repo/tree/main/data/famous-100/)
    #[arg(long)]
    pub git_base_url: Option<String>,
    /// Title of the generated leaderboard README
    #[arg(long, default_value = "AI crawler leaderboard")]
    pub title: String,
    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,
    /// Parallel fetches
    #[arg(long, default_value = "16")]
    pub concurrency: usize,
    /// Fetch and diff without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SiteEntry {
    pub domain: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub notify_email: Option<String>,
    #[serde(default)]
    pub webhook_url: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SiteMeta {
    pub domain: String,
    pub robots_url: String,
    pub final_url: String,
    pub http_status: u16,
    pub bytes: usize,
    pub fetched_at: String,
    pub redirects: usize,
    /// Non-2xx answer: crawlers see no robots.txt, and so does the snapshot.
    pub unavailable: bool,
    /// The server answered with an HTML page (a soft 404 or a bot wall); stored as empty.
    pub html: bool,
}

struct Row {
    domain: String,
    name: String,
    category: String,
    verdicts: Vec<Verdict>,
    issues: usize,
    security: usize,
    status: u16,
    html: bool,
}

pub struct Outcome {
    pub checked: usize,
    pub failed: Vec<(String, String)>,
    pub changed: Vec<(SiteEntry, SemanticDiff)>,
    /// Sites that answered 403/429/5xx or an HTML page while a snapshot exists: kept as is.
    pub unavailable: Vec<(String, String)>,
}

/// A 403, 429, 5xx or HTML page is most likely a bot wall or an outage, not a
/// policy change: the previous snapshot is kept. 404 and 410 mean the file is
/// gone, which crawlers treat as "no rules", and that is a real change.
fn transient(status: u16, html: bool) -> bool {
    html || status == 403 || status == 429 || status >= 500
}

/// The crawlers shown as leaderboard columns: every AI crawler of the config.
fn columns(engine: &Engine) -> Vec<String> {
    let c = &engine.config.crawlers;
    c.list.iter().filter(|x| c.ai_categories.contains(&x.category)).map(|x| x.name.clone()).collect()
}

fn write_text(path: &Path, content: String) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))
}

/// Fetch every site on a small thread pool; results come back in list order.
fn fetch_all(sites: &[SiteEntry], ua: &str, timeout: u64, concurrency: usize) -> Vec<Result<FetchInfo, String>> {
    let queue = Arc::new(std::sync::Mutex::new((0usize, sites.iter().map(|s| s.domain.clone()).collect::<Vec<_>>())));
    let results = Arc::new(std::sync::Mutex::new((0..sites.len()).map(|_| None).collect::<Vec<Option<Result<FetchInfo, String>>>>()));
    let ua = ua.to_string();
    let handles: Vec<_> = (0..concurrency.max(1))
        .map(|_| {
            let (queue, results, ua) = (Arc::clone(&queue), Arc::clone(&results), ua.clone());
            std::thread::spawn(move || loop {
                let next = {
                    let mut q = queue.lock().unwrap();
                    if q.0 >= q.1.len() {
                        break;
                    }
                    let i = q.0;
                    q.0 += 1;
                    (i, q.1[i].clone())
                };
                let r = fetch_robots(&format!("https://{}", next.1), &ua, timeout);
                results.lock().unwrap()[next.0] = Some(r);
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    Arc::try_unwrap(results).ok().unwrap().into_inner().unwrap().into_iter().map(|r| r.unwrap_or_else(|| Err("not fetched".into()))).collect()
}

/// An HTML page served at /robots.txt: a soft 404 or a bot wall. Crawlers find
/// no rules in it, and storing it would make every fetch a "change".
pub fn looks_like_html(content_type: Option<&str>, text: &str) -> bool {
    if content_type.map(|c| c.to_ascii_lowercase().contains("text/html")).unwrap_or(false) {
        return true;
    }
    let head: String = text.trim_start().chars().take(64).collect::<String>().to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

fn read_snapshot(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("robots.txt")).ok()
}

fn write_snapshot(dir: &Path, text: &str, meta: &SiteMeta) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join("robots.txt"), text).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(meta).unwrap() + "\n").map_err(|e| e.to_string())
}

fn analyse(text: &str, origin: &str, engine: &Arc<Engine>, locale: &Arc<Locale>) -> Result<Analysis, String> {
    // No fetch info on either side, so the diff only reflects the text.
    Analysis::with_engine(text, Options { site_url: Some(format!("{origin}/robots.txt")), now: Some(iso_now()), ..Default::default() }, Arc::clone(engine), Arc::clone(locale))
}

pub fn run(cli: &TrackArgs) -> Result<ExitCode, String> {
    let raw = std::fs::read_to_string(&cli.config).map_err(|e| format!("{}: {e}", cli.config.display()))?;
    let sites: Vec<SiteEntry> = serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", cli.config.display()))?;
    let rules = match &cli.rules {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let engine = Arc::new(Engine::from_toml(rules.as_deref())?);
    let locale = Arc::new(Locale::english());
    let cols = columns(&engine);

    let mut outcome = Outcome { checked: 0, failed: Vec::new(), changed: Vec::new(), unavailable: Vec::new() };
    let mut rows: Vec<Row> = Vec::new();
    let enabled: Vec<SiteEntry> = sites.iter().filter(|s| s.enabled).cloned().collect();
    eprintln!("tracking {} domain(s){}", enabled.len(), if cli.dry_run { " (dry run)" } else { "" });
    let fetched = fetch_all(&enabled, &engine.config.crawlers.browser_ua, cli.timeout, cli.concurrency);

    for (site, fetch_result) in enabled.iter().zip(fetched) {
        outcome.checked += 1;
        let dir = cli.data_dir.join(&site.domain);
        let origin = format!("https://{}", site.domain);
        let f: FetchInfo = match fetch_result {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  {:<32} failed: {e}", site.domain);
                outcome.failed.push((site.domain.clone(), e));
                continue;
            }
        };
        // Like crawlers, treat a non-2xx answer (or an HTML page) as "no robots.txt".
        let html = looks_like_html(f.content_type.as_deref(), &f.text);
        let new_text = if f.usable() && !html { f.text.clone() } else { String::new() };
        let new = match analyse(&new_text, &origin, &engine, &locale) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("  {:<32} analysis error: {e}", site.domain);
                outcome.failed.push((site.domain.clone(), e));
                continue;
            }
        };
        let verdicts = cols.iter().map(|name| new.report.crawlers.iter().find(|c| &c.name == name).map(|c| c.verdict).unwrap_or(Verdict::Open)).collect();
        rows.push(Row {
            domain: site.domain.clone(),
            name: site.name.clone().unwrap_or_else(|| site.domain.clone()),
            category: site.category.clone().unwrap_or_else(|| "general".into()),
            verdicts,
            issues: new.report.summary.issues.errors + new.report.summary.issues.warnings,
            security: new.report.summary.security_findings,
            status: f.status,
            html,
        });
        let meta = SiteMeta { domain: site.domain.clone(), robots_url: f.robots_url.clone(), final_url: f.final_url.clone(), http_status: f.status, bytes: f.bytes, fetched_at: iso_now(), redirects: f.redirects.len(), unavailable: !f.usable(), html };

        match read_snapshot(&dir) {
            None => {
                // A first snapshot must be something crawlers actually saw: a
                // file, or a 404/410 (no file). A bot wall (403, 429, 5xx, an
                // HTML page) would seed an empty snapshot that misrepresents
                // the site and later hides the real file behind the
                // "transient failure" rule, so the site waits for a run that
                // can reach it.
                if transient(f.status, html) && f.status != 404 && f.status != 410 {
                    let why = if html { format!("HTTP {} HTML page", f.status) } else { format!("HTTP {}", f.status) };
                    eprintln!("  {:<32} {why} on first fetch, not seeded", site.domain);
                    outcome.unavailable.push((site.domain.clone(), format!("{why}, not seeded yet")));
                    continue;
                }
                eprintln!("  {:<32} HTTP {} new, seeding", site.domain, f.status);
                if !cli.dry_run {
                    write_snapshot(&dir, &new_text, &meta)?;
                }
            }
            Some(old_text) => {
                if transient(f.status, html) && !old_text.is_empty() {
                    let why = if html { format!("HTTP {} HTML page", f.status) } else { format!("HTTP {}", f.status) };
                    eprintln!("  {:<32} {why}, keeping the last snapshot", site.domain);
                    outcome.unavailable.push((site.domain.clone(), why));
                    continue;
                }
                let old = analyse(&old_text, &origin, &engine, &locale)?;
                let diff = diff_analyses(&old, &new);
                if !diff.is_changed {
                    eprintln!("  {:<32} HTTP {} unchanged", site.domain, f.status);
                    continue;
                }
                eprintln!("  {:<32} HTTP {} CHANGED ({} crawler flip(s), {} new sensitive path(s))", site.domain, f.status, diff.bot_changes.len(), diff.new_security_findings.len());
                if !cli.dry_run {
                    write_snapshot(&dir, &new_text, &meta)?;
                }
                let diff_url = cli.git_base_url.as_ref().map(|b| format!("{b}{}/robots.txt", site.domain));
                let webhook = site.webhook_url.as_deref().filter(|w| !w.is_empty()).or(cli.webhook.as_deref());
                if let Some(wh) = webhook {
                    if !cli.webhook_high_impact_only || diff.has_high_impact() {
                        if let Err(e) = send_webhook(wh, &diff.to_social_post(&site.domain, diff_url.as_deref()), cli.timeout) {
                            eprintln!("  {:<32} webhook failed: {e}", site.domain);
                        }
                    }
                }
                outcome.changed.push((site.clone(), diff));
            }
        }
    }

    eprintln!("checked {}, changed {}, unavailable {}, failed {}", outcome.checked, outcome.changed.len(), outcome.unavailable.len(), outcome.failed.len());

    if let Some(p) = &cli.social_out {
        let mut s = format!("# robots.txt changes, {}\n\n", today());
        if outcome.changed.is_empty() {
            s.push_str("No changes among the monitored domains today.\n");
        }
        for (site, d) in &outcome.changed {
            let diff_url = cli.git_base_url.as_ref().map(|b| format!("{b}{}/robots.txt", site.domain));
            s.push_str(&d.to_social_post(&site.domain, diff_url.as_deref()));
            s.push_str("\n---\n\n");
        }
        write_text(p, s)?;
    }
    if let Some(p) = &cli.summary_out {
        let mut s = format!("## robots.txt tracking, {}\n\n- checked: {}\n- changed: {}\n- unavailable (snapshot kept): {}\n- failed: {}\n\n", today(), outcome.checked, outcome.changed.len(), outcome.unavailable.len(), outcome.failed.len());
        for (site, d) in &outcome.changed {
            s.push_str(&d.to_markdown(&site.domain));
            s.push('\n');
        }
        if !outcome.unavailable.is_empty() {
            s.push_str("### Unavailable this run (last snapshot kept)\n\n");
            for (d, why) in &outcome.unavailable {
                s.push_str(&format!("- {d}: {why}\n"));
            }
            s.push('\n');
        }
        if !outcome.failed.is_empty() {
            s.push_str("### Not reachable\n\n");
            for (d, e) in &outcome.failed {
                s.push_str(&format!("- {d}: {e}\n"));
            }
        }
        write_text(p, s)?;
    }
    if let Some(p) = &cli.changed_out {
        write_text(p, outcome.changed.iter().map(|(s, _)| s.domain.clone()).collect::<Vec<_>>().join("\n") + "\n")?;
    }
    // The leaderboard only changes when a snapshot changed, so it is written
    // then (or when missing), which keeps quiet days out of git history.
    let readme = cli.data_dir.join("README.md");
    if !cli.dry_run && !rows.is_empty() && (!outcome.changed.is_empty() || !readme.exists()) {
        std::fs::create_dir_all(&cli.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(&readme, leaderboard(&cli.title, &cols, &rows)).map_err(|e| e.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

fn icon(v: Verdict) -> &'static str {
    match v {
        Verdict::Blocked => "Blocked",
        Verdict::Open => "Open",
        Verdict::Partial => "Restricted",
    }
}

fn leaderboard(title: &str, cols: &[String], rows: &[Row]) -> String {
    let total = rows.len().max(1);
    let mut s = format!("# {title}\n\n*Updated {}. Snapshots are in one directory per domain; git history is the change log.*\n\n", today());
    s.push_str("| Crawler | Blocked | Restricted | Open |\n| --- | ---: | ---: | ---: |\n");
    for (i, name) in cols.iter().enumerate() {
        let count = |v: Verdict| rows.iter().filter(|r| r.verdicts[i] == v).count();
        let (b, p, o) = (count(Verdict::Blocked), count(Verdict::Partial), count(Verdict::Open));
        s.push_str(&format!("| {name} | {b} ({:.0}%) | {p} | {o} |\n", b as f64 * 100.0 / total as f64));
    }
    s.push_str(&format!("\n| Domain | Category | {} | Issues | Sensitive paths |\n| --- | --- | {} | ---: | ---: |\n", cols.join(" | "), cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
    for r in rows {
        let label = if r.name != r.domain { format!("{} ({})", r.name, r.domain) } else { r.domain.clone() };
        let status = if r.html { " (HTML page)".to_string() } else if (200..300).contains(&r.status) { String::new() } else { format!(" (HTTP {})", r.status) };
        s.push_str(&format!("| [{label}](./{}/robots.txt){status} | {} | {} | {} | {} |\n", r.domain, r.category, r.verdicts.iter().map(|v| icon(*v)).collect::<Vec<_>>().join(" | "), r.issues, r.security));
    }
    s.push_str("\nGenerated by `susbot track` ([sus.bot](https://sus.bot), a free tool by [Sitefig](https://sitefig.eu)).\n");
    s
}
