//! The one normalised report every export derives from. The shape is
//! described by schema/report.schema.json; keep the two in sync.

use crate::agents::{category_label, summaries};
use crate::ai_status::{ai_status, AiStatus};
use crate::analyser::{summarise, Verdict};
use crate::config::Engine;
use crate::fetch::{FetchInfo, Redirect};
use crate::i18n::Locale;
use crate::model::*;
use crate::recon::{self, Recon};
use crate::security::{find_sensitive_paths, Finding};
use serde::Serialize;

pub const SCHEMA_VERSION: &str = "1.2.0";

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub vendor: String,
    pub url: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    #[serde(rename = "siteUrl")]
    pub site_url: Option<String>,
    #[serde(rename = "robotsUrl")]
    pub robots_url: Option<String>,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "fetchedVia")]
    pub fetched_via: String,
    #[serde(rename = "httpStatus")]
    pub http_status: Option<u16>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    pub bytes: usize,
    pub truncated: bool,
    pub redirects: Vec<Redirect>,
    #[serde(rename = "redirectLimit")]
    pub redirect_limit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssueCounts {
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DefaultPolicy {
    #[serde(rename = "hasStarGroup")]
    pub has_star_group: bool,
    pub verdict: Verdict,
    #[serde(rename = "allowRules")]
    pub allow_rules: usize,
    #[serde(rename = "disallowRules")]
    pub disallow_rules: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiTraining {
    pub blocked: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub groups: usize,
    pub rules: usize,
    pub sitemaps: usize,
    pub issues: IssueCounts,
    #[serde(rename = "securityFindings")]
    pub security_findings: usize,
    #[serde(rename = "reconFindings")]
    pub recon_findings: usize,
    #[serde(rename = "defaultPolicy")]
    pub default_policy: DefaultPolicy,
    #[serde(rename = "aiTraining")]
    pub ai_training: AiTraining,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupOut {
    pub index: usize,
    #[serde(rename = "userAgents")]
    pub user_agents: Vec<String>,
    pub tokens: Vec<String>,
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
    #[serde(rename = "crawlDelay")]
    pub crawl_delay: Option<f64>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleOut {
    pub group: usize,
    #[serde(rename = "userAgents")]
    pub user_agents: Vec<String>,
    #[serde(rename = "type")]
    pub rule_type: RuleType,
    pub path: String,
    pub line: u32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SitemapOut {
    pub url: String,
    pub line: u32,
    pub valid: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Directives {
    pub host: Option<HostDirective>,
    #[serde(rename = "cleanParams")]
    pub clean_params: Vec<CleanParam>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlerOut {
    pub name: String,
    pub category: String,
    #[serde(rename = "categoryLabel")]
    pub category_label: String,
    pub tokens: Vec<String>,
    pub note: Option<String>,
    #[serde(rename = "groupUsed")]
    pub group_used: Option<String>,
    #[serde(rename = "ownGroup")]
    pub own_group: bool,
    pub verdict: Verdict,
    #[serde(rename = "rootAllowed")]
    pub root_allowed: bool,
    #[serde(rename = "allowRules")]
    pub allow_rules: usize,
    #[serde(rename = "disallowRules")]
    pub disallow_rules: usize,
    #[serde(rename = "crawlDelay")]
    pub crawl_delay: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityCategoryOut {
    pub id: String,
    pub label: String,
    pub advice: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "generatedAt")]
    pub generated_at: String,
    pub language: String,
    pub tool: ToolInfo,
    pub source: Source,
    pub summary: Summary,
    pub groups: Vec<GroupOut>,
    pub rules: Vec<RuleOut>,
    pub sitemaps: Vec<SitemapOut>,
    pub directives: Directives,
    pub issues: Vec<Warning>,
    pub crawlers: Vec<CrawlerOut>,
    #[serde(rename = "aiStatus")]
    pub ai_status: AiStatus,
    pub security: Vec<Finding>,
    /// Categories present in `security`, in config order, with translated label and advice.
    #[serde(rename = "securityCategories")]
    pub security_categories: Vec<SecurityCategoryOut>,
    pub recon: Recon,
    pub raw: String,
}

pub struct ReportInput<'a> {
    pub model: &'a Model,
    pub text: &'a str,
    pub fetch: Option<&'a FetchInfo>,
    pub site_url: Option<&'a str>,
    pub schema_url: &'a str,
    pub generated_at: &'a str,
    pub today: Option<(i32, u32, u32)>,
}

pub fn build_report(input: ReportInput, engine: &Engine, locale: &Locale) -> Report {
    let model = input.model;
    let rec = recon::recon(model, input.site_url, engine, locale, input.today);
    let security = find_sensitive_paths(model, engine, locale);
    let ai = ai_status(model, engine, locale);
    let notes = |line: u32| -> Vec<String> { model.warnings.iter().filter(|w| w.line == Some(line)).map(|w| w.message.clone()).collect() };

    let groups: Vec<GroupOut> = model
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| GroupOut {
            index: i + 1,
            user_agents: g.agents.iter().map(|a| a.raw.clone()).collect(),
            tokens: g.agents.iter().filter(|a| !a.token.is_empty()).map(|a| a.token.clone()).collect(),
            start_line: g.start_line,
            end_line: g.end_line,
            crawl_delay: g.crawl_delay,
            rules: g.rules.clone(),
        })
        .collect();
    let rules: Vec<RuleOut> = groups
        .iter()
        .flat_map(|g| g.rules.iter().map(move |r| RuleOut { group: g.index, user_agents: g.user_agents.clone(), rule_type: r.rule_type, path: r.path.clone(), line: r.line, notes: notes(r.line) }))
        .collect();
    let sitemaps: Vec<SitemapOut> = model.sitemaps.iter().map(|s| SitemapOut { url: s.url.clone(), line: s.line, valid: s.valid, notes: notes(s.line) }).collect();
    let issues = model.warnings.clone();
    let counts = IssueCounts {
        errors: issues.iter().filter(|i| i.level == Level::Error).count(),
        warnings: issues.iter().filter(|i| i.level == Level::Warning).count(),
        notes: issues.iter().filter(|i| i.level == Level::Info).count(),
    };
    let crawlers: Vec<CrawlerOut> = summaries(model, engine)
        .into_iter()
        .map(|s| CrawlerOut {
            name: s.crawler.name.clone(),
            category: s.crawler.category.clone(),
            category_label: category_label(locale, &s.crawler.category),
            tokens: s.crawler.tokens.clone(),
            note: s.crawler.note.as_ref().map(|n| locale.s(n)),
            group_used: s.summary.token.clone(),
            own_group: s.summary.specific,
            verdict: s.summary.verdict,
            root_allowed: s.summary.root_allowed,
            allow_rules: s.summary.allow_count,
            disallow_rules: s.summary.disallow_count,
            crawl_delay: s.summary.crawl_delay,
        })
        .collect();
    let star = summarise(model, &[]);
    let totals = recon::counts(&rec);
    let fetched_via = match input.fetch {
        Some(f) => f.source.clone(),
        None if input.site_url.is_some() => "example".into(),
        None => "pasted".into(),
    };
    Report {
        schema: input.schema_url.to_string(),
        schema_version: SCHEMA_VERSION.into(),
        generated_at: input.generated_at.to_string(),
        language: locale.lang().to_string(),
        tool: ToolInfo { name: engine.config.tool.name.clone(), vendor: engine.config.tool.vendor.clone(), url: engine.config.tool.url.clone(), version: crate::VERSION.into() },
        source: Source {
            site_url: input.site_url.map(String::from),
            robots_url: input.fetch.map(|f| f.robots_url.clone()),
            final_url: input.fetch.map(|f| f.final_url.clone()),
            fetched_via,
            http_status: input.fetch.map(|f| f.status),
            content_type: input.fetch.and_then(|f| f.content_type.clone()),
            bytes: input.fetch.map(|f| f.bytes).unwrap_or(input.text.len()),
            truncated: input.fetch.map(|f| f.truncated).unwrap_or(false),
            redirects: input.fetch.map(|f| f.redirects.clone()).unwrap_or_default(),
            redirect_limit: input.fetch.map(|f| f.redirect_limit).unwrap_or(false),
        },
        summary: Summary {
            groups: groups.len(),
            rules: rules.len(),
            sitemaps: sitemaps.len(),
            issues: counts,
            security_findings: security.len(),
            recon_findings: totals.total(),
            default_policy: DefaultPolicy { has_star_group: star.token.is_some(), verdict: if star.token.is_none() { Verdict::Open } else { star.verdict }, allow_rules: star.allow_count, disallow_rules: star.disallow_count },
            ai_training: AiTraining { blocked: ai.groups.first().map(|g| g.counts.blocked).unwrap_or(0), total: ai.groups.first().map(|g| g.crawlers.len()).unwrap_or(0) },
            platform: rec.stack.primary.as_ref().map(|p| p.name.clone()),
        },
        groups,
        rules,
        sitemaps,
        directives: Directives { host: model.host.clone(), clean_params: model.clean_params.clone() },
        issues,
        crawlers,
        ai_status: ai,
        security_categories: crate::security::categories_in(&security, engine)
            .into_iter()
            .map(|id| {
                let (label, advice) = crate::security::category_info(engine, locale, id);
                SecurityCategoryOut { id: id.to_string(), label, advice }
            })
            .collect(),
        security,
        recon: rec,
        raw: input.text.to_string(),
    }
}

/// Hostname the report is about, for titles and filenames.
pub fn report_host(r: &Report) -> Option<String> {
    let u = r.source.final_url.as_ref().or(r.source.site_url.as_ref())?;
    url::Url::parse(u).ok()?.host_str().map(String::from)
}

/// Safe base filename such as "www-example-com-robots-audit-2026-09-16".
pub fn report_filename(r: &Report) -> String {
    let host = report_host(r).unwrap_or_else(|| "pasted".into());
    let mut slug = String::new();
    let mut last_dash = false;
    for c in host.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let date: String = r.generated_at.chars().take(10).collect();
    format!("{}-robots-audit-{date}", slug.trim_matches('-'))
}
