//! Disallow rules that point at commonly sensitive locations. robots.txt is
//! public, so every Disallow line doubles as a map for reconnaissance. This
//! does not test whether the paths exist or are protected.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub path: String,
    pub line: u32,
    #[serde(rename = "userAgents")]
    pub agents: Vec<String>,
    pub category: String,
    pub severity: String,
    pub reason: String,
}

pub fn severity_rank(s: &str) -> u8 {
    match s {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

/// One finding per Disallow rule that matches a signature, highest severity
/// first, then by line. The highest-severity signature wins per rule.
pub fn find_sensitive_paths(model: &Model, engine: &Engine, locale: &Locale) -> Vec<Finding> {
    if !engine.config.checks.security {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for group in &model.groups {
        let agents: Vec<String> = group.agents.iter().map(|a| a.raw.clone()).collect();
        for rule in &group.rules {
            if rule.rule_type != RuleType::Disallow || rule.path.is_empty() || whole_site(&rule.path) {
                continue;
            }
            let mut p = rule.path.to_lowercase();
            if p.ends_with('$') {
                p.pop();
            }
            if engine.security_ignore.iter().any(|re| re.is_match(&p).unwrap_or(false)) {
                continue;
            }
            let best = engine
                .security
                .iter()
                .filter(|s| s.regexes.iter().any(|re| re.is_match(&p).unwrap_or(false)))
                .min_by_key(|s| severity_rank(&s.severity));
            if let Some(s) = best {
                findings.push(Finding { path: rule.path.clone(), line: rule.line, agents: agents.clone(), category: s.category.clone(), severity: s.severity.clone(), reason: locale.s(&s.reason) });
            }
        }
    }
    findings.sort_by(|a, b| severity_rank(&a.severity).cmp(&severity_rank(&b.severity)).then(a.line.cmp(&b.line)));
    findings
}

/// Categories present in a set of findings, in config order.
pub fn categories_in<'a>(findings: &[Finding], engine: &'a Engine) -> Vec<&'a str> {
    engine.config.security.categories.iter().map(|c| c.id.as_str()).filter(|id| findings.iter().any(|f| f.category == *id)).collect()
}

/// Translated label and advice for a category (plain text when the config holds text).
pub fn category_info(engine: &Engine, locale: &Locale, id: &str) -> (String, String) {
    match engine.security_category(id) {
        Some(c) => (locale.s(&c.label), locale.s(&c.advice)),
        None => (id.to_string(), String::new()),
    }
}
