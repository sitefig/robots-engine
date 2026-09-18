//! API gateways, interactive docs and schemas, GraphQL and RPC endpoints that rules point at.

use crate::config::Engine;
use crate::model::{is_absolute_url, plain_path, Model};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ApiEndpoint {
    pub path: String,
    pub line: u32,
    pub kind: String,
    pub version: Option<String>,
}

fn version_of(p: &str) -> Option<String> {
    for seg in p.split('/') {
        if seg.len() > 1 && seg.starts_with('v') && seg[1..].chars().all(|c| c.is_ascii_digit()) {
            return Some(seg.to_string());
        }
    }
    None
}

pub fn find_api_endpoints(model: &Model, engine: &Engine) -> Vec<ApiEndpoint> {
    let mut found: Vec<ApiEndpoint> = Vec::new();
    for r in model.each_rule() {
        if is_absolute_url(&r.rule.path) {
            continue;
        }
        let p = plain_path(&r.lower);
        // "/v2/" alone is a parallel site version, see hosts.
        let trimmed = p.trim_end_matches('/');
        if trimmed.len() > 2 && trimmed.starts_with("/v") && trimmed[2..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(sig) = engine.api.iter().find(|s| s.pattern.is_match(&p).unwrap_or(false)) else { continue };
        if found.iter().any(|f| f.path == r.rule.path) {
            continue;
        }
        found.push(ApiEndpoint { path: r.rule.path.clone(), line: r.rule.line, kind: sig.kind.clone(), version: version_of(&p) });
    }
    let order: Vec<&str> = engine.api.iter().map(|s| s.kind.as_str()).collect();
    let pos = |k: &str| order.iter().position(|o| *o == k).unwrap_or(usize::MAX);
    found.sort_by(|a, b| pos(&a.kind).cmp(&pos(&b.kind)).then(a.line.cmp(&b.line)));
    found
}
