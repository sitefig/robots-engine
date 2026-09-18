//! RFC 9309 matching on the model, as implemented by Google's robotstxt
//! library: most specific group first, `*` fallback, longest pattern wins,
//! Allow wins ties, empty Disallow never matches, percent-encoding normalised
//! on both sides, /robots.txt always allowed.

use crate::model::*;
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use url::Url;

thread_local! {
    static CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

fn is_unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~".contains(c)
}

/// Normalise percent-encoding so equivalent paths compare equal.
pub fn normalise_encoding(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '%' && i + 2 < chars.len() && chars[i + 1].is_ascii_hexdigit() && chars[i + 2].is_ascii_hexdigit() {
            let hex: String = chars[i + 1..=i + 2].iter().collect::<String>().to_uppercase();
            let code = u8::from_str_radix(&hex, 16).unwrap();
            if code < 128 && is_unreserved(code as char) {
                out.push(code as char);
            } else {
                out.push('%');
                out.push_str(&hex);
            }
            i += 3;
            continue;
        }
        let code = ch as u32;
        if code > 32 && code < 127 {
            out.push(ch);
        } else {
            let mut buf = [0u8; 4];
            for b in ch.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        i += 1;
    }
    out
}

fn compile(pattern: &str) -> Regex {
    let mut p = normalise_encoding(pattern);
    let anchored = p.ends_with('$');
    if anchored {
        p.pop();
    }
    let body = p.split('*').map(|s| fancy_regex::escape(s).into_owned()).collect::<Vec<_>>().join(".*");
    Regex::new(&format!("^{body}{}", if anchored { "$" } else { "" })).unwrap()
}

fn with_regex<T>(pattern: &str, f: impl FnOnce(&Regex) -> T) -> T {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if !c.contains_key(pattern) {
            c.insert(pattern.to_string(), compile(pattern));
        }
        f(&c[pattern])
    })
}

/// Match a rule pattern against a path. Both are normalised for encoding.
pub fn path_matches(pattern: &str, path: &str) -> bool {
    let p = normalise_encoding(path);
    with_regex(pattern, |re| re.is_match(&p).unwrap_or(false))
}

/// Turn user input (a path, or a full URL) into the path+query used for matching.
pub fn normalise_path(input: &str) -> String {
    let p = input.trim();
    if p.is_empty() {
        return "/".into();
    }
    if let Some(i) = p.find("://") {
        if p[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) && p[..i].chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
            if let Ok(u) = Url::parse(p) {
                return crate::url_util::path_and_query(&u);
            }
        }
    }
    if p.starts_with('/') { p.to_string() } else { format!("/{p}") }
}

pub fn groups_for_token<'a>(model: &'a Model, token: &str) -> Vec<&'a Group> {
    model.groups.iter().filter(|g| g.agents.iter().any(|a| a.token == token)).collect()
}

pub struct Selection<'a> {
    pub token: Option<String>,
    pub groups: Vec<&'a Group>,
    pub specific: bool,
}

/// Pick the groups that apply to a crawler; `tokens` most specific first.
pub fn select_groups<'a>(model: &'a Model, tokens: &[String]) -> Selection<'a> {
    for t in tokens {
        let token = t.to_lowercase();
        let groups = groups_for_token(model, &token);
        if !groups.is_empty() {
            return Selection { token: Some(token), groups, specific: true };
        }
    }
    let star = groups_for_token(model, "*");
    if !star.is_empty() {
        return Selection { token: Some("*".into()), groups: star, specific: false };
    }
    Selection { token: None, groups: Vec::new(), specific: false }
}

pub fn merged_rules<'a>(groups: &[&'a Group]) -> Vec<&'a Rule> {
    groups.iter().flat_map(|g| g.rules.iter()).filter(|r| !r.path.is_empty()).collect()
}

pub fn crawl_delay_for(groups: &[&Group]) -> Option<f64> {
    groups.iter().find_map(|g| g.crawl_delay)
}

/// Normalised pattern length, the precedence measure.
pub fn rule_len(rule: &Rule) -> usize {
    normalise_encoding(&rule.path).len()
}

/// Evaluate a path against a flat list of rules.
pub fn evaluate_path<'a>(rules: &[&'a Rule], path: &str) -> (bool, Option<&'a Rule>) {
    let normalised = normalise_encoding(path);
    let mut best: Option<(&Rule, usize)> = None;
    for rule in rules {
        if !with_regex(&rule.path, |re| re.is_match(&normalised).unwrap_or(false)) {
            continue;
        }
        let len = rule_len(rule);
        let better = match best {
            None => true,
            Some((b, bl)) => len > bl || (len == bl && rule.rule_type == RuleType::Allow && b.rule_type == RuleType::Disallow),
        };
        if better {
            best = Some((rule, len));
        }
    }
    match best {
        Some((r, _)) => (r.rule_type == RuleType::Allow, Some(r)),
        None => (true, None),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Access {
    pub token: Option<String>,
    pub specific: bool,
    #[serde(rename = "crawlDelay")]
    pub crawl_delay: Option<f64>,
    pub always: bool,
    pub allowed: bool,
    pub rule: Option<Rule>,
    pub path: String,
}

/// Full answer for one crawler and one path.
pub fn check_access(model: &Model, tokens: &[String], path: &str) -> Access {
    let sel = select_groups(model, tokens);
    let p = normalise_path(path);
    let crawl_delay = crawl_delay_for(&sel.groups);
    if p == "/robots.txt" {
        return Access { token: sel.token, specific: sel.specific, crawl_delay, always: true, allowed: true, rule: None, path: p };
    }
    let (allowed, rule) = evaluate_path(&merged_rules(&sel.groups), &p);
    Access { token: sel.token, specific: sel.specific, crawl_delay, always: false, allowed, rule: rule.cloned(), path: p }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cleaned {
    pub path: String,
    pub removed: Vec<String>,
}

/// Apply Yandex Clean-param directives to a path.
pub fn apply_clean_params(model: &Model, path: &str) -> Cleaned {
    let p = normalise_path(path);
    let Some(q) = p.find('?') else { return Cleaned { path: p, removed: vec![] } };
    if model.clean_params.is_empty() {
        return Cleaned { path: p, removed: vec![] };
    }
    let base = &p[..q];
    let mut names: Vec<&str> = Vec::new();
    for cp in &model.clean_params {
        if cp.valid && path_matches(&cp.path, base) {
            for n in &cp.params {
                if !names.contains(&n.as_str()) {
                    names.push(n);
                }
            }
        }
    }
    if names.is_empty() {
        return Cleaned { path: p.clone(), removed: vec![] };
    }
    let mut removed = Vec::new();
    let kept: Vec<&str> = p[q + 1..]
        .split('&')
        .filter(|pair| {
            let name = pair.split('=').next().unwrap_or("");
            if names.contains(&name) {
                removed.push(name.to_string());
                false
            } else {
                true
            }
        })
        .collect();
    let path = if kept.is_empty() { base.to_string() } else { format!("{base}?{}", kept.join("&")) };
    Cleaned { path, removed }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Open,
    Partial,
    Blocked,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Open => "open",
            Verdict::Partial => "partial",
            Verdict::Blocked => "blocked",
        }
    }
}

pub struct Summary {
    pub token: Option<String>,
    pub specific: bool,
    pub allow_count: usize,
    pub disallow_count: usize,
    pub root_allowed: bool,
    pub verdict: Verdict,
    pub crawl_delay: Option<f64>,
}

/// Overview of how a crawler fares.
pub fn summarise(model: &Model, tokens: &[String]) -> Summary {
    let sel = select_groups(model, tokens);
    let rules = merged_rules(&sel.groups);
    let (root_allowed, _) = evaluate_path(&rules, "/");
    let allow_count = rules.iter().filter(|r| r.rule_type == RuleType::Allow).count();
    let disallow_count = rules.len() - allow_count;
    let verdict = if rules.iter().any(|r| r.rule_type == RuleType::Disallow && whole_site(&r.path)) && allow_count == 0 {
        Verdict::Blocked
    } else if disallow_count == 0 {
        Verdict::Open
    } else {
        Verdict::Partial
    };
    Summary { token: sel.token, specific: sel.specific, allow_count, disallow_count, root_allowed, verdict, crawl_delay: crawl_delay_for(&sel.groups) }
}
