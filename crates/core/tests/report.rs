mod common;
use common::*;
use susbot_core::ai_status::ai_status;
use susbot_core::analyser::Verdict;
use susbot_core::fetch::{FetchInfo, Redirect};
use susbot_core::i18n::Locale;
use susbot_core::model::Level;
use susbot_core::{Analysis, Options};

fn fetch_info() -> FetchInfo {
    FetchInfo {
        source: "proxy".into(),
        robots_url: "https://example.com/robots.txt".into(),
        final_url: SITE.into(),
        status: 200,
        status_text: Some("OK".into()),
        content_type: Some("text/plain".into()),
        bytes: KITCHEN_SINK.len(),
        truncated: false,
        redirects: vec![Redirect { from: "https://example.com/robots.txt".into(), status: Some(301), to: SITE.into() }],
        redirect_limit: false,
        text: KITCHEN_SINK.into(),
    }
}

fn full() -> Analysis {
    Analysis::new(KITCHEN_SINK, Options { site_url: Some(SITE.into()), fetch: Some(fetch_info()), schema_url: Some("https://example.org/schema/report.schema.json".into()), now: Some("2026-09-16T10:00:00Z".into()), ..Default::default() }).unwrap()
}

#[test]
fn kitchen_sink_report_validates_against_the_schema() {
    let a = full();
    let json: serde_json::Value = serde_json::from_str(&a.report_json()).unwrap();
    let schema: serde_json::Value = serde_json::from_str(include_str!("../../../schema/report.schema.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator.iter_errors(&json).map(|e| format!("{} at {}", e, e.instance_path)).collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    let r = &a.report;
    assert_eq!(r.schema_version, susbot_core::report::SCHEMA_VERSION);
    assert_eq!(r.language, "en");
    assert_eq!(r.source.fetched_via, "proxy");
    assert_eq!(r.source.redirects.len(), 1);
    assert_eq!(r.summary.platform.as_deref(), Some("WordPress"));
    assert!(r.summary.issues.errors > 0 && r.summary.issues.warnings > 0);
    assert!(r.rules.len() > 50);
    assert!(r.rules.iter().find(|x| x.path == "/shop").unwrap().notes.iter().any(|n| n.starts_with("Trailing slash trap")));
    assert!(r.issues.iter().any(|i| i.kind == susbot_core::model::WarningKind::SeoTrap) && r.issues.iter().any(|i| i.kind == susbot_core::model::WarningKind::Sitemap) && r.issues.iter().any(|i| i.kind == susbot_core::model::WarningKind::Fetch));
    assert!(r.issues.iter().all(|i| i.id.contains('.') && i.message != i.id));
    assert_eq!(susbot_core::report::report_host(r).as_deref(), Some("www.example.com"));
    assert_eq!(a.filename(), "www-example-com-robots-audit-2026-09-16");
}

#[test]
fn pasted_and_empty_reports() {
    let pasted = Analysis::new("User-agent: *\nDisallow: /a/\n", Options { now: Some("2026-09-16T10:00:00Z".into()), ..Default::default() }).unwrap();
    assert_eq!(pasted.report.source.fetched_via, "pasted");
    assert_eq!(pasted.filename(), "pasted-robots-audit-2026-09-16");
    let empty = Analysis::new("", Options::default()).unwrap();
    assert!(!empty.report.summary.default_policy.has_star_group);
    assert_eq!(empty.report.summary.default_policy.verdict, Verdict::Open);
    let example = Analysis::new("User-agent: *\nDisallow: /a/\n", Options { site_url: Some(SITE.into()), ..Default::default() }).unwrap();
    assert_eq!(example.report.source.fetched_via, "example");
    // Non-2xx bodies are discarded like crawlers do.
    let mut f = fetch_info();
    f.status = 404;
    let gone = Analysis::new(KITCHEN_SINK, Options { fetch: Some(f), ..Default::default() }).unwrap();
    assert!(gone.report.rules.is_empty() && gone.report.raw.is_empty());
    let mut f = fetch_info();
    f.redirect_limit = true;
    let looped = Analysis::new(KITCHEN_SINK, Options { fetch: Some(f), ..Default::default() }).unwrap();
    assert!(looped.report.issues.iter().any(|i| i.id == "fetch.warn.redirectLimit" && i.level == Level::Error));
}

#[test]
fn csv_tabs_and_tsv() {
    let a = full();
    let tabs = susbot_core::export::csv::tab_rows(&a.report, &a.engine, &a.locale);
    for (name, t) in [("rules", &tabs.rules), ("sitemaps", &tabs.sitemaps), ("issues", &tabs.issues), ("recon", &tabs.recon)] {
        assert!(!t.rows.is_empty(), "{name}");
        for row in &t.rows {
            assert_eq!(row.len(), t.header.len(), "{name} row width");
        }
    }
    assert_eq!(tabs.rules.header, ["User-agents", "Group", "Rule type", "Path", "Line", "Notes"]);
    assert_eq!(tabs.sitemaps.rows[0][0], "robots.txt");
    assert_eq!(tabs.sitemaps.rows[0][3], "HTTP 200");
    assert!(tabs.sitemaps.rows[0][5].contains("301"));
    assert!(tabs.issues.rows.iter().any(|r| r[0] == "note"));
    let cats: Vec<&str> = tabs.recon.rows.iter().map(|r| r[0].as_str()).collect();
    for c in ["Platform", "Cloud", "Host", "API", "Data feed", "Portal", "File type", "Comment", "Security"] {
        assert!(cats.contains(&c), "{c}");
    }
    let csv = a.csv_tabs();
    assert_eq!(csv.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["rules", "sitemaps", "issues", "recon"]);
    assert!(csv[0].1.starts_with("User-agents,Group,Rule type,Path,Line,Notes\r\n"));
    assert!(a.tagged_csv().starts_with("Sheet,Item,Type,Value,Line,Status,Notes\r\n"));
    assert!(a.tsv("issues").unwrap().starts_with("Level\tKind\tLine\tMessage\n"));
    assert!(a.tsv("all").unwrap().starts_with("Sheet\tItem\t"));
    assert!(a.tsv("nope").is_none());
    use susbot_core::export::csv::{csv_cell, to_csv, to_tsv};
    assert_eq!(csv_cell("a,b"), "\"a,b\"");
    assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(to_csv(&["a".into(), "b".into()], &[vec!["1".into(), "c,d".into()]]), "a,b\r\n1,\"c,d\"\r\n");
    assert_eq!(to_tsv(&["a".into(), "b".into()], &[vec!["1".into(), "c\td".into()]]), "a\tb\n1\tc d\n");
}

#[test]
fn markdown_audit_and_actions() {
    let a = full();
    let md = a.markdown();
    for h in ["# robots.txt audit: www.example.com", "## At a glance", "## Recommended actions", "## AI and search crawler access", "## Issues to fix", "## Security exposure", "## What the file reveals", "## Sitemaps"] {
        assert!(md.contains(h), "{h}");
    }
    assert!(md.contains("**Platform detected:** WordPress"));
    assert!(md.contains("| GPTBot | AI training / LLMs | Blocked |"));
    assert!(md.contains("Trailing slash trap"));
    assert!(md.contains("`Disallow: /.env`"));
    assert!(md.contains("Cloud storage and CDNs"));
    assert!(md.contains("1. "));
    let joined = a.recommended_actions().join("\n");
    for s in ["syntax error", "prefix rule", "/robots.txt itself", "never take effect", "https URLs", "Search Console", "Backups & archives", "AI-training policy", "cloud buckets", "non-production hostnames", "Strip names"] {
        assert!(joined.contains(s), "{s}");
    }
    let clean = Analysis::new("User-agent: *\nDisallow: /a/\nSitemap: https://www.example.com/s.xml", Options { site_url: Some(SITE.into()), ..Default::default() }).unwrap();
    assert!(clean.recommended_actions().iter().all(|x| x.starts_with("Decide whether AI")));
    let html = a.html();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>robots.txt audit: www.example.com</title>"));
    assert!(html.contains("<h2>Recommended actions</h2>"));
    use susbot_core::export::markdown::markdown_to_html;
    let h = markdown_to_html("# T <b>\n\nPara with `code` and **bold** and [l](https://x.y) and <https://a.b/c>\n\n- one\n- two\n\n1. first\n2. second\n\n| a | b |\n| --- | --- |\n| 1 | p\\|q |\n");
    for s in ["<h1>T &lt;b&gt;</h1>", "<code>code</code>", "<strong>bold</strong>", "<a href=\"https://x.y\">l</a>", "<a href=\"https://a.b/c\">https://a.b/c</a>", "<ul><li>one</li><li>two</li></ul>", "<ol><li>first</li><li>second</li></ol>", "<th>a</th>", "<td>p|q</td>"] {
        assert!(h.contains(s), "{s}");
    }
}

#[test]
fn ai_status_card() {
    let e = engine();
    let training: Vec<&susbot_core::config::Crawler> = e.config.crawlers.list.iter().filter(|c| c.category == "ai-training").collect();
    let block_all = |names: &[&str]| format!("User-agent: *\nDisallow: /admin/\n{}", names.iter().map(|n| format!("User-agent: {n}\nDisallow: /\n")).collect::<String>());
    let s = ai_status(&model(&block_all(&["GPTBot", "ClaudeBot"])), &e, &en());
    assert_eq!(s.groups.len(), 2);
    assert_eq!(s.groups[0].category, "ai-training");
    assert_eq!(s.groups[0].counts.blocked, 2);
    assert_eq!(s.groups[0].counts.partial, training.len() - 2);
    assert!(s.headline.contains(&format!("AI training / LLMs: 2 of {} blocked", training.len())));
    assert_eq!(s.callout.state, "warning");
    assert!(s.callout.text.contains("CCBot") && !s.callout.text.contains("GPTBot"));
    let all_tokens: Vec<&str> = training.iter().map(|c| c.tokens[0].as_str()).collect();
    assert_eq!(ai_status(&model(&block_all(&all_tokens)), &e, &en()).callout.state, "ok");
    let none = ai_status(&model("User-agent: *\nAllow: /"), &e, &en());
    assert_eq!(none.callout.state, "info");
    assert_eq!(none.groups[0].counts.open, training.len());
    let uniform = ai_status(&model("User-agent: *\nDisallow: /admin/"), &e, &en());
    assert!(uniform.callout.state == "info" && uniform.callout.text.contains("follow the default"));
    assert_eq!(ai_status(&model("User-agent: *\nDisallow: /admin/\nUser-agent: GPTBot\nDisallow: /private/"), &e, &en()).callout.state, "warning");
    let s = ai_status(&model("User-agent: *\nDisallow: /a/\nUser-agent: GPTBot\nDisallow: /"), &e, &en());
    let gpt = s.groups[0].crawlers.iter().find(|c| c.name == "GPTBot").unwrap();
    let cc = s.groups[0].crawlers.iter().find(|c| c.name == "CCBot").unwrap();
    assert!(gpt.explanation.contains("own \"gptbot\" group"));
    assert!(cc.explanation.contains("\"*\" group") && cc.explanation.contains("Common Crawl"));
}

#[test]
fn translated_report_never_falls_back_to_keys() {
    for code in ["de", "fr", "nl", "es", "it"] {
        let json = std::fs::read_to_string(format!("../../locales/{code}.json")).unwrap();
        let a = Analysis::new(KITCHEN_SINK, Options { site_url: Some(SITE.into()), fetch: Some(fetch_info()), lang: Some(code.into()), locale: Some(json), now: Some("2026-09-16T10:00:00Z".into()), ..Default::default() }).unwrap();
        assert_eq!(a.report.language, code);
        let english = Locale::english();
        for i in &a.report.issues {
            assert!(!i.message.contains(&format!("{}", i.id)), "{code}: {} untranslated", i.id);
            assert_ne!(i.message, english.s(&i.id), "{code}: {} still English", i.id);
        }
        let texts: Vec<&String> = a.report.security.iter().map(|f| &f.reason).chain(a.report.recon.cloud.iter().map(|c| &c.risk)).chain(a.report.recon.extensions.iter().map(|e| &e.label)).chain(a.report.crawlers.iter().map(|c| &c.category_label)).collect();
        assert!(texts.len() > 20);
        for t in texts {
            assert!(!t.contains("recon.") && !t.contains("security.") && !t.contains("agents."), "{code}: raw key {t}");
        }
        let md = a.markdown();
        assert!(!md.contains("md."), "{code}: raw md key in audit");
        assert!(a.html().contains(&format!("<html lang=\"{code}\">")));
    }
    let bg = std::fs::read_to_string("../../locales/bg.json").unwrap();
    let a = Analysis::new(KITCHEN_SINK, Options { site_url: Some(SITE.into()), lang: Some("bg".into()), locale: Some(bg), ..Default::default() }).unwrap();
    assert_eq!(a.report.language, "bg");
    assert!(a.report.issues.iter().all(|i| !i.message.starts_with("parser.")));
}
