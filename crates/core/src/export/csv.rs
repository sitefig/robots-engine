//! CSV and TSV exports built from the report. Four "tabs" mirror what an SEO
//! would want as sheets; because CSV has no tabs there are two forms: one
//! file per tab, or one tagged file whose first column names the sheet.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::RuleType;
use crate::params;
use crate::report::Report;
use serde::Serialize;

pub const TABS: [&str; 4] = ["rules", "sitemaps", "issues", "recon"];

#[derive(Debug, Clone, Serialize)]
pub struct Tab {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tabs {
    pub rules: Tab,
    pub sitemaps: Tab,
    pub issues: Tab,
    pub recon: Tab,
}

impl Tabs {
    pub fn get(&self, tab: &str) -> Option<&Tab> {
        match tab {
            "rules" => Some(&self.rules),
            "sitemaps" => Some(&self.sitemaps),
            "issues" => Some(&self.issues),
            "recon" => Some(&self.recon),
            _ => None,
        }
    }
}

pub fn tab_labels(locale: &Locale) -> Vec<(String, String)> {
    TABS.iter().map(|t| (t.to_string(), locale.s(&format!("csv.tab.{t}")))).collect()
}

fn join(v: &[String]) -> String {
    v.join("; ")
}

pub fn csv_cell(s: &str) -> String {
    if s.contains(['"', ',', '\r', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

pub fn to_csv(header: &[String], rows: &[Vec<String>]) -> String {
    let mut lines: Vec<String> = vec![header.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(",")];
    for r in rows {
        lines.push(r.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
    }
    lines.join("\r\n") + "\r\n"
}

/// Tab-separated, for pasting straight into Google Sheets or Excel.
pub fn to_tsv(header: &[String], rows: &[Vec<String>]) -> String {
    let cell = |s: &str| s.replace(['\t', '\r', '\n'], " ");
    let mut lines: Vec<String> = vec![header.iter().map(|c| cell(c)).collect::<Vec<_>>().join("\t")];
    for r in rows {
        lines.push(r.iter().map(|c| cell(c)).collect::<Vec<_>>().join("\t"));
    }
    lines.join("\n") + "\n"
}

fn opt(v: Option<u32>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

pub fn tab_rows(r: &Report, engine: &Engine, l: &Locale) -> Tabs {
    let h = |keys: &[&str]| -> Vec<String> { keys.iter().map(|k| l.s(&format!("csv.{k}"))).collect() };
    let rules = Tab {
        header: h(&["rules.userAgents", "rules.group", "rules.type", "rules.path", "line", "notes"]),
        rows: r.rules.iter().map(|x| vec![x.user_agents.join(", "), x.group.to_string(), x.rule_type.label().into(), x.path.clone(), x.line.to_string(), join(&x.notes)]).collect(),
    };
    let src = &r.source;
    let source_status = match src.http_status {
        Some(s) => l.t("csv.source.http", &params! {"status" => s}),
        None if src.fetched_via == "example" => l.s("csv.source.example"),
        None => l.s("csv.source.pasted"),
    };
    let mut source_notes: Vec<String> = vec![l.t("csv.source.via", &params! {"via" => src.fetched_via})];
    if !src.redirects.is_empty() {
        source_notes.push(l.t("csv.source.redirects", &params! {"chain" => src.redirects.iter().map(|x| format!("{} → {}", x.status.map(|s| s.to_string()).unwrap_or_default(), x.to)).collect::<Vec<_>>().join(" ; ")}));
    }
    if src.redirect_limit {
        source_notes.push(l.s("csv.source.redirectLimit"));
    }
    if src.truncated {
        source_notes.push(l.s("csv.source.truncated"));
    }
    let mut sitemap_rows = vec![vec!["robots.txt".to_string(), src.final_url.clone().or(src.site_url.clone()).unwrap_or_default(), String::new(), source_status, src.content_type.clone().unwrap_or_default(), join(&source_notes)]];
    for s in &r.sitemaps {
        sitemap_rows.push(vec!["Sitemap".into(), s.url.clone(), s.line.to_string(), if s.valid { l.s("csv.validUrl") } else { l.s("csv.invalidUrl") }, String::new(), join(&s.notes)]);
    }
    let sitemaps = Tab { header: h(&["sitemaps.kind", "sitemaps.url", "line", "sitemaps.status", "sitemaps.contentType", "notes"]), rows: sitemap_rows };
    let issues = Tab {
        header: h(&["issues.level", "issues.kind", "line", "issues.message"]),
        rows: r.issues.iter().map(|i| vec![l.s(&format!("csv.level.{}", i.level.as_str())), l.s(&format!("enum.kind.{}", i.kind.as_str())), opt(i.line), i.message.clone()]).collect(),
    };
    let rec = &r.recon;
    let cat = |k: &str| l.s(&format!("csv.cat.{k}"));
    let mut rows: Vec<Vec<String>> = Vec::new();
    for d in &rec.stack.detections {
        rows.push(vec![cat("platform"), d.name.clone(), l.t("csv.detail.confidence", &params! {"kind" => l.s(&format!("recon.cms.kind.{}", d.kind)), "confidence" => l.s(&format!("enum.confidence.{}", d.confidence))}), opt(d.evidence.first().map(|e| e.line)), join(&d.evidence.iter().map(|e| e.text.clone()).collect::<Vec<_>>())]);
    }
    for t in &rec.tech {
        rows.push(vec![cat("tech"), t.clone(), l.s("csv.detail.fromExtensions"), String::new(), String::new()]);
    }
    for c in &rec.cloud {
        let bucket = c.bucket.as_ref().map(|b| l.t("csv.detail.bucket", &params! {"bucket" => b})).unwrap_or_default();
        rows.push(vec![cat("cloud"), c.provider.clone(), format!("{}{bucket}", l.s(&format!("recon.cloud.kind.{}", c.kind))), c.line.to_string(), c.host.clone()]);
    }
    for x in &rec.hosts.hosts {
        let env = x.env.as_ref().map(|e| format!(", {e}")).unwrap_or_default();
        let sources: Vec<String> = x.sources.iter().map(|s| l.t("csv.detail.sourceLine", &params! {"source" => if s.source == "comment" { l.s("enum.source.comment") } else { s.source.clone() }, "line" => s.line})).collect();
        rows.push(vec![cat("host"), x.host.clone(), format!("{}{env}", l.s(&format!("enum.relation.{}", x.relation))), opt(x.sources.first().map(|s| s.line)), join(&sources)]);
    }
    for p in &rec.hosts.paths {
        rows.push(vec![cat("envPath"), p.path.clone(), p.hint.clone(), p.line.to_string(), String::new()]);
    }
    for a in &rec.api {
        let v = a.version.as_ref().map(|v| format!(", {v}")).unwrap_or_default();
        rows.push(vec![cat("api"), a.path.clone(), format!("{}{v}", l.s(&format!("recon.api.kind.{}", a.kind))), a.line.to_string(), String::new()]);
    }
    for f in &rec.data.feeds {
        rows.push(vec![cat("feed"), f.path.clone(), f.kind.clone(), f.line.to_string(), String::new()]);
    }
    for f in &rec.data.portals {
        rows.push(vec![cat("portal"), f.path.clone(), f.kind.clone(), f.line.to_string(), String::new()]);
    }
    for f in &rec.data.search.paths {
        rows.push(vec![cat("searchPath"), f.path.clone(), l.s("csv.detail.internalSearch"), f.line.to_string(), String::new()]);
    }
    for p in &rec.data.search.params {
        rows.push(vec![cat("param"), p.name.clone(), l.s(&format!("enum.role.{}", p.role)), opt(p.lines.first().copied()), l.t("csv.detail.lines", &params! {"lines" => p.lines.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")})]);
    }
    for e in &rec.extensions {
        let mut notes = vec![l.t("csv.detail.ruleCount", &params! {"n" => e.count})];
        notes.extend(e.examples.iter().map(|x| x.path.clone()));
        rows.push(vec![cat("ext"), format!(".{}", e.ext), l.t("csv.detail.extRisk", &params! {"label" => e.label, "risk" => l.s(&format!("enum.risk.{}", e.risk))}), opt(e.examples.first().map(|x| x.line)), join(&notes)]);
    }
    for c in &rec.comments {
        let note = c.note.as_ref().map(|n| format!(", {n}")).unwrap_or_default();
        rows.push(vec![cat("comment"), c.value.clone(), format!("{}{note}", l.s(&format!("recon.comments.kind.{}", c.kind))), c.line.to_string(), c.comment.clone()]);
    }
    for s in &r.security {
        let label = crate::security::category_info(engine, l, &s.category).0;
        rows.push(vec![cat("security"), format!("Disallow: {}", s.path), l.t("csv.detail.severity", &params! {"category" => label, "severity" => l.s(&format!("enum.severity.{}", s.severity))}), s.line.to_string(), s.reason.clone()]);
    }
    let recon = Tab { header: h(&["recon.category", "recon.item", "recon.detail", "line", "notes"]), rows };
    let _ = RuleType::Allow;
    Tabs { rules, sitemaps, issues, recon }
}

/// One CSV string per tab.
pub fn csv_tabs(r: &Report, engine: &Engine, l: &Locale) -> Vec<(String, String)> {
    let t = tab_rows(r, engine, l);
    TABS.iter().map(|k| (k.to_string(), { let tab = t.get(k).unwrap(); to_csv(&tab.header, &tab.rows) })).collect()
}

/// One sheet with a Sheet column and uniform columns across all tabs.
pub fn tagged_rows(r: &Report, engine: &Engine, l: &Locale) -> Tab {
    let t = tab_rows(r, engine, l);
    let labels = tab_labels(l);
    let label = |k: &str| labels.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut rows = Vec::new();
    for x in &t.rules.rows {
        rows.push(vec![label("rules"), x[0].clone(), x[2].clone(), x[3].clone(), x[4].clone(), l.t("csv.group", &params! {"n" => x[1]}), x[5].clone()]);
    }
    for x in &t.sitemaps.rows {
        rows.push(vec![label("sitemaps"), x[0].clone(), x[4].clone(), x[1].clone(), x[2].clone(), x[3].clone(), x[5].clone()]);
    }
    for x in &t.issues.rows {
        rows.push(vec![label("issues"), x[1].clone(), x[0].clone(), String::new(), x[2].clone(), String::new(), x[3].clone()]);
    }
    for x in &t.recon.rows {
        rows.push(vec![label("recon"), x[0].clone(), x[2].clone(), x[1].clone(), x[3].clone(), String::new(), x[4].clone()]);
    }
    let header = ["tagged.sheet", "tagged.item", "tagged.type", "tagged.value", "line", "tagged.status", "notes"].iter().map(|k| l.s(&format!("csv.{k}"))).collect();
    Tab { header, rows }
}

pub fn tagged_csv(r: &Report, engine: &Engine, l: &Locale) -> String {
    let t = tagged_rows(r, engine, l);
    to_csv(&t.header, &t.rows)
}

/// TSV for one tab, or for the tagged form when tab is "all".
pub fn tsv_for(r: &Report, engine: &Engine, l: &Locale, tab: &str) -> Option<String> {
    if tab == "all" {
        let t = tagged_rows(r, engine, l);
        return Some(to_tsv(&t.header, &t.rows));
    }
    let t = tab_rows(r, engine, l);
    t.get(tab).map(|x| to_tsv(&x.header, &x.rows))
}
