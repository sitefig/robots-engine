//! Parses robots.txt text into a [`Model`], following RFC 9309 with the
//! lenient handling Google's parser applies (misspelt fields, comments, BOM).
//! Groups are delimited by User-agent lines only; blank lines do not end one.
//!
//! Besides the directives it also reports what the file should not contain at
//! all: HTML markup, plugin and server error output, injected spam, UTF-16
//! text. Those triggers come from a scan of the robots.txt files of 36 million
//! hosts (Common Crawl CC-MAIN-2026-34); each one is common enough in the wild
//! to be worth naming.

use crate::i18n::Locale;
use crate::model::*;
use crate::params;
use std::collections::HashMap;

pub const CRAWLER_READ_LIMIT: usize = 500 * 1024;
const CLEAN_PARAM_MAX_LENGTH: usize = 500;
/// Google reads a line of any length, but a line this long is generated, not written.
const LONG_LINE: usize = 2000;
/// A Crawl-delay at or above this keeps a crawler under 1,440 pages a day.
const HIGH_CRAWL_DELAY: f64 = 60.0;

/// Canonical directive names, for the misspelling search.
const CANONICAL: [&str; 7] = ["user-agent", "allow", "disallow", "crawl-delay", "sitemap", "host", "clean-param"];

/// Directives that are not RFC 9309 but are real proposals or old
/// conventions, reported as a note instead of "unknown directive".
const AI_DIRECTIVES: [&str; 8] = ["license", "disallowaitraining", "allowaitraining", "ai", "tdm-reservation", "tdmrep", "tdm-policy", "content-usage"];
/// Directives some crawlers once honoured; a note, not a warning.
const LEGACY_DIRECTIVES: [&str; 3] = ["request-rate", "visit-time", "schemamap"];
/// Meta-robots values written as if they were directives.
const META_ROBOTS: [&str; 13] = ["nofollow", "noarchive", "nosnippet", "noimageindex", "unavailable_after", "notranslate", "max-snippet", "max-image-preview", "max-video-preview", "noodp", "noydir", "index", "follow"];

/// Zero-width and invisible characters that break a directive name.
const INVISIBLE: [char; 6] = ['\u{feff}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{00ad}'];
/// Cyrillic letters that look like Latin ones in a field name.
const HOMOGLYPHS: [(char, char); 12] = [('\u{0430}', 'a'), ('\u{0410}', 'a'), ('\u{0435}', 'e'), ('\u{0415}', 'e'), ('\u{043e}', 'o'), ('\u{041e}', 'o'), ('\u{0440}', 'p'), ('\u{0420}', 'p'), ('\u{0441}', 'c'), ('\u{0421}', 'c'), ('\u{0443}', 'y'), ('\u{0445}', 'x')];

/// Extract the product token from a User-agent value: "Googlebot/2.1" -> "googlebot", "*" stays "*".
pub fn extract_token(value: &str) -> String {
    if value.starts_with('*') {
        return "*".into();
    }
    value.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect::<String>().to_lowercase()
}

fn alias(raw: &str) -> Option<&'static str> {
    Some(match raw {
        "user-agent" | "useragent" | "user agent" | "user_agent" | "user -agent" | "user - agent" => "user-agent",
        "allow" => "allow",
        "disallow" | "dissallow" | "disalow" | "dissalow" => "disallow",
        "crawl-delay" | "crawldelay" | "crawl delay" => "crawl-delay",
        "sitemap" | "site-map" | "sitemaps" => "sitemap",
        "host" => "host",
        "clean-param" | "cleanparam" => "clean-param",
        _ => return None,
    })
}

/// Levenshtein distance, capped: returns `max + 1` once it is certainly above.
fn edit_distance(a: &str, b: &str, max: usize) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > max {
        return max + 1;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The canonical directive a misspelt field means, or None when it is too far
/// off. Short names are not searched: "ai" is a directive of its own, not a
/// typo of "allow".
fn nearest_directive(raw: &str) -> Option<&'static str> {
    if raw.len() < 4 {
        return None;
    }
    let mut best: Option<(&'static str, usize)> = None;
    for name in CANONICAL {
        let d = edit_distance(raw, name, 2);
        if d <= 2 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((name, d));
        }
    }
    best.map(|(n, _)| n)
}

/// What had to be repaired before a field name could be read.
#[derive(Default)]
struct FieldRepairs {
    invisible: bool,
    homoglyph: bool,
    junk: Option<String>,
}

/// Clean a raw field name: drop invisible characters, fold look-alike
/// letters, and strip template or copy-paste junk before the name.
fn clean_field(raw: &str) -> (String, FieldRepairs) {
    let mut r = FieldRepairs::default();
    let mut s: String = raw
        .chars()
        .filter(|c| {
            let drop = INVISIBLE.contains(c);
            r.invisible |= drop;
            !drop
        })
        .map(|c| match HOMOGLYPHS.iter().find(|(from, _)| *from == c) {
            Some((_, to)) => {
                r.homoglyph = true;
                *to
            }
            None => c,
        })
        .collect::<String>()
        .trim()
        .to_lowercase();
    let junk_len = s.chars().take_while(|c| !c.is_ascii_alphanumeric()).count();
    if junk_len > 0 && junk_len < s.chars().count() {
        let junk: String = s.chars().take(junk_len).collect();
        // A field is a name; anything before it came from a template or a paste.
        if !junk.chars().all(|c| c == ' ') {
            r.junk = Some(junk.clone());
        }
        s = s.chars().skip(junk_len).collect();
    }
    (s, r)
}

pub struct Warner<'a> {
    pub locale: &'a Locale,
    pub kind: WarningKind,
    pub out: Vec<Warning>,
}

impl<'a> Warner<'a> {
    pub fn new(locale: &'a Locale, kind: WarningKind) -> Self {
        Warner { locale, kind, out: Vec::new() }
    }
    pub fn push(&mut self, level: Level, line: Option<u32>, id: &str, params: &crate::i18n::Params) {
        self.out.push(warning(self.locale, self.kind, level, line, id, params, None));
    }
    pub fn push_variant(&mut self, level: Level, line: Option<u32>, id: &str, params: &crate::i18n::Params, variant: &str) {
        self.out.push(warning(self.locale, self.kind, level, line, id, params, Some(variant)));
    }
    /// One finding of another kind than this warner's (security notes inside the parser).
    pub fn push_kind(&mut self, kind: WarningKind, level: Level, line: Option<u32>, id: &str, params: &crate::i18n::Params) {
        self.out.push(warning(self.locale, kind, level, line, id, params, None));
    }
    fn has(&self, id: &str) -> bool {
        self.out.iter().any(|w| w.id == id)
    }
}

/// Build one finding; `variant` selects `id.variant` for the wording while the id stays stable.
pub fn warning(locale: &Locale, kind: WarningKind, level: Level, line: Option<u32>, id: &str, params: &crate::i18n::Params, variant: Option<&str>) -> Warning {
    let key = match variant {
        Some(v) => format!("{id}.{v}"),
        None => id.to_string(),
    };
    Warning { level, kind, id: id.to_string(), line, message: locale.t(&key, params) }
}

/// Yandex syntax: "Clean-param: p0[&p1&...&pn] [path]".
pub fn parse_clean_param(value: &str) -> (Vec<String>, String, bool) {
    let mut it = value.split_whitespace();
    let params_part = it.next().unwrap_or("");
    let path = it.next().unwrap_or("/").to_string();
    let rest = it.count();
    let params: Vec<String> = params_part.split('&').filter(|p| !p.is_empty()).map(String::from).collect();
    let valid = rest == 0 && !params.is_empty() && params.iter().all(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))) && (path.starts_with('/') || path.starts_with('*'));
    (params, path, valid)
}

/// True when the text is a whole HTML document rather than a robots.txt with
/// markup in it; the fetch layer reports that case on its own.
pub fn looks_like_html_page(text: &str) -> bool {
    let head: String = text.chars().take(2000).collect::<String>().trim_start().to_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html") || head.starts_with("<?xml") && head.contains("<html")
}

/// UTF-16 text decoded from bytes that were read as UTF-8 keeps its NUL
/// padding, so every other character is a NUL. Returns the repaired text.
fn de_utf16(text: &str) -> Option<String> {
    let head: Vec<char> = text.chars().take(512).collect();
    if head.len() < 4 {
        return None;
    }
    let nuls = head.iter().filter(|c| **c == '\0').count();
    if nuls * 4 < head.len() {
        return None;
    }
    Some(text.chars().filter(|c| *c != '\0' && *c != '\u{feff}' && *c != '\u{fffe}').collect())
}

/// Markup that does not belong in a text file, with the tag that was found.
fn html_tag_in(lower: &str) -> Option<&'static str> {
    const TAGS: [&str; 18] = ["<script", "</script", "<a href", "<p>", "</p>", "<div", "<center", "<iframe", "<!--", "<br", "<span", "<img ", "<meta ", "<style", "<marquee", "<?php", "</body", "<table"];
    TAGS.into_iter().find(|t| lower.contains(t))
}

/// Output of a caching or minifying plugin appended to the file.
fn plugin_marker_in(lower: &str) -> Option<&'static str> {
    const MARKERS: [&str; 10] = ["total size saved", "load from cache", "page cache", "cached page generated", "served from cache", "wp super cache", "w3 total cache", "litespeed cache", "performance optimized by", "minified by"];
    MARKERS.into_iter().find(|m| lower.contains(m))
}

/// A PHP warning or a stack trace printed into the file.
fn server_error_in(lower: &str) -> Option<&'static str> {
    const MARKERS: [&str; 8] = ["<b>warning</b>:", "<b>notice</b>:", "<b>deprecated</b>:", "<b>fatal error</b>:", "php warning:", "php fatal error:", "stack trace:", "traceback (most recent call last)"];
    MARKERS.into_iter().find(|m| lower.contains(m))
}

/// Script, iframe or hidden-link markup: the signature of an SEO spam
/// injection. Returns the variant that matched, for the wording.
fn injected_code_in(lower: &str) -> Option<&'static str> {
    const OBFUSCATED: [&str; 7] = ["atob(", "eval(", "fromcharcode", "_0x", "document.write", "unescape(", "window.location"];
    const HIDING: [&str; 6] = ["display:none", "display: none", "width:0px", "width: 0px", "left:-", "left: -"];
    if lower.contains("<script") && OBFUSCATED.iter().any(|m| lower.contains(m)) {
        return Some("script");
    }
    if lower.contains("<iframe") {
        return Some("iframe");
    }
    if lower.contains("<marquee") {
        return Some("marquee");
    }
    if lower.contains("<a ") && HIDING.iter().any(|m| lower.contains(m)) {
        return Some("hiddenLink");
    }
    None
}

/// The first line holding `needle`, for a finding about the whole body.
fn line_of(lines: &[&str], needle: &str) -> Option<u32> {
    lines.iter().position(|l| l.to_lowercase().contains(needle)).map(|i| i as u32 + 1)
}

/// Findings about the file as a whole: markup, plugin and error output,
/// injected spam. Skipped when the response is an HTML page from end to end,
/// which the fetch layer reports instead.
fn body_checks(lines: &[&str], text: &str, w: &mut Warner) {
    if looks_like_html_page(text) {
        return;
    }
    let lower = text.to_lowercase();
    if injected_code_in(&lower).is_some() {
        w.push_kind(WarningKind::Security, Level::Error, None, "security.injectedCode", &[]);
    }
    if let Some(marker) = server_error_in(&lower) {
        w.push(Level::Error, line_of(lines, marker), "parser.serverErrorOutput", &[]);
    }
    if let Some(marker) = plugin_marker_in(&lower) {
        w.push(Level::Warning, line_of(lines, marker), "parser.pluginOutput", &[]);
    }
    if let Some(tag) = html_tag_in(&lower) {
        w.push(Level::Error, line_of(lines, tag), "parser.htmlFragment", &params! {"tag" => tag.trim_end()});
    }
    if text.contains('\u{fffd}') {
        w.push(Level::Warning, None, "parser.invalidUtf8", &[]);
    }
}

/// Directive names hidden in a comment because a line break is missing
/// before them, so the rule is silently lost.
fn glued_directive(comment_lower: &str) -> Option<&'static str> {
    const NAMES: [&str; 5] = ["user-agent:", "disallow:", "allow:", "sitemap:", "crawl-delay:"];
    for name in NAMES {
        let mut from = 0;
        while let Some(rel) = comment_lower[from..].find(name) {
            let at = from + rel;
            if at > 0 {
                let before = comment_lower[..at].chars().next_back().unwrap_or(' ');
                let glued = before.is_alphanumeric() || before == '.';
                // "disallow:" also ends with "allow:"; report it once, as disallow.
                let is_dis_tail = name == "allow:" && comment_lower[..at].ends_with("dis");
                if glued && !is_dis_tail {
                    return Some(name);
                }
            }
            from = at + name.len();
        }
    }
    None
}

/// A comment whose text is a rule: it does nothing, but it says what the
/// file used to do.
fn commented_rule(comment: &str) -> Option<&'static str> {
    const NAMES: [&str; 5] = ["user-agent:", "disallow:", "allow:", "sitemap:", "crawl-delay:"];
    let squeezed: String = comment.trim_start_matches('#').chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    NAMES.into_iter().find(|n| squeezed.starts_with(&n.replace(' ', "")))
}

/// Template placeholders nobody filled in.
fn placeholder_in(value_lower: &str) -> Option<&'static str> {
    const WORDS: [&str; 14] = ["example.com", "example.org", "example.net", "domainnamehere", "yourdomain", "your-domain", "yoursite", "your-site", "mydomain.com", "your_domain", "<name of spider>", "<path>", "<nothing>", "{{"];
    WORDS.into_iter().find(|p| value_lower.contains(p))
}

/// Regular-expression syntax in a path. `.*` is left out on purpose: in
/// robots.txt it is a literal dot followed by the `*` wildcard.
fn regex_syntax_in(value: &str) -> bool {
    const TOKENS: [&str; 7] = [r"\.", "^/", "(?", "[a-z", "[0-9", ".+", "|"];
    TOKENS.iter().any(|t| value.contains(t))
}

pub fn parse(text: &str, locale: &Locale) -> Model {
    let mut groups: Vec<Group> = Vec::new();
    let mut sitemaps = Vec::new();
    let mut clean_params = Vec::new();
    let mut host: Option<HostDirective> = None;
    let mut lines = Vec::new();
    let mut unknown = Vec::new();
    let mut w = Warner::new(locale, WarningKind::Syntax);

    if text.len() > CRAWLER_READ_LIMIT {
        w.push(Level::Warning, None, "parser.tooLarge", &params! {"kib" => text.len() / 1024});
    }
    let src = match text.strip_prefix('\u{feff}') {
        Some(s) => {
            w.push(Level::Info, Some(1), "parser.bom", &[]);
            s
        }
        None => text,
    };
    // UTF-16 arrives as NUL-padded text; repair it so every other check still runs.
    let repaired = de_utf16(src);
    if repaired.is_some() {
        w.push(Level::Error, Some(1), "parser.utf16", &[]);
    }
    let src: &str = repaired.as_deref().unwrap_or(src);

    let raw_lines = split_lines(src);
    body_checks(&raw_lines, src, &mut w);

    let mut collecting_agents = false;
    let mut crawl_delay_noted = false;

    for (i, raw) in raw_lines.iter().copied().enumerate() {
        let n = (i + 1) as u32;
        let mut entry = Line { n, raw: raw.to_string(), kind: LineKind::Blank, field: None, value: String::new() };
        if raw.chars().count() > LONG_LINE && !w.has("parser.longLine") {
            w.push(Level::Info, Some(n), "parser.longLine", &params! {"chars" => raw.chars().count()});
        }
        let hash = raw.find('#');
        // The full-width colon is a separator on keyboards that produce it.
        let content = match hash {
            Some(h) => &raw[..h],
            None => raw,
        }
        .replace('\u{ff1a}', ":");
        let had_fullwidth_colon = content.len() != raw[..hash.unwrap_or(raw.len())].len();
        let content = content.trim();
        if let Some(h) = hash {
            let comment = &raw[h..];
            let comment_lower = comment.to_lowercase();
            if let Some(directive) = glued_directive(&comment_lower) {
                w.push(Level::Error, Some(n), "parser.gluedComment", &params! {"directive" => directive.trim_end_matches(':')});
            } else if content.is_empty() {
                if let Some(directive) = commented_rule(comment) {
                    w.push(Level::Info, Some(n), "lint.commentedRule", &params! {"directive" => directive.trim_end_matches(':')});
                }
            }
        }
        if content.is_empty() {
            entry.kind = if hash.is_some() { LineKind::Comment } else { LineKind::Blank };
            lines.push(entry);
            continue;
        }
        let Some(colon) = content.find(':') else {
            entry.kind = LineKind::Invalid;
            match nearest_directive(&clean_field(content).0) {
                Some(d) => w.push_variant(Level::Error, Some(n), "parser.notFieldValue", &params! {"field" => d}, "hint"),
                None => w.push(Level::Error, Some(n), "parser.notFieldValue", &[]),
            }
            lines.push(entry);
            continue;
        };
        let (raw_field, repairs) = clean_field(&content[..colon]);
        let value = content[colon + 1..].trim().to_string();
        if repairs.invisible && !w.has("parser.invisibleChar") {
            w.push(Level::Warning, Some(n), "parser.invisibleChar", &[]);
        }
        if repairs.homoglyph {
            w.push(Level::Warning, Some(n), "parser.homoglyph", &params! {"field" => raw_field});
        }
        if had_fullwidth_colon && !w.has("parser.fullwidthColon") {
            w.push(Level::Warning, Some(n), "parser.fullwidthColon", &[]);
        }
        let mut field = alias(&raw_field);
        let mut misspelt = field.is_some() && field != Some(raw_field.as_str());
        if field.is_none() {
            // Everything that is not a directive of its own may be a typo.
            let known_other = AI_DIRECTIVES.contains(&raw_field.as_str()) || LEGACY_DIRECTIVES.contains(&raw_field.as_str()) || META_ROBOTS.contains(&raw_field.as_str()) || raw_field == "noindex" || raw_field.starts_with("ai-") || raw_field.starts_with("llm") || raw_field == "content-signal" || raw_field == "http" || raw_field == "https";
            if !known_other {
                if let Some(d) = nearest_directive(&raw_field) {
                    field = Some(d);
                    misspelt = true;
                }
            }
        }
        if let Some(f) = field {
            if misspelt {
                w.push(Level::Info, Some(n), "parser.misspelling", &params! {"raw" => raw_field, "field" => f});
            }
            if let Some(junk) = &repairs.junk {
                w.push(Level::Warning, Some(n), "parser.junkBeforeField", &params! {"junk" => junk, "field" => f});
            }
        }
        entry.field = Some(field.map(String::from).unwrap_or_else(|| raw_field.clone()));
        entry.value = value.clone();
        // A "#" glued to a value cuts the rule short: the rest is a comment.
        if matches!(field, Some("allow" | "disallow" | "sitemap")) && !value.is_empty() {
            if let Some(h) = hash {
                let before = raw[..h].chars().next_back().unwrap_or(' ');
                if !before.is_whitespace() {
                    let full = format!("{}{}", value, raw[h..].trim_end());
                    w.push(Level::Warning, Some(n), "parser.hashInValue", &params! {"value" => full, "truncated" => value});
                }
            }
        }
        match field {
            Some("user-agent") => {
                entry.kind = LineKind::UserAgent;
                if groups.is_empty() || !collecting_agents {
                    groups.push(Group { agents: Vec::new(), rules: Vec::new(), crawl_delay: None, start_line: n, end_line: n });
                    collecting_agents = true;
                }
                let group = groups.last_mut().unwrap();
                group.end_line = n;
                let token = extract_token(&value);
                if value.is_empty() {
                    w.push(Level::Error, Some(n), "parser.emptyUserAgent", &[]);
                } else if token.is_empty() {
                    w.push(Level::Warning, Some(n), "parser.invalidToken", &params! {"value" => value});
                } else if token != "*" && token != value.to_lowercase() {
                    w.push(Level::Info, Some(n), "parser.tokenTail", &params! {"token" => token, "value" => value});
                }
                if let Some(p) = placeholder_in(&value.to_lowercase()) {
                    w.push(Level::Warning, Some(n), "parser.placeholder", &params! {"value" => value, "placeholder" => p});
                }
                group.agents.push(Agent { raw: value, token, line: n });
            }
            Some(f @ ("allow" | "disallow")) => {
                entry.kind = LineKind::Rule;
                let rule_type = if f == "allow" { RuleType::Allow } else { RuleType::Disallow };
                if groups.is_empty() {
                    w.push(Level::Error, Some(n), "parser.ruleBeforeAgent", &params! {"directive" => rule_type.label()});
                    lines.push(entry);
                    continue;
                }
                collecting_agents = false;
                let group = groups.last_mut().unwrap();
                group.end_line = n;
                if value.is_empty() {
                    w.push(Level::Info, Some(n), if rule_type == RuleType::Allow { "parser.emptyAllow" } else { "parser.emptyDisallow" }, &[]);
                } else {
                    rule_value_checks(&value, n, rule_type, &mut w);
                }
                group.rules.push(Rule { rule_type, path: value, line: n });
            }
            Some("crawl-delay") => {
                entry.kind = LineKind::CrawlDelay;
                if groups.is_empty() {
                    w.push(Level::Error, Some(n), "parser.crawlDelayBeforeAgent", &[]);
                    lines.push(entry);
                    continue;
                }
                collecting_agents = false;
                let group = groups.last_mut().unwrap();
                group.end_line = n;
                match value.parse::<f64>() {
                    Ok(d) if d.is_finite() && d >= 0.0 => {
                        if group.crawl_delay.is_some() {
                            w.push(Level::Info, Some(n), "parser.crawlDelayDuplicate", &[]);
                        } else {
                            group.crawl_delay = Some(d);
                        }
                        if d >= HIGH_CRAWL_DELAY {
                            let pages = (86400.0 / d).floor() as i64;
                            w.push(Level::Warning, Some(n), "parser.crawlDelayHigh", &params! {"seconds" => d, "pages" => pages});
                        }
                    }
                    _ => w.push(Level::Warning, Some(n), "parser.crawlDelayNaN", &params! {"value" => value}),
                }
                if !crawl_delay_noted {
                    crawl_delay_noted = true;
                    w.push(Level::Info, Some(n), "parser.crawlDelayNonStandard", &[]);
                }
            }
            Some("sitemap") => {
                entry.kind = LineKind::Sitemap;
                let valid = url::Url::parse(&value).map(|u| u.scheme() == "http" || u.scheme() == "https").unwrap_or(false);
                if !valid {
                    w.push(Level::Warning, Some(n), "parser.sitemapInvalid", &params! {"value" => value});
                }
                if let Some(p) = placeholder_in(&value.to_lowercase()) {
                    w.push(Level::Warning, Some(n), "parser.placeholder", &params! {"value" => value, "placeholder" => p});
                }
                sitemaps.push(Sitemap { url: value, line: n, valid });
            }
            Some("host") => {
                entry.kind = LineKind::Host;
                if host.is_some() {
                    w.push(Level::Info, Some(n), "parser.hostDuplicate", &[]);
                } else {
                    host = Some(HostDirective { value, line: n });
                    w.push(Level::Info, Some(n), "parser.hostObsolete", &[]);
                }
            }
            Some("clean-param") => {
                entry.kind = LineKind::CleanParam;
                let (params, path, valid) = parse_clean_param(&value);
                if !valid {
                    w.push(Level::Warning, Some(n), "parser.cleanParamMalformed", &[]);
                } else if value.len() > CLEAN_PARAM_MAX_LENGTH {
                    w.push(Level::Warning, Some(n), "parser.cleanParamTooLong", &params! {"max" => CLEAN_PARAM_MAX_LENGTH});
                } else {
                    w.push(Level::Info, Some(n), "parser.cleanParamInfo", &[]);
                }
                clean_params.push(CleanParam { params, path, line: n, valid });
            }
            _ => {
                entry.kind = LineKind::Unknown;
                unknown_directive(&raw_field, &value, n, &mut w);
                unknown.push(Unknown { field: raw_field, value, line: n });
            }
        }
        lines.push(entry);
    }

    post_checks(&groups, &sitemaps, &mut w);

    Model { groups, sitemaps, host, clean_params, lines, warnings: w.out, unknown, size: text.len() }
}

/// Findings about one Allow/Disallow value.
fn rule_value_checks(value: &str, n: u32, rule_type: RuleType, w: &mut Warner) {
    if !value.starts_with('/') && !value.starts_with('*') && !is_absolute_url(value) {
        w.push(Level::Warning, Some(n), "parser.pathNoSlash", &params! {"value" => value});
    }
    if rule_type == RuleType::Disallow && value.chars().all(|c| c == '*') {
        w.push(Level::Warning, Some(n), "parser.wildcardOnly", &params! {"value" => value});
    }
    if value.contains(' ') {
        w.push(Level::Warning, Some(n), "parser.spaceInPath", &params! {"value" => value});
    }
    if !is_absolute_url(value) && regex_syntax_in(value) {
        w.push(Level::Warning, Some(n), "parser.regexSyntax", &params! {"value" => value});
    }
    if value.chars().any(|c| c > '\u{7f}') {
        w.push(Level::Info, Some(n), "parser.nonAsciiPath", &params! {"value" => value});
    }
    if let Some(p) = placeholder_in(&value.to_lowercase()) {
        w.push(Level::Warning, Some(n), "parser.placeholder", &params! {"value" => value, "placeholder" => p});
    }
}

/// A field none of the directives matched: name what it really is, so the
/// generic "unknown directive" stays for the cases nobody can explain.
fn unknown_directive(field: &str, value: &str, n: u32, w: &mut Warner) {
    if (field == "http" || field == "https") && value.starts_with("//") {
        let url = format!("{field}:{value}");
        w.push(Level::Error, Some(n), "parser.bareUrl", &params! {"url" => url});
        return;
    }
    if field == "noindex" {
        w.push(Level::Warning, Some(n), "parser.noindex", &[]);
        return;
    }
    if META_ROBOTS.contains(&field) {
        w.push(Level::Warning, Some(n), "parser.metaRobots", &params! {"field" => field});
        return;
    }
    if AI_DIRECTIVES.contains(&field) || field.starts_with("ai-") || field.starts_with("llm") {
        w.push(Level::Info, Some(n), "parser.aiDirective", &params! {"field" => field});
        return;
    }
    if field == "content-signal" {
        content_signal(value, n, w);
        return;
    }
    if LEGACY_DIRECTIVES.contains(&field) {
        w.push(Level::Info, Some(n), "parser.nonStandardDirective", &params! {"field" => field});
        return;
    }
    w.push(Level::Warning, Some(n), "parser.unknownDirective", &params! {"field" => field});
}

/// Cloudflare's Content Signals Policy: a list of `key=value` pairs, where
/// the keys are search, ai-input and ai-train and the values yes or no.
fn content_signal(value: &str, n: u32, w: &mut Warner) {
    const KEYS: [&str; 3] = ["search", "ai-input", "ai-train"];
    let mut bad: Vec<String> = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, val) = match part.split_once('=') {
            Some((k, v)) => (k.trim().to_lowercase(), v.trim().to_lowercase()),
            None => (part.to_lowercase(), String::new()),
        };
        if !KEYS.contains(&key.as_str()) || !matches!(val.as_str(), "yes" | "no") {
            bad.push(part.to_string());
        }
    }
    if bad.is_empty() {
        w.push(Level::Info, Some(n), "parser.contentSignal", &params! {"value" => value});
    } else {
        w.push(Level::Warning, Some(n), "parser.contentSignalInvalid", &params! {"parts" => bad.join(", ")});
    }
}

/// Split on CRLF, CR or LF like the browser did.
fn split_lines(src: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                out.push(&src[start..i]);
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                start = i + 1;
            }
            b'\n' => {
                out.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&src[start..]);
    out
}

fn post_checks(groups: &[Group], sitemaps: &[Sitemap], w: &mut Warner) {
    if groups.is_empty() {
        w.push(Level::Info, None, "parser.noRules", &[]);
    }
    for g in groups {
        if g.rules.is_empty() && g.crawl_delay.is_none() {
            w.push(Level::Info, Some(g.start_line), "parser.emptyGroup", &[]);
        }
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for g in groups {
        for a in &g.agents {
            if a.token.is_empty() {
                continue;
            }
            let e = seen.entry(&a.token).or_insert_with(|| {
                order.push(&a.token);
                0
            });
            *e += 1;
        }
    }
    for token in order {
        let count = seen[token];
        if count > 1 {
            w.push(Level::Info, None, "parser.tokenRepeated", &params! {"token" => token, "n" => count});
        }
    }
    let star_rules: Vec<&Rule> = groups.iter().filter(|g| g.agents.iter().any(|a| a.token == "*")).flat_map(|g| g.rules.iter()).collect();
    let blocks_all = star_rules.iter().any(|r| r.rule_type == RuleType::Disallow && whole_site(&r.path));
    let has_allow = star_rules.iter().any(|r| r.rule_type == RuleType::Allow && !r.path.is_empty());
    if blocks_all && !has_allow {
        w.push(Level::Warning, None, "parser.starBlocksAll", &[]);
    }
    if sitemaps.is_empty() && !groups.is_empty() {
        w.push(Level::Info, None, "parser.noSitemap", &[]);
    }
}
