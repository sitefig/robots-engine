mod common;
use common::*;
use susbot_core::checks::*;
use susbot_core::model::{Level, Warning};
use susbot_core::parser::Warner;

fn traps(text: &str) -> Vec<Warning> {
    seo_traps(&model(text), &en())
}

fn only(f: fn(&susbot_core::model::Model, &mut Warner), lines: &[&str]) -> Vec<Warning> {
    let l = en();
    let mut w = Warner::new(&l, susbot_core::model::WarningKind::SeoTrap);
    f(&rules(lines), &mut w);
    w.out
}

#[test]
fn trailing_slash_trap() {
    let w = only(trailing_slash_traps, &["Disallow: /shop", "Disallow: /shop/", "Disallow: /shop$", "Disallow: /shop*", "Disallow: /file.pdf", "Disallow: /", "Disallow: /a?b", "Allow: /blog"]);
    assert_eq!(w.iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [2, 9]);
    assert!(w[0].message.contains("/shops, /shop-old and /shop2"));
    assert!(w[0].message.contains("\"/shop/\""));
    assert!(w[1].message.contains("also allows"));
    assert_eq!(w[0].id, "seo.trailingSlash");
}

#[test]
fn self_block_and_case() {
    assert_eq!(only(self_blocks, &["Disallow: /robots.txt"]).len(), 1);
    assert_eq!(only(self_blocks, &["Disallow: /*.txt$"]).len(), 1);
    assert_eq!(only(self_blocks, &["Disallow: /rob"]).len(), 1);
    assert_eq!(only(self_blocks, &["Disallow: /"]).len(), 0);
    assert_eq!(only(self_blocks, &["Disallow: /admin", "Allow: /robots.txt"]).len(), 0);
    let w = only(case_notes, &["Disallow: /Admin/", "Disallow: /admin/", "Allow: /Files/PDF"]);
    assert_eq!(w.iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [2, 4]);
    assert!(w[0].message.contains("\"/admin/\""));
}

#[test]
fn shadowed_rules_cases() {
    let w = only(shadowed_rules, &["Disallow: /blog", "Disallow: /blog/post"]);
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].line, w[0].id.as_str()), (Some(3), "seo.redundant"));
    assert!(w[0].message.contains("line 2"));
    assert!(only(shadowed_rules, &["Disallow: /blog", "Allow: /blog/p", "Disallow: /blog/post"]).is_empty());
    let w = only(shadowed_rules, &["Disallow: /a", "Disallow: /b", "Disallow: /a"]);
    assert_eq!((w.len(), w[0].line, w[0].id.as_str()), (1, Some(4), "seo.duplicate"));
    let w = only(shadowed_rules, &["Disallow: /blog", "Disallow: /blog/post", "Disallow: /blog/post"]);
    assert_eq!(w.iter().map(|x| (x.line.unwrap(), x.id.as_str())).collect::<Vec<_>>(), [(3, "seo.redundant"), (4, "seo.duplicate")]);
    let w = only(shadowed_rules, &["Allow: /x", "Disallow: /x"]);
    assert_eq!((w.len(), w[0].line, w[0].id.as_str()), (1, Some(3), "seo.overridden"));
    assert_eq!(only(shadowed_rules, &["Disallow: /x", "Allow: /x"]).iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [2]);
    assert_eq!(only(shadowed_rules, &["Disallow: /blog", "Allow: /blog*"]).iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [2]);
    assert!(only(shadowed_rules, &["Disallow: /a$", "Disallow: /a/b"]).is_empty());
    assert_eq!(only(shadowed_rules, &["Disallow: /a", "Disallow: /a$"]).iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [3]);
    assert!(only(shadowed_rules, &["Disallow: /a", "Disallow: /a*b"]).is_empty());
    let l = en();
    let mut w = Warner::new(&l, susbot_core::model::WarningKind::SeoTrap);
    shadowed_rules(&model("User-agent: bot\nDisallow: /a\n\nUser-agent: other\nDisallow: /a/b\n\nUser-agent: bot\nDisallow: /a/c"), &mut w);
    let w = w.out;
    assert_eq!(w.iter().map(|x| x.line.unwrap()).collect::<Vec<_>>(), [8]);
    assert!(w[0].message.contains("merged across groups"));
}

#[test]
fn seo_traps_combined() {
    assert!(traps("User-agent: *\nDisallow: /admin/\nDisallow: /tmp/\nAllow: /tmp/public/").is_empty());
    let all = traps("User-agent: *\nDisallow: /Shop\nDisallow: /robots.txt\nDisallow: /Shop/x");
    let ids: Vec<&str> = all.iter().map(|w| w.id.as_str()).collect();
    for id in ["seo.trailingSlash", "seo.selfBlock", "seo.caseSensitive", "seo.redundant"] {
        assert!(ids.contains(&id), "{id}");
    }
    assert!(all.iter().all(|w| w.message != w.id));
}

fn with_maps(urls: &[&str]) -> susbot_core::model::Model {
    model(&format!("User-agent: *\nDisallow: /a/\n{}", urls.iter().map(|u| format!("Sitemap: {u}")).collect::<Vec<_>>().join("\n")))
}

#[test]
fn sitemap_hygiene() {
    let l = en();
    let w = sitemap_checks(&with_maps(&["http://example.com/sitemap.xml"]), Some("https://example.com/robots.txt"), &l);
    assert_eq!((w.len(), w[0].level, w[0].line), (1, Level::Warning, Some(3)));
    assert!(w[0].message.contains("https://example.com/sitemap.xml"));
    let w = sitemap_checks(&with_maps(&["https://example.com/sitemap.xml"]), Some("http://example.com/robots.txt"), &l);
    assert_eq!((w.len(), w[0].level), (1, Level::Info));
    assert!(sitemap_checks(&with_maps(&["https://www.example.com/sitemap.xml"]), Some("https://example.com/robots.txt"), &l).is_empty());
    assert!(sitemap_checks(&with_maps(&["https://example.com/sitemap.xml"]), Some("https://www.example.com/robots.txt"), &l).is_empty());
    let w = sitemap_checks(&with_maps(&["https://cdn-host.net/sitemap.xml"]), Some("https://example.com/robots.txt"), &l);
    assert_eq!((w.len(), w[0].level, w[0].id.as_str()), (1, Level::Warning, "sitemap.crossDomain"));
    assert!(w[0].message.contains("Search Console"));
    let w = sitemap_checks(&with_maps(&["https://sitemaps.example.com/sitemap.xml"]), Some("https://www.example.com/robots.txt"), &l);
    assert_eq!((w.len(), w[0].level), (1, Level::Info));
    let w = sitemap_checks(&with_maps(&["https://other.co.uk/sitemap.xml"]), Some("https://example.co.uk/robots.txt"), &l);
    assert_eq!(w[0].level, Level::Warning);
    let w = sitemap_checks(&with_maps(&["http://other.com/sitemap.xml"]), Some("https://example.com/robots.txt"), &l);
    assert_eq!(w.iter().map(|x| x.level).collect::<Vec<_>>(), [Level::Warning, Level::Warning]);
    let w = sitemap_checks(&with_maps(&["https://example.com/a.xml", "https://example.com/a.xml", "/relative.xml"]), Some("https://example.com/robots.txt"), &l);
    assert_eq!((w.len(), w[0].id.as_str(), w[0].line), (1, "sitemap.duplicate", Some(4)));
    let w = sitemap_checks(&with_maps(&["http://other.com/a.xml", "http://other.com/a.xml"]), None, &l);
    assert_eq!((w.len(), w[0].id.as_str()), (1, "sitemap.duplicate"));
    assert!(sitemap_checks(&with_maps(&["http://other.com/a.xml"]), None, &l).is_empty());
}

#[test]
fn absolute_urls_and_run_checks() {
    let w = absolute_rule_check(&rules(&["Disallow: https://beta.example.com/checkout/", "Disallow: /ok/"]), &en());
    assert_eq!((w.len(), w[0].level, w[0].id.as_str()), (1, Level::Warning, "lint.absoluteUrl"));
    assert!(w[0].message.contains("\"/checkout/\""));
    let e = engine();
    let m = model("User-agent: *\nDisallow: /shop\nSitemap: http://example.com/s.xml");
    let w = run_checks(&m, Some("https://example.com/robots.txt"), &e, &en());
    assert!(w.iter().any(|x| x.id == "seo.trailingSlash" && x.message.starts_with("Trailing slash trap")));
    assert!(w.iter().any(|x| x.id == "sitemap.protocolMismatch"));
    assert!(run_checks(&model("User-agent: *\nDisallow: /a/\nSitemap: http://other.com/s.xml"), None, &e, &en()).is_empty());
}

#[test]
fn rule_overrides_and_disabled_checks() {
    let e = susbot_core::Engine::from_toml(Some("[rules]\ndisabled = [\"seo.caseSensitive\"]\n[rules.levels]\n\"seo.trailingSlash\" = \"info\"\n[checks]\nsitemaps = false\n")).unwrap();
    let m = model("User-agent: *\nDisallow: /Shop\nSitemap: http://example.com/s.xml");
    let mut w = run_checks(&m, Some("https://example.com/robots.txt"), &e, &en());
    apply_rule_overrides(&mut w, &e);
    assert!(!w.iter().any(|x| x.id == "seo.caseSensitive"));
    assert!(!w.iter().any(|x| x.id == "sitemap.protocolMismatch"));
    assert_eq!(w.iter().find(|x| x.id == "seo.trailingSlash").unwrap().level, Level::Info);
}

#[test]
fn what_the_star_group_blocks_for_everyone() {
    let e = engine();
    let ids = |text: &str| run_checks(&model(text), None, &e, &en()).into_iter().map(|w| w.id).collect::<Vec<_>>();
    assert!(ids("User-agent: *\nDisallow: /*?\n").contains(&"seo.queryStringBlock".to_string()));
    assert!(ids("User-agent: *\nDisallow: /js/\n").contains(&"seo.assetBlock".to_string()));
    assert!(ids("User-agent: *\nDisallow: /*.css\n").contains(&"seo.cssJsBlock".to_string()));
    assert!(ids("User-agent: *\nDisallow: /images/\n").contains(&"seo.imageBlock".to_string()));
    // A rule aimed at one named crawler is a choice, not a trap.
    assert!(!ids("User-agent: AhrefsBot\nDisallow: /*?\n").contains(&"seo.queryStringBlock".to_string()));
    assert!(!ids("User-agent: *\nAllow: /images/\n").contains(&"seo.imageBlock".to_string()));
}

#[test]
fn the_trailing_slash_trap_is_a_warning_only_with_siblings() {
    let e = engine();
    let level = |text: &str| run_checks(&model(text), None, &e, &en()).into_iter().find(|w| w.id == "seo.trailingSlash").map(|w| w.level);
    // /shop and /shop-old in one file: the prefix really does catch siblings.
    assert_eq!(level("User-agent: *\nDisallow: /shop\nDisallow: /shop-old\n"), Some(Level::Warning));
    // On its own it is how most platforms write a directory rule.
    assert_eq!(level("User-agent: *\nDisallow: /shop\n"), Some(Level::Info));
}

#[test]
fn a_legacy_bad_bot_list_is_reported_once() {
    let e = engine();
    let names = ["WebCopier", "HTTrack", "EmailSiphon", "WebZIP", "Teleport", "TeleportPro", "Zeus", "WebStripper", "SiteSnagger", "Offline Explorer", "NetAnts"];
    let text: String = names.iter().map(|n| format!("User-agent: {n}\nDisallow: /\n")).collect();
    let w = run_checks(&model(&text), None, &e, &en());
    let hits: Vec<&susbot_core::model::Warning> = w.iter().filter(|x| x.id == "lint.legacyBadBots").collect();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].message.contains("11"));
    // Blocking one or two named tools is not a copied block list.
    let few = run_checks(&model("User-agent: HTTrack\nDisallow: /\n"), None, &e, &en());
    assert!(!few.iter().any(|x| x.id == "lint.legacyBadBots"));
}

#[test]
fn how_the_file_was_served() {
    use susbot_core::fetch::{fetch_warnings, FetchInfo};
    let base = |status: u16, content_type: &str, text: &str| FetchInfo {
        source: "cli".into(),
        robots_url: "https://example.com/robots.txt".into(),
        final_url: "https://example.com/robots.txt".into(),
        status,
        status_text: None,
        content_type: (!content_type.is_empty()).then(|| content_type.to_string()),
        bytes: text.len(),
        truncated: false,
        redirects: vec![],
        redirect_limit: false,
        text: text.into(),
    };
    let ids = |f: FetchInfo| fetch_warnings(&f, &en()).into_iter().map(|w| w.id).collect::<Vec<_>>();
    assert!(ids(base(503, "text/plain", "")).contains(&"fetch.warn.serverError".to_string()));
    assert!(ids(base(429, "text/plain", "")).contains(&"fetch.warn.rateLimited".to_string()));
    assert!(ids(base(403, "text/plain", "")).contains(&"fetch.warn.forbidden".to_string()));
    assert!(ids(base(200, "text/html", "User-agent: *\n")).contains(&"fetch.warn.contentType".to_string()));
    assert!(ids(base(200, "", "User-agent: *\n")).contains(&"fetch.warn.contentTypeMissing".to_string()));
    assert!(ids(base(200, "text/html", "<!DOCTYPE html>\n<html><body>Not found</body></html>")).contains(&"fetch.warn.htmlBody".to_string()));
    // A file served correctly says nothing.
    assert_eq!(ids(base(200, "text/plain; charset=utf-8", "User-agent: *\nDisallow: /a\n")), Vec::<String>::new());
    // A status finding still appears when the body is unusable.
    assert!(ids(base(500, "text/html", "<html>error</html>")).contains(&"fetch.warn.serverError".to_string()));
}
