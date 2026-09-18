//! Cloud object storage, CDN and hosting hostnames leaked via Sitemap lines,
//! Host lines, absolute URLs in rules, and comments.

use crate::config::{BucketPart, Engine};
use crate::i18n::Locale;
use crate::model::{is_absolute_url, Model};
use crate::url_util::extract_urls;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Classified {
    pub provider: String,
    pub kind: String,
    pub host: String,
    pub bucket: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudAsset {
    pub provider: String,
    pub kind: String,
    pub host: String,
    pub bucket: Option<String>,
    pub url: String,
    pub line: u32,
    pub source: String,
    pub risk: String,
}

fn segment(u: &Url, n: usize) -> Option<String> {
    u.path().split('/').filter(|s| !s.is_empty()).nth(n.checked_sub(1)?).map(String::from)
}

fn extract_bucket(spec: &[Vec<BucketPart>], caps: &fancy_regex::Captures, u: &Url) -> Option<String> {
    for alt in spec {
        let mut parts: Vec<String> = Vec::new();
        for part in alt {
            let v = match part {
                BucketPart::HostGroup(n) => caps.get(*n).map(|m| m.as_str().to_string()),
                BucketPart::Segment(n) => segment(u, *n),
                BucketPart::Path(re) => re.captures(u.path()).ok().flatten().and_then(|c| c.get(1).map(|m| m.as_str().to_string())),
            };
            if let Some(v) = v.filter(|v| !v.is_empty()) {
                parts.push(v);
            }
        }
        if !parts.is_empty() && (alt.len() == 1 || !parts.is_empty()) {
            if parts.len() == alt.len() || alt.len() == 1 {
                return Some(parts.join(" / "));
            }
            if !parts.is_empty() {
                return Some(parts.join(" / "));
            }
        }
    }
    None
}

pub fn classify_host(u: &Url, engine: &Engine) -> Option<Classified> {
    let host = u.host_str()?.to_lowercase();
    for p in &engine.cloud {
        if let Ok(Some(caps)) = p.host.captures(&host) {
            return Some(Classified { provider: p.provider.clone(), kind: p.kind.clone(), host: host.clone(), bucket: extract_bucket(&p.bucket, &caps, u) });
        }
    }
    None
}

/// Every absolute URL in the file with where it came from.
pub fn collect_urls(model: &Model) -> Vec<(Url, u32, &'static str)> {
    let mut out = Vec::new();
    for s in &model.sitemaps {
        if s.valid {
            if let Ok(u) = Url::parse(&s.url) {
                out.push((u, s.line, "sitemap"));
            }
        }
    }
    if let Some(h) = &model.host {
        let v = if is_absolute_url(&h.value) { h.value.clone() } else { format!("https://{}", h.value) };
        if let Ok(u) = Url::parse(&v) {
            if u.host_str().is_some() {
                out.push((u, h.line, "host"));
            }
        }
    }
    for r in model.each_rule() {
        if is_absolute_url(&r.rule.path) {
            if let Ok(u) = Url::parse(&r.rule.path) {
                out.push((u, r.rule.line, "rule"));
            }
        }
    }
    for (line, text) in model.comment_lines() {
        for u in extract_urls(&text) {
            out.push((u, line, "comment"));
        }
    }
    out
}

pub fn find_cloud_assets(model: &Model, engine: &Engine, locale: &Locale) -> Vec<CloudAsset> {
    let mut found: Vec<CloudAsset> = Vec::new();
    for (url, line, source) in collect_urls(model) {
        let Some(c) = classify_host(&url, engine) else { continue };
        let key = (c.host.clone(), c.bucket.clone().unwrap_or_default());
        if found.iter().any(|f| f.host == key.0 && f.bucket.clone().unwrap_or_default() == key.1) {
            continue;
        }
        let risk = locale.s(&format!("recon.cloud.risk.{}", c.kind));
        found.push(CloudAsset { provider: c.provider, kind: c.kind, host: c.host, bucket: c.bucket, url: url.to_string(), line, source: source.into(), risk });
    }
    found
}
