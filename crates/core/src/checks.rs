//! Lint checks that produce findings in the parser's shape: SEO traps,
//! sitemap hygiene and absolute URLs in rules. Each runs only when enabled in
//! the config; `run_checks` concatenates them.

use crate::analyser::{groups_for_token, path_matches, rule_len};
use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::*;
use crate::params;
use crate::parser::Warner;
use crate::url_util::{root_domain, strip_www};
use url::Url;

/// "Disallow: /shop" is a prefix, so it also hits /shopping and /shop-locator.
/// A warning when the file itself shows the prefix has siblings (another rule
/// continues the same path), a note otherwise: most CMS defaults write their
/// paths without the slash and mean exactly that one directory.
pub fn trailing_slash_traps(model: &Model, w: &mut Warner) {
    let all: Vec<&Rule> = model.groups.iter().flat_map(|g| g.rules.iter()).collect();
    for group in &model.groups {
        for rule in &group.rules {
            let p = &rule.path;
            if p.is_empty() || whole_site(p) || p.contains('*') || p.contains('$') || p.contains('?') || p.ends_with('/') {
                continue;
            }
            let last = &p[p.rfind('/').map(|i| i + 1).unwrap_or(0)..];
            if last.is_empty() || last.contains('.') {
                continue;
            }
            let sibling = all.iter().any(|r| {
                let q = &r.path;
                q.len() > p.len() && q.starts_with(p.as_str()) && !q[p.len()..].starts_with('/')
            });
            let level = if sibling { Level::Warning } else { Level::Info };
            let variant = if rule.rule_type == RuleType::Allow { "allow" } else { "disallow" };
            w.push_variant(level, Some(rule.line), "seo.trailingSlash", &params! {"path" => p}, variant);
        }
    }
}

/// Rules of every group that the `*` token is in: what a crawler without its
/// own group has to follow.
fn star_rules(model: &Model) -> Vec<&Rule> {
    model.groups.iter().filter(|g| g.agents.iter().any(|a| a.token == "*")).flat_map(|g| g.rules.iter()).filter(|r| r.rule_type == RuleType::Disallow && !r.path.is_empty()).collect()
}

/// What the `*` group blocks for everyone: query strings, assets, images.
/// Only the `*` group, because a rule aimed at one named crawler is a choice.
pub fn broad_blocks(model: &Model, w: &mut Warner) {
    const QUERY: [&str; 6] = ["/*?", "/*?*", "/?*", "*?", "*?*", "/*?$"];
    const ASSET_DIRS: [&str; 4] = ["/js/", "/css/", "/js", "/css"];
    const IMAGE_DIRS: [&str; 6] = ["/images/", "/images", "/img/", "/image/", "/wp-content/uploads/", "/uploads/"];
    const IMAGE_FILES: [&str; 9] = [".jpg$", ".jpeg$", ".png$", ".gif$", ".webp$", "*.jpg", "*.png", "*.jpeg", "*.gif"];
    for rule in star_rules(model) {
        let p = rule.path.to_lowercase();
        if QUERY.contains(&p.as_str()) {
            w.push(Level::Warning, Some(rule.line), "seo.queryStringBlock", &params! {"path" => rule.path});
            continue;
        }
        if ASSET_DIRS.contains(&p.as_str()) {
            w.push(Level::Warning, Some(rule.line), "seo.assetBlock", &params! {"path" => rule.path});
            continue;
        }
        if p.ends_with(".css") || p.ends_with(".js") || p.ends_with(".css$") || p.ends_with(".js$") {
            w.push(Level::Warning, Some(rule.line), "seo.cssJsBlock", &params! {"path" => rule.path});
            continue;
        }
        if IMAGE_DIRS.contains(&p.as_str()) || IMAGE_FILES.iter().any(|s| p.ends_with(s)) {
            w.push(Level::Info, Some(rule.line), "seo.imageBlock", &params! {"path" => rule.path});
        }
    }
}

/// A Disallow that matches /robots.txt itself.
pub fn self_blocks(model: &Model, w: &mut Warner) {
    for group in &model.groups {
        for rule in &group.rules {
            if rule.rule_type != RuleType::Disallow || rule.path.is_empty() || whole_site(&rule.path) || !path_matches(&rule.path, "/robots.txt") {
                continue;
            }
            w.push(Level::Warning, Some(rule.line), "seo.selfBlock", &params! {"path" => rule.path});
        }
    }
}

/// Paths are case-sensitive; an uppercase letter is worth a second look.
pub fn case_notes(model: &Model, w: &mut Warner) {
    for group in &model.groups {
        for rule in &group.rules {
            if !rule.path.chars().any(|c| c.is_ascii_uppercase()) {
                continue;
            }
            w.push(Level::Info, Some(rule.line), "seo.caseSensitive", &params! {"path" => rule.path, "lower" => rule.path.to_lowercase()});
        }
    }
}

/// Highest-precedence rule among candidates: longest, Allow on a tie, earliest line on a full tie.
fn winner<'a>(mut candidates: Vec<&'a Rule>) -> &'a Rule {
    candidates.sort_by(|a, b| {
        rule_len(b).cmp(&rule_len(a)).then_with(|| {
            if a.rule_type == b.rule_type {
                std::cmp::Ordering::Equal
            } else if a.rule_type == RuleType::Allow {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }).then_with(|| a.line.cmp(&b.line))
    });
    candidates[0]
}

/// Rules that never change the outcome: duplicates, rules covered by a
/// broader rule of the same type, rules always overridden by a stronger one.
pub fn shadowed_rules(model: &Model, w: &mut Warner) {
    let mut seen: Vec<u32> = Vec::new();
    let locale = w.locale;
    let mut check = |rules: &[&Rule], scope: String, w: &mut Warner| {
        for b in rules {
            if b.path.is_empty() || b.path.contains('*') || seen.contains(&b.line) {
                continue;
            }
            let anchored = b.path.ends_with('$');
            let literal = if anchored { &b.path[..b.path.len() - 1] } else { &b.path[..] };
            let candidates: Vec<&Rule> = rules
                .iter()
                .copied()
                .filter(|r| {
                    if std::ptr::eq(*r, *b) || r.path.is_empty() {
                        return false;
                    }
                    if !anchored && r.path.ends_with('$') {
                        return false;
                    }
                    if r.rule_type == b.rule_type && r.path == b.path && r.line > b.line {
                        return false;
                    }
                    path_matches(&r.path, literal)
                })
                .collect();
            if candidates.is_empty() {
                continue;
            }
            let win = winner(candidates);
            let identical = win.rule_type == b.rule_type && win.path == b.path;
            let p = params! {"rule" => b.label(), "winner" => win.label(), "line" => win.line, "scope" => scope};
            if identical {
                w.push(Level::Info, Some(b.line), "seo.duplicate", &p);
                seen.push(b.line);
            } else if win.rule_type == b.rule_type {
                w.push(Level::Info, Some(b.line), "seo.redundant", &p);
                seen.push(b.line);
            } else if rule_len(win) > rule_len(b) || (rule_len(win) == rule_len(b) && win.rule_type == RuleType::Allow) {
                w.push(Level::Warning, Some(b.line), "seo.overridden", &p);
                seen.push(b.line);
            }
        }
    };
    for group in &model.groups {
        let rules: Vec<&Rule> = group.rules.iter().collect();
        check(&rules, String::new(), w);
    }
    let mut tokens: Vec<&str> = Vec::new();
    for g in &model.groups {
        for a in &g.agents {
            if !a.token.is_empty() && !tokens.contains(&a.token.as_str()) {
                tokens.push(&a.token);
            }
        }
    }
    for token in tokens {
        let groups = groups_for_token(model, token);
        if groups.len() < 2 {
            continue;
        }
        let rules: Vec<&Rule> = groups.iter().flat_map(|g| g.rules.iter()).collect();
        check(&rules, locale.t("seo.mergedScope", &params! {"token" => token}), w);
    }
}

pub fn seo_traps(model: &Model, locale: &Locale) -> Vec<Warning> {
    let mut w = Warner::new(locale, WarningKind::SeoTrap);
    trailing_slash_traps(model, &mut w);
    self_blocks(model, &mut w);
    case_notes(model, &mut w);
    broad_blocks(model, &mut w);
    shadowed_rules(model, &mut w);
    w.out
}

/// Block lists of 1990s offline downloaders and e-mail harvesters, copied
/// from site to site for decades. None of those tools read robots.txt.
pub fn legacy_bad_bots(model: &Model, engine: &Engine, locale: &Locale) -> Vec<Warning> {
    const THRESHOLD: usize = 10;
    let legacy = &engine.legacy_tokens;
    if legacy.is_empty() {
        return Vec::new();
    }
    let mut w = Warner::new(locale, WarningKind::Lint);
    let mut hits: Vec<String> = Vec::new();
    let mut first_line = None;
    for group in &model.groups {
        for agent in &group.agents {
            let name = agent.raw.trim().to_lowercase();
            if legacy.contains(&name) && !hits.contains(&name) {
                hits.push(name);
                first_line.get_or_insert(agent.line);
            }
        }
    }
    if hits.len() >= THRESHOLD {
        w.push(Level::Info, first_line, "lint.legacyBadBots", &params! {"n" => hits.len()});
    }
    w.out
}

/// Sitemap URL hygiene: protocol mismatch with the site, sitemaps on another
/// host or domain, duplicate lines. Without a site URL only duplicates are reported.
pub fn sitemap_checks(model: &Model, site_url: Option<&str>, locale: &Locale) -> Vec<Warning> {
    let mut w = Warner::new(locale, WarningKind::Sitemap);
    let site = site_url.and_then(|s| Url::parse(s).ok());
    let mut seen: Vec<(String, u32)> = Vec::new();
    for s in &model.sitemaps {
        if !s.valid {
            continue;
        }
        let Ok(u) = Url::parse(&s.url) else { continue };
        let key = u.to_string();
        if let Some((_, line)) = seen.iter().find(|(k, _)| *k == key) {
            w.push(Level::Info, Some(s.line), "sitemap.duplicate", &params! {"line" => line});
            continue;
        }
        seen.push((key, s.line));
        let Some(site) = &site else { continue };
        let (Some(site_host), Some(map_host)) = (site.host_str(), u.host_str()) else { continue };
        if site.scheme() == "https" && u.scheme() == "http" {
            w.push(Level::Warning, Some(s.line), "sitemap.protocolMismatch", &params! {"url" => u.to_string().replacen("http:", "https:", 1)});
        } else if site.scheme() == "http" && u.scheme() == "https" {
            w.push(Level::Info, Some(s.line), "sitemap.httpsOnHttp", &[]);
        }
        if strip_www(site_host) == strip_www(map_host) {
            continue;
        }
        let p = params! {"host" => map_host, "site" => site_host};
        if root_domain(site_host) != root_domain(map_host) {
            w.push(Level::Warning, Some(s.line), "sitemap.crossDomain", &p);
        } else {
            w.push(Level::Info, Some(s.line), "sitemap.otherSubdomain", &p);
        }
    }
    w.out
}

/// Absolute URLs in Allow/Disallow never match anything.
pub fn absolute_rule_check(model: &Model, locale: &Locale) -> Vec<Warning> {
    let mut w = Warner::new(locale, WarningKind::SeoTrap);
    for r in model.each_rule() {
        if !is_absolute_url(&r.rule.path) {
            continue;
        }
        let path_only = Url::parse(&r.rule.path).map(|u| crate::url_util::path_and_query(&u)).unwrap_or_default();
        let p = params! {"rule" => r.rule.label(), "path" => path_only};
        if path_only.is_empty() {
            w.push(Level::Warning, Some(r.rule.line), "lint.absoluteUrl", &p);
        } else {
            w.push_variant(Level::Warning, Some(r.rule.line), "lint.absoluteUrl", &p, "hint");
        }
    }
    w.out
}

/// Every enabled check, in order.
pub fn run_checks(model: &Model, site_url: Option<&str>, engine: &Engine, locale: &Locale) -> Vec<Warning> {
    let mut out = Vec::new();
    if engine.config.checks.seo_traps {
        out.extend(seo_traps(model, locale));
    }
    if engine.config.checks.sitemaps {
        out.extend(sitemap_checks(model, site_url, locale));
    }
    if engine.config.checks.absolute_urls {
        out.extend(absolute_rule_check(model, locale));
    }
    if engine.config.checks.seo_traps {
        out.extend(legacy_bad_bots(model, engine, locale));
    }
    out
}

/// Apply `[rules]` from the config: drop disabled ids, override levels.
pub fn apply_rule_overrides(warnings: &mut Vec<Warning>, engine: &Engine) {
    warnings.retain(|w| !engine.disabled_ids.contains(&w.id));
    for w in warnings.iter_mut() {
        if let Some(l) = engine.level_overrides.get(&w.id) {
            w.level = *l;
        }
    }
}
