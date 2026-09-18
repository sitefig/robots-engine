//! Hostnames and environment hints that a robots.txt gives away: absolute
//! URLs in Sitemap, Host, rules and comments; bare hostnames in comments; and
//! path names that smell like a staging site or redesign.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::{is_absolute_url, segments, Model};
use crate::url_util::*;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub line: u32,
    pub source: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeakedHost {
    pub host: String,
    pub relation: String,
    pub env: Option<String>,
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvPath {
    pub path: String,
    pub line: u32,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Hosts {
    pub hosts: Vec<LeakedHost>,
    pub paths: Vec<EnvPath>,
}

pub fn environment_tag(host: &str, engine: &Engine) -> Option<String> {
    let bare = strip_www(host);
    let labels: Vec<&str> = bare.split('.').collect();
    let root = root_domain(host);
    let sub = if bare.len() > root.len() + 1 { &bare[..bare.len() - root.len() - 1] } else { "" };
    let sub_labels: Vec<&str> = if sub.is_empty() { Vec::new() } else { sub.split('.').collect() };
    sub_labels
        .iter()
        .find(|l| engine.hosts.env_labels.is_match(l).unwrap_or(false))
        .or_else(|| labels.iter().find(|l| engine.hosts.env_label_fallback.is_match(l).unwrap_or(false)))
        .map(|s| s.to_string())
}

pub fn find_leaked_hosts(model: &Model, site_url: Option<&str>, engine: &Engine, locale: &Locale) -> Hosts {
    let site = site_host(site_url);
    let mut mentions: Vec<(String, u32, &str, String)> = Vec::new();
    let mut add = |host: &str, line: u32, source: &'static str, text: String| {
        let host = host.to_lowercase();
        if let Some(s) = &site {
            if strip_www(&host) == strip_www(s) {
                return;
            }
        }
        mentions.push((host, line, source, text));
    };
    for s in &model.sitemaps {
        if s.valid {
            if let Some(h) = Url::parse(&s.url).ok().and_then(|u| u.host_str().map(String::from)) {
                add(&h, s.line, "Sitemap", s.url.clone());
            }
        }
    }
    if let Some(h) = &model.host {
        let v = h.value.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("HTTPS://").trim_start_matches("HTTP://");
        let v = v.split('/').next().unwrap_or("");
        if !v.is_empty() {
            add(v, h.line, "Host", h.value.clone());
        }
    }
    for r in model.each_rule() {
        if !is_absolute_url(&r.rule.path) {
            continue;
        }
        if let Some(h) = Url::parse(&r.rule.path).ok().and_then(|u| u.host_str().map(String::from)) {
            add(&h, r.rule.line, r.rule.rule_type.label(), r.rule.path.clone());
        }
    }
    let email = fancy_regex::Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap();
    for (line, text) in model.comment_lines() {
        let without_emails = email.replace_all(&text, " ").into_owned();
        for u in extract_urls(&without_emails) {
            if let Some(h) = u.host_str() {
                add(h, line, "comment", text.clone());
            }
        }
        for h in extract_hostnames(&without_emails) {
            add(&h, line, "comment", text.clone());
        }
    }

    let mut hosts: Vec<LeakedHost> = Vec::new();
    for (host, line, source, text) in mentions {
        let entry = match hosts.iter_mut().find(|h| h.host == host) {
            Some(e) => e,
            None => {
                let relation = if is_ipv4(&host) {
                    "ip"
                } else if site.as_ref().map(|s| root_domain(&host) == root_domain(s)).unwrap_or(false) {
                    "subdomain"
                } else {
                    "external"
                };
                let env = if relation == "ip" {
                    if is_private_ip(&host) { Some(locale.s("recon.hosts.privateIp")) } else { None }
                } else {
                    environment_tag(&host, engine)
                };
                hosts.push(LeakedHost { host: host.clone(), relation: relation.into(), env, sources: Vec::new() });
                hosts.last_mut().unwrap()
            }
        };
        if !entry.sources.iter().any(|s| s.line == line && s.source == source) {
            entry.sources.push(Source { line, source: source.into(), text });
        }
    }
    hosts.sort_by(|a, b| b.env.is_some().cmp(&a.env.is_some()).then_with(|| a.host.cmp(&b.host)));

    let mut paths: Vec<EnvPath> = Vec::new();
    for r in model.each_rule() {
        if is_absolute_url(&r.rule.path) {
            continue;
        }
        let segs = segments(&r.rule.path);
        let mut hint = segs.iter().find(|s| engine.hosts.env_paths.is_match(s).unwrap_or(false) || engine.hosts.env_path_hint.is_match(s).unwrap_or(false)).cloned();
        // "/v2/" on its own is a parallel site; "/api/v2/" is just API versioning.
        if hint.is_none() && segs.len() == 1 && segs[0].starts_with('v') && segs[0][1..].chars().all(|c| c.is_ascii_digit()) && segs[0].len() > 1 {
            hint = Some(segs[0].clone());
        }
        if let Some(hint) = hint {
            if !paths.iter().any(|p| p.path == r.rule.path) {
                paths.push(EnvPath { path: r.rule.path.clone(), line: r.rule.line, hint });
            }
        }
    }
    Hosts { hosts, paths }
}
