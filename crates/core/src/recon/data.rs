//! Business intelligence hidden in rules: product and inventory feeds,
//! exports, partner and VIP portals, and how the internal search is queried.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::{is_absolute_url, plain_path, segments, Model};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PathKind {
    pub path: String,
    pub line: u32,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPath {
    pub path: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Param {
    pub name: String,
    pub role: String,
    pub lines: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Search {
    pub paths: Vec<SearchPath>,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DataFeeds {
    pub feeds: Vec<PathKind>,
    pub portals: Vec<PathKind>,
    pub search: Search,
}

fn describe(list: &[crate::config::CompiledLabelled], fallback: &str, text: &str, locale: &Locale) -> String {
    let key = list.iter().find(|l| l.pattern.is_match(text).unwrap_or(false)).map(|l| l.label.as_str()).unwrap_or(fallback);
    locale.s(key)
}

fn role_of(name: &str, engine: &Engine) -> &'static str {
    let d = &engine.data;
    if d.search_keys.contains(name) {
        "search"
    } else if d.filter_keys.contains(name) {
        "filter"
    } else if d.tracking_keys.is_match(name).unwrap_or(false) {
        "tracking"
    } else {
        "other"
    }
}

fn param_names(query: &str) -> Vec<String> {
    query.split('&').filter_map(|pair| {
        let name = pair.split('=').next().unwrap_or("");
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-[]".contains(c)) { Some(name.to_string()) } else { None }
    }).collect()
}

pub fn find_data_feeds(model: &Model, engine: &Engine, locale: &Locale) -> DataFeeds {
    let d = &engine.data;
    let cfg = &engine.config.recon.data;
    let mut out = DataFeeds::default();
    for r in model.each_rule() {
        if is_absolute_url(&r.rule.path) {
            continue;
        }
        let segs = segments(&r.rule.path);
        let plain = plain_path(&r.lower);
        let path_part = plain.split('?').next().unwrap_or("");
        let is_feed = segs.iter().any(|s| d.feed_words.is_match(s).unwrap_or(false))
            || (d.feed_file.is_match(path_part).unwrap_or(false) && (d.feed_hint.is_match(&plain).unwrap_or(false) || d.feed_file_wildcard.is_match(&r.lower).unwrap_or(false)));
        if is_feed && !out.feeds.iter().any(|f| f.path == r.rule.path) {
            out.feeds.push(PathKind { path: r.rule.path.clone(), line: r.rule.line, kind: describe(&d.feed_kinds, &cfg.feed_fallback, &r.lower, locale) });
        }
        if let Some(seg) = segs.iter().find(|s| d.portal_words.is_match(s).unwrap_or(false)) {
            if !out.portals.iter().any(|f| f.path == r.rule.path) {
                out.portals.push(PathKind { path: r.rule.path.clone(), line: r.rule.line, kind: describe(&d.portal_kinds, &cfg.portal_fallback, seg, locale) });
            }
        }
        let has_search_seg = segs.iter().any(|s| d.search_segments.is_match(s).unwrap_or(false));
        let mut push_search = |path: &str, line: u32| {
            if !out.search.paths.iter().any(|p| p.path == path) {
                out.search.paths.push(SearchPath { path: path.to_string(), line });
            }
        };
        if has_search_seg {
            push_search(&r.rule.path, r.rule.line);
        }
        if let Some(q) = plain.find('?') {
            let query = &plain[q + 1..];
            for name in param_names(query) {
                match out.search.params.iter_mut().find(|p| p.name == name) {
                    Some(p) => p.lines.push(r.rule.line),
                    None => out.search.params.push(Param { role: role_of(&name, engine).into(), name, lines: vec![r.rule.line] }),
                }
            }
            if !has_search_seg && query.split('&').any(|pair| pair.contains('=') && d.search_keys.contains(pair.split('=').next().unwrap_or(""))) {
                push_search(&r.rule.path, r.rule.line);
            }
        }
    }
    let rank = |r: &str| match r {
        "search" => 0,
        "filter" => 1,
        "tracking" => 2,
        _ => 3,
    };
    out.search.params.sort_by(|a, b| rank(&a.role).cmp(&rank(&b.role)).then_with(|| a.name.cmp(&b.name)));
    out
}
