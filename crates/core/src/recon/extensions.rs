//! File extensions mentioned in rules, what they say about the site's tech
//! and content, and how risky an exposure would be.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::{is_absolute_url, plain_path, Model};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Example {
    pub path: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Extension {
    pub ext: String,
    pub category: String,
    pub label: String,
    pub risk: String,
    pub tech: Option<String>,
    pub count: u32,
    pub examples: Vec<Example>,
}

fn risk_rank(r: &str) -> u8 {
    match r {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

/// Extensions at the end of the path, or right before a wildcard/anchor/slash/query boundary.
fn extensions_in(lower: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes: Vec<char> = lower.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '.' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_alphanumeric() && j - i <= 8 {
                j += 1;
            }
            let len = j - i - 1;
            if (1..=8).contains(&len) && (j == bytes.len() || "$*/?".contains(bytes[j])) {
                let ext: String = bytes[i + 1..j].iter().collect();
                if !out.contains(&ext) {
                    out.push(ext);
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    let p = plain_path(lower);
    let p = p.split('?').next().unwrap_or("");
    if let Some(dot) = p.rfind('.') {
        let tail = &p[dot + 1..];
        if (1..=8).contains(&tail.len()) && tail.chars().all(|c| c.is_ascii_alphanumeric()) && !out.contains(&tail.to_string()) {
            out.push(tail.to_string());
        }
    }
    out
}

pub fn find_extensions(model: &Model, engine: &Engine, locale: &Locale) -> Vec<Extension> {
    let mut by_ext: Vec<Extension> = Vec::new();
    for r in model.each_rule() {
        if is_absolute_url(&r.rule.path) {
            continue;
        }
        for ext in extensions_in(&r.lower) {
            let Some(def) = engine.extensions.get(&ext) else { continue };
            let entry = match by_ext.iter_mut().find(|e| e.ext == ext) {
                Some(e) => e,
                None => {
                    by_ext.push(Extension {
                        ext: ext.clone(),
                        category: def.category.clone(),
                        label: locale.s(&def.label),
                        risk: def.risk.clone(),
                        tech: def.tech.as_ref().map(|t| locale.s(t)),
                        count: 0,
                        examples: Vec::new(),
                    });
                    by_ext.last_mut().unwrap()
                }
            };
            entry.count += 1;
            if entry.examples.len() < 3 {
                entry.examples.push(Example { path: r.rule.path.clone(), line: r.rule.line });
            }
        }
    }
    by_ext.sort_by(|a, b| risk_rank(&a.risk).cmp(&risk_rank(&b.risk)).then(b.count.cmp(&a.count)).then_with(|| a.ext.cmp(&b.ext)));
    by_ext
}

/// Distinct server technologies implied by extensions, e.g. ["PHP"].
pub fn tech_hints(extensions: &[Extension]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in extensions {
        if let Some(t) = &e.tech {
            if !out.contains(t) {
                out.push(t.clone());
            }
        }
    }
    out
}
