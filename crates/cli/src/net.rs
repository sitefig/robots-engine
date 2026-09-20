//! Shared plumbing: fetching with a reported redirect chain, the clock, webhooks.

use std::io::Read;
use susbot_core::fetch::{FetchInfo, Redirect};

pub const MAX_BYTES: usize = 512 * 1024;

pub fn iso_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

pub fn today() -> String {
    iso_now()[..10].to_string()
}

pub fn agent(timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(timeout)).redirects(0).build()
}

/// Fetch `<origin>/robots.txt`, following up to five redirects by hand so the
/// chain is reported, reading at most 512 KiB. Network failures are `Err`;
/// any HTTP answer, including 4xx/5xx, is `Ok` with the status inside.
/// What we send when the fetch is ours: the named crawler string from the
/// config, or the browser one when a user config dropped it.
pub fn our_ua(c: &susbot_core::config::Crawlers) -> &str {
    c.bot_ua.as_deref().unwrap_or(&c.browser_ua)
}

pub fn fetch_robots(origin: &str, ua: &str, timeout: u64) -> Result<FetchInfo, String> {
    match fetch_robots_at(origin, ua, timeout) {
        // Some apex hosts (hsbc.com) accept no connections at all while
        // www. answers; a crawler given the bare domain would try that too.
        Err(e) if !origin.contains("://www.") => match origin.split_once("://") {
            Some((scheme, host)) => fetch_robots_at(&format!("{scheme}://www.{host}"), ua, timeout).map_err(|_| e),
            None => Err(e),
        },
        r => r,
    }
}

fn fetch_robots_at(origin: &str, ua: &str, timeout: u64) -> Result<FetchInfo, String> {
    let robots_url = format!("{origin}/robots.txt");
    let client = agent(timeout);
    let mut url = robots_url.clone();
    let mut redirects = Vec::new();
    loop {
        let res = match client.get(&url).set("User-Agent", ua).set("Accept", "text/plain,*/*;q=0.8").call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("could not reach {url}: {e}")),
        };
        let status = res.status();
        if (300..400).contains(&status) {
            if let Some(loc) = res.header("Location") {
                let next = url::Url::parse(&url).and_then(|u| u.join(loc)).map(|u| u.to_string()).map_err(|e| e.to_string())?;
                redirects.push(Redirect { from: url.clone(), status: Some(status), to: next.clone() });
                if redirects.len() > 5 {
                    return Ok(FetchInfo { source: "cli".into(), robots_url, final_url: url, status, status_text: None, content_type: None, bytes: 0, truncated: false, redirects, redirect_limit: true, text: String::new() });
                }
                url = next;
                continue;
            }
        }
        let content_type = res.header("Content-Type").map(String::from);
        let status_text = Some(res.status_text().to_string());
        let mut buf = Vec::new();
        res.into_reader().take(MAX_BYTES as u64 + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let truncated = buf.len() > MAX_BYTES;
        buf.truncate(MAX_BYTES);
        let text = String::from_utf8_lossy(&buf).into_owned();
        return Ok(FetchInfo { source: "cli".into(), robots_url, final_url: url, status, status_text, content_type, bytes: buf.len(), truncated, redirects, redirect_limit: false, text });
    }
}

/// Post a message to a Slack- or Discord-style incoming webhook. Both `text`
/// (Slack) and `content` (Discord) are set. Returns the failure reason, if any.
pub fn send_webhook(url: &str, message: &str, timeout: u64) -> Result<(), String> {
    let payload = serde_json::json!({ "text": message, "content": message });
    let client = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(timeout)).build();
    match client.post(url).set("Content-Type", "application/json").send_string(&payload.to_string()) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) => Err(format!("webhook answered HTTP {code}")),
        Err(e) => Err(e.to_string()),
    }
}

/// FNV-1a 64-bit, enough to spot identical bodies across a dataset.
pub fn fnv1a(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}
