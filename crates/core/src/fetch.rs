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

/// The media type of the response, without parameters.
fn media_type(f: &FetchInfo) -> Option<String> {
    let ct = f.content_type.as_ref()?;
    let t = ct.split(';').next().unwrap_or("").trim().to_lowercase();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Findings about the answer itself: status, media type and body.
fn response_warnings(f: &FetchInfo, locale: &Locale, out: &mut Vec<Warning>) {
    let w = |level, id: &str, p: &crate::i18n::Params| warning(locale, WarningKind::Fetch, level, None, id, p, None);
    // Google treats a 5xx or 429 robots.txt as "disallow everything" while it lasts.
    if (500..600).contains(&f.status) {
        out.push(w(Level::Error, "fetch.warn.serverError", &params! {"status" => f.status}));
    } else if f.status == 429 {
        out.push(w(Level::Warning, "fetch.warn.rateLimited", &[]));
    } else if f.status == 401 || f.status == 403 {
        out.push(w(Level::Info, "fetch.warn.forbidden", &params! {"status" => f.status}));
    }
    if !(200..300).contains(&f.status) {
        return;
    }
    match media_type(f) {
        Some(t) if t == "text/plain" => {}
        Some(t) => out.push(w(Level::Warning, "fetch.warn.contentType", &params! {"type" => t})),
        None => out.push(w(Level::Warning, "fetch.warn.contentTypeMissing", &[])),
    }
    if crate::parser::looks_like_html_page(&f.text) {
        out.push(w(Level::Error, "fetch.warn.htmlBody", &[]));
    }
}

/// Findings about the redirect chain and the final URL.
pub fn fetch_warnings(f: &FetchInfo, locale: &Locale) -> Vec<Warning> {
    let mut out = Vec::new();
    let w = |level, id: &str, p: &crate::i18n::Params| warning(locale, WarningKind::Fetch, level, None, id, p, None);
    response_warnings(f, locale, &mut out);
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
