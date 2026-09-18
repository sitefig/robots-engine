//! URL and hostname helpers.

use fancy_regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;
use url::Url;

// Common second-level public suffixes, enough to tell example.co.uk from
// other.co.uk without shipping the full public suffix list.
static TWO_LEVEL: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "co.uk", "org.uk", "ac.uk", "gov.uk", "me.uk", "ltd.uk", "plc.uk", "net.uk", "com.au", "net.au", "org.au", "edu.au", "gov.au", "co.nz",
        "org.nz", "net.nz", "co.jp", "ne.jp", "or.jp", "ac.jp", "com.br", "com.mx", "com.ar", "com.co", "com.pe", "com.cn", "com.tw", "com.hk",
        "com.sg", "com.my", "co.in", "co.kr", "co.id", "com.ph", "co.za", "com.tr", "com.ua", "co.il", "com.pl", "com.eg", "com.sa",
    ]
    .into_iter()
    .collect()
});

/// "www.shop.example.co.uk" -> "example.co.uk"
pub fn root_domain(host: &str) -> String {
    let host = host.to_lowercase();
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() <= 2 {
        return host;
    }
    let last_two = parts[parts.len() - 2..].join(".");
    if TWO_LEVEL.contains(last_two.as_str()) { parts[parts.len() - 3..].join(".") } else { last_two }
}

pub fn strip_www(host: &str) -> String {
    let h = host.to_lowercase();
    h.strip_prefix("www.").map(String::from).unwrap_or(h)
}

static URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)https?://[^\s"'<>()\[\]]+"#).unwrap());
static HOST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b((?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,10})\b").unwrap());
static VERSION_LIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+\.\d+").unwrap());

// File-ish "extensions" that must not be mistaken for top-level domains.
static NOT_TLDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "txt", "php", "html", "htm", "xml", "json", "js", "css", "md", "png", "jpg", "jpeg", "gif", "svg", "pdf", "csv", "ico", "yml", "yaml", "log",
        "zip", "gz", "aspx", "asp", "jsp", "cgi", "sql", "bak", "exe", "map", "py", "rb", "env", "ini", "conf", "cfg", "tsx", "ts", "jsx", "vue",
        "scss", "less", "woff", "ttf", "mp4", "mp3", "webp",
    ]
    .into_iter()
    .collect()
});

/// Absolute http(s) URLs found in free text, parsed and de-duplicated.
pub fn extract_urls(text: &str) -> Vec<Url> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for m in URL_RE.find_iter(text).flatten() {
        let raw = m.as_str().trim_end_matches(|c| ".,;:!?".contains(c));
        if let Ok(u) = Url::parse(raw) {
            if seen.insert(u.to_string()) {
                out.push(u);
            }
        }
    }
    out
}

/// Bare hostnames (no scheme) in free text, e.g. "dev-portal.example.com".
pub fn extract_hostnames(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in HOST_RE.captures_iter(text).flatten() {
        let host = m.get(1).unwrap().as_str().to_lowercase();
        let tld = host.rsplit('.').next().unwrap_or("");
        if NOT_TLDS.contains(tld) || VERSION_LIKE.is_match(&host).unwrap_or(false) || out.contains(&host) {
            continue;
        }
        out.push(host);
    }
    out
}

/// Hostname of a site URL, lowercased.
pub fn site_host(site_url: Option<&str>) -> Option<String> {
    Url::parse(site_url?).ok()?.host_str().map(|h| h.to_lowercase())
}

pub fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_private_ip(ip: &str) -> bool {
    let parts: Vec<u32> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 4 {
        return false;
    }
    let (a, b) = (parts[0], parts[1]);
    a == 10 || a == 127 || (a == 192 && b == 168) || (a == 172 && (16..=31).contains(&b)) || (a == 169 && b == 254)
}

/// Path plus query of a URL, the part robots.txt rules match against.
pub fn path_and_query(u: &Url) -> String {
    match u.query() {
        Some(q) => format!("{}?{}", u.path(), q),
        None => u.path().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains() {
        assert_eq!(root_domain("www.example.com"), "example.com");
        assert_eq!(root_domain("shop.example.co.uk"), "example.co.uk");
        assert_eq!(root_domain("example.co.uk"), "example.co.uk");
        assert_eq!(root_domain("cdn.assets.example.com.au"), "example.com.au");
        assert_eq!(root_domain("localhost"), "localhost");
        assert_eq!(root_domain("dev-portal.example.co.uk"), "example.co.uk");
        assert_eq!(extract_hostnames("see dev-portal.example.com and robots.txt v1.2.3"), vec!["dev-portal.example.com"]);
        assert!(is_private_ip("10.0.0.12"));
        assert!(!is_private_ip("8.8.8.8"));
    }
}
