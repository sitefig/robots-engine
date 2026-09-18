//! How the file was served. The engine does no I/O; the CLI and the browser
//! pass in what they fetched and get the fetch-level findings back.

use crate::i18n::Locale;
use crate::model::{Level, Warning, WarningKind};
use crate::params;
use crate::parser::warning;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redirect {
    pub from: String,
    pub status: Option<u16>,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchInfo {
    /// "direct" or "proxy" in the browser, "cli" on the command line.
    pub source: String,
    #[serde(rename = "robotsUrl")]
    pub robots_url: String,
    #[serde(rename = "finalUrl")]
    pub final_url: String,
    pub status: u16,
    #[serde(rename = "statusText", default)]
    pub status_text: Option<String>,
    #[serde(rename = "contentType", default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub bytes: usize,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub redirects: Vec<Redirect>,
    #[serde(rename = "redirectLimit", default)]
    pub redirect_limit: bool,
    /// Full body as fetched; the engine decides whether crawlers would read it.
    #[serde(default)]
    pub text: String,
}

impl FetchInfo {
    /// Crawlers ignore the body of non-2xx responses and give up after five redirects.
    pub fn usable(&self) -> bool {
        (200..300).contains(&self.status) && !self.redirect_limit
    }
}

/// Findings about the redirect chain and the final URL.
pub fn fetch_warnings(f: &FetchInfo, locale: &Locale) -> Vec<Warning> {
    let mut out = Vec::new();
    let w = |level, id: &str, p: &crate::i18n::Params| warning(locale, WarningKind::Fetch, level, None, id, p, None);
    if f.redirect_limit {
        out.push(w(Level::Error, "fetch.warn.redirectLimit", &[]));
        return out;
    }
    let (Ok(from), Ok(to)) = (Url::parse(&f.robots_url), Url::parse(&f.final_url)) else { return out };
    let host = |u: &Url| u.host_str().map(|h| match u.port() {
        Some(p) => format!("{h}:{p}"),
        None => h.to_string(),
    }).unwrap_or_default();
    let (fh, th) = (host(&from), host(&to));
    if fh != th {
        let bare = |h: &str| h.strip_prefix("www.").map(String::from).unwrap_or_else(|| h.to_string());
        if bare(&fh) == bare(&th) {
            out.push(w(Level::Info, "fetch.warn.redirectSameSite", &params! {"host" => th}));
        } else {
            out.push(w(Level::Warning, "fetch.warn.redirectOtherHost", &params! {"from" => fh, "to" => th}));
        }
    }
    if to.path() != "/robots.txt" {
        out.push(w(Level::Warning, "fetch.warn.notRobotsPath", &params! {"path" => to.path()}));
    }
    out
}
