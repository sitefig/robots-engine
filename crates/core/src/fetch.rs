//! How the file was served. The engine does no I/O; the CLI and the browser
//! pass in what they fetched and get the fetch-level findings back.

use crate::config::{Checks, HeaderLimits};
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

/// Response headers the caller could read, beyond the content type.
///
/// A browser fetching another origin directly only sees the CORS-safelisted
/// headers (content-type, cache-control, expires, last-modified, pragma,
/// content-language), so the rest arrive only through the proxy or the command
/// line. An absent header here means "not seen", not "not sent", which is why
/// every check below fires on a header that is present and never on a missing
/// one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Headers {
    #[serde(rename = "cacheControl", default)]
    pub cache_control: Option<String>,
    #[serde(default)]
    pub expires: Option<String>,
    #[serde(rename = "xRobotsTag", default)]
    pub x_robots_tag: Option<String>,
    #[serde(rename = "retryAfter", default)]
    pub retry_after: Option<String>,
    #[serde(default)]
    pub vary: Option<String>,
    #[serde(rename = "contentEncoding", default)]
    pub content_encoding: Option<String>,
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
    /// What the response said beyond its body. Empty when the caller could not
    /// read them, which is the normal case for a direct fetch in a browser.
    #[serde(default)]
    pub headers: Headers,
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

/// The charset parameter of the content type, lower-cased.
fn charset(f: &FetchInfo) -> Option<String> {
    let ct = f.content_type.as_ref()?;
    ct.split(';')
        .skip(1)
        .map(str::trim)
        .find_map(|p| p.strip_prefix("charset=").or_else(|| p.strip_prefix("CHARSET=")))
        .map(|v| v.trim_matches('"').to_lowercase())
}

/// One directive of a Cache-Control header, by name: `max-age` gives its value.
fn cache_directive<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header
        .split(',')
        .map(str::trim)
        .find(|d| d.to_lowercase() == name || d.to_lowercase().starts_with(&format!("{name}=")))
        .map(|d| d.split_once('=').map(|(_, v)| v.trim().trim_matches('"')).unwrap_or(""))
}

/// Findings from the headers of the response rather than from the file.
///
/// Each one needs a header that is actually there: a file served with no cache
/// headers at all is normal and says nothing.
fn header_warnings(f: &FetchInfo, limits: &HeaderLimits, locale: &Locale, out: &mut Vec<Warning>) {
    let w = |level, id: &str, p: &crate::i18n::Params| warning(locale, WarningKind::Fetch, level, None, id, p, None);
    let h = &f.headers;

    // A 429 or a 503 is read as "block everything" while it lasts, so what the
    // server asks crawlers to wait is how long the site can stay out of search.
    if matches!(f.status, 429 | 503) {
        if let Some(seconds) = h.retry_after.as_deref().and_then(|v| v.trim().parse::<u64>().ok()) {
            if seconds > limits.retry_after {
                out.push(w(Level::Warning, "fetch.warn.retryAfterLong", &params! {"seconds" => seconds, "status" => f.status}));
            }
        }
    }
    if !(200..300).contains(&f.status) {
        return;
    }
    // RFC 9309 requires UTF-8. A parser that honours the charset reads a file
    // in another encoding as something else, and ASCII is a subset of UTF-8.
    if let Some(cs) = charset(f) {
        if !matches!(cs.as_str(), "utf-8" | "utf8" | "us-ascii" | "ascii") {
            out.push(w(Level::Warning, "fetch.warn.charset", &params! {"charset" => cs}));
        }
    }
    if let Some(cc) = h.cache_control.as_deref() {
        let no_store = ["no-store", "no-cache"].iter().any(|d| cache_directive(cc, d).is_some());
        let zero = cache_directive(cc, "max-age").map(|v| v.trim() == "0").unwrap_or(false);
        if no_store || zero {
            out.push(w(Level::Info, "fetch.warn.cacheNoStore", &params! {"value" => cc.trim()}));
        } else if let Some(age) = cache_directive(cc, "max-age").and_then(|v| v.trim().parse::<u64>().ok()) {
            if age > limits.max_age {
                out.push(w(Level::Warning, "fetch.warn.cacheTooLong", &params! {"maxage" => age}));
            }
        }
    }
    if let Some(tag) = h.x_robots_tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        out.push(w(Level::Info, "fetch.warn.xRobotsTag", &params! {"value" => tag}));
    }
    // Vary: User-Agent says the answer depends on who asks, which is what the
    // access check measures by asking as each crawler.
    if let Some(vary) = h.vary.as_deref() {
        if vary.split(',').map(|v| v.trim().to_lowercase()).any(|v| v == "user-agent" || v == "*") {
            out.push(w(Level::Info, "fetch.warn.varyUserAgent", &params! {"value" => vary.trim()}));
        }
    }
}

/// Findings about the redirect chain and the final URL.
pub fn fetch_warnings(f: &FetchInfo, checks: &Checks, locale: &Locale) -> Vec<Warning> {
    let mut out = Vec::new();
    let w = |level, id: &str, p: &crate::i18n::Params| warning(locale, WarningKind::Fetch, level, None, id, p, None);
    response_warnings(f, locale, &mut out);
    if checks.headers {
        header_warnings(f, &checks.header_limits, locale, &mut out);
    }
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
