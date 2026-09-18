//! Parses robots.txt text into a [`Model`], following RFC 9309 with the
//! lenient handling Google's parser applies (misspelt fields, comments, BOM).
//! Groups are delimited by User-agent lines only; blank lines do not end one.

use crate::i18n::Locale;
use crate::model::*;
use crate::params;
use std::collections::HashMap;

pub const CRAWLER_READ_LIMIT: usize = 500 * 1024;
const CLEAN_PARAM_MAX_LENGTH: usize = 500;

/// Extract the product token from a User-agent value: "Googlebot/2.1" -> "googlebot", "*" stays "*".
pub fn extract_token(value: &str) -> String {
    if value.starts_with('*') {
        return "*".into();
    }
    value.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect::<String>().to_lowercase()
}

fn alias(raw: &str) -> Option<&'static str> {
    Some(match raw {
        "user-agent" | "useragent" | "user agent" => "user-agent",
        "allow" => "allow",
        "disallow" | "dissallow" | "disalow" | "dissalow" => "disallow",
        "crawl-delay" | "crawldelay" => "crawl-delay",
        "sitemap" | "site-map" => "sitemap",
        "host" => "host",
        "clean-param" => "clean-param",
        _ => return None,
    })
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

    let mut collecting_agents = false;
    let mut crawl_delay_noted = false;

    for (i, raw) in split_lines(src).into_iter().enumerate() {
        let n = (i + 1) as u32;
        let mut entry = Line { n, raw: raw.to_string(), kind: LineKind::Blank, field: None, value: String::new() };
        let content = match raw.find('#') {
            Some(h) => &raw[..h],
            None => raw,
        }
        .trim();
        if content.is_empty() {
            entry.kind = if raw.contains('#') { LineKind::Comment } else { LineKind::Blank };
            lines.push(entry);
            continue;
        }
        let Some(colon) = content.find(':') else {
            entry.kind = LineKind::Invalid;
            w.push(Level::Error, Some(n), "parser.notFieldValue", &[]);
            lines.push(entry);
            continue;
        };
        let raw_field = content[..colon].trim().to_lowercase();
        let value = content[colon + 1..].trim().to_string();
        let field = alias(&raw_field);
        entry.field = Some(field.map(String::from).unwrap_or_else(|| raw_field.clone()));
        entry.value = value.clone();
        if let Some(f) = field {
            if f != raw_field {
                w.push(Level::Info, Some(n), "parser.misspelling", &params! {"raw" => raw_field, "field" => f});
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
                } else if !value.starts_with('/') && !value.starts_with('*') && !is_absolute_url(&value) {
                    w.push(Level::Warning, Some(n), "parser.pathNoSlash", &params! {"value" => value});
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
                if raw_field == "noindex" {
                    w.push(Level::Warning, Some(n), "parser.noindex", &[]);
                } else {
                    w.push(Level::Warning, Some(n), "parser.unknownDirective", &params! {"field" => raw_field});
                }
                unknown.push(Unknown { field: raw_field, value, line: n });
            }
        }
        lines.push(entry);
    }

    post_checks(&groups, &sitemaps, &mut w);

    Model { groups, sitemaps, host, clean_params, lines, warnings: w.out, unknown, size: text.len() }
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
