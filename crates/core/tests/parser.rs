mod common;
use common::*;
use susbot_core::model::{Level, LineKind, RuleType};
use susbot_core::parser::extract_token;

#[test]
fn token_extraction() {
    assert_eq!(extract_token("Googlebot/2.1"), "googlebot");
    assert_eq!(extract_token("*"), "*");
    assert_eq!(extract_token("* foo"), "*");
    assert_eq!(extract_token("MJ12bot"), "mj12bot");
    assert_eq!(extract_token("/nope"), "");
}

#[test]
fn groups_share_rules_and_a_rule_ends_agent_collection() {
    let m = model("\nUser-agent: a\nUser-agent: b\nDisallow: /x\n\nUser-agent: c\nDisallow: /y\nUser-agent: d\nAllow: /z\n");
    assert_eq!(m.groups.len(), 3);
    assert_eq!(m.groups[0].agents.iter().map(|a| a.token.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(m.groups[0].rules.len(), 1);
    assert_eq!(m.groups[1].agents[0].token, "c");
    assert_eq!(m.groups[2].agents[0].token, "d");
    assert_eq!(m.groups[2].rules[0].rule_type, RuleType::Allow);
}

#[test]
fn comments_blank_lines_and_misspellings() {
    let m = model("# top\nUser-Agent: *   # trailing\nDissallow: /a\n\nSitemap: https://e.com/s.xml");
    assert_eq!(m.groups.len(), 1);
    assert_eq!(m.groups[0].rules[0].rule_type, RuleType::Disallow);
    assert_eq!(m.groups[0].rules[0].path, "/a");
    assert_eq!(m.lines[0].kind, LineKind::Comment);
    assert_eq!(m.lines[3].kind, LineKind::Blank);
    assert_eq!(m.sitemaps[0].url, "https://e.com/s.xml");
    assert!(m.warnings.iter().any(|w| w.line == Some(3) && w.id == "parser.misspelling" && w.message.contains("misspelling")));
}

#[test]
fn rules_before_any_user_agent_are_errors() {
    let m = model("Disallow: /\nUser-agent: *\nAllow: /");
    assert_eq!(m.groups.len(), 1);
    assert_eq!(m.groups[0].rules.len(), 1);
    assert!(m.warnings.iter().any(|w| w.level == Level::Error && w.line == Some(1) && w.id == "parser.ruleBeforeAgent"));
}

#[test]
fn crawl_delay_once_per_group_and_flagged() {
    let m = model("User-agent: *\nCrawl-delay: 5\nCrawl-delay: 10\nDisallow: /a");
    assert_eq!(m.groups[0].crawl_delay, Some(5.0));
    assert!(m.warnings.iter().any(|w| w.line == Some(3) && w.id == "parser.crawlDelayDuplicate"));
    assert!(m.warnings.iter().any(|w| w.id == "parser.crawlDelayNonStandard" && w.message.contains("RFC 9309")));
    assert!(model("User-agent: *\nCrawl-delay: 1").warnings.iter().any(|w| w.message.contains("Google and Yandex ignore it")));
}

#[test]
fn paths_without_leading_slash_bom_unknown_and_invalid() {
    assert!(model("User-agent: *\nDisallow: admin/").warnings.iter().any(|w| w.level == Level::Warning && w.line == Some(2) && w.id == "parser.pathNoSlash"));
    let m = model("\u{feff}User-agent: *\nDisallow: /");
    assert_eq!(m.groups[0].agents[0].token, "*");
    assert!(m.warnings.iter().any(|w| w.id == "parser.bom"));
    let m = model("User-agent: *\nNoindex: /x\nFoo: bar");
    assert_eq!(m.unknown.len(), 2);
    assert!(m.warnings.iter().any(|w| w.line == Some(2) && w.id == "parser.noindex"));
    assert!(m.warnings.iter().any(|w| w.line == Some(3) && w.id == "parser.unknownDirective"));
    let m = model("User-agent: *\nthis is junk\nDisallow: /");
    assert_eq!(m.lines[1].kind, LineKind::Invalid);
    assert_eq!(m.groups[0].rules.len(), 1);
}

#[test]
fn empty_file_star_block_crlf() {
    let m = model("");
    assert!(m.groups.is_empty());
    assert!(m.warnings.iter().any(|w| w.id == "parser.noRules"));
    assert!(model("User-agent: *\nDisallow: /").warnings.iter().any(|w| w.level == Level::Warning && w.id == "parser.starBlocksAll"));
    assert!(!model("User-agent: *\nDisallow: /\nAllow: /public").warnings.iter().any(|w| w.id == "parser.starBlocksAll"));
    assert_eq!(model("User-agent: *\r\nDisallow: /a\r\n").groups[0].rules[0].path, "/a");
}

#[test]
fn host_and_clean_param() {
    let m = model("User-agent: *\nDisallow: /a\nHost: example.com\nHost: other.com");
    assert_eq!(m.host.as_ref().map(|h| (h.value.as_str(), h.line)), Some(("example.com", 3)));
    assert!(m.warnings.iter().any(|w| w.line == Some(3) && w.message.contains("2018")));
    assert!(m.warnings.iter().any(|w| w.line == Some(4) && w.id == "parser.hostDuplicate"));
    assert!(m.unknown.is_empty());
    let m = model("User-agent: *\nDisallow: /a\nClean-param: utm_source&utm_medium /articles/\nClean-param: ref\nClean-param: bad param extra");
    assert_eq!(m.clean_params.len(), 3);
    assert_eq!(m.clean_params[0].params, ["utm_source", "utm_medium"]);
    assert_eq!(m.clean_params[0].path, "/articles/");
    assert!(m.clean_params[0].valid);
    assert_eq!(m.clean_params[1].path, "/");
    assert!(!m.clean_params[2].valid);
    assert!(m.warnings.iter().any(|w| w.line == Some(5) && w.level == Level::Warning && w.id == "parser.cleanParamMalformed"));
}

#[test]
fn markup_and_output_that_does_not_belong_in_a_text_file() {
    let ids = |m: &susbot_core::model::Model| m.warnings.iter().map(|w| w.id.clone()).collect::<Vec<_>>();

    let html = model("User-agent: *\n<p>Disallow: /a</p>\n");
    assert!(ids(&html).contains(&"parser.htmlFragment".to_string()));
    // A whole HTML page is the fetch layer's finding, not the parser's.
    let page = model("<!DOCTYPE html>\n<html><body><p>Not found</p></body></html>\n");
    assert!(!ids(&page).contains(&"parser.htmlFragment".to_string()));

    let php = model("User-agent: *\n<b>Warning</b>: fopen(): failed in /var/www/x.php on line 3\nDisallow: /a\n");
    assert!(ids(&php).contains(&"parser.serverErrorOutput".to_string()));

    let spam = model("User-agent: *\nDisallow: /a\n<script>var _0x3bb1=atob('DkBT');</script>\n");
    assert!(ids(&spam).contains(&"security.injectedCode".to_string()));
    let hidden = model("User-agent: *\n<marquee style='width:0px'><a href=\"https://x.test/\">x</a></marquee>\n");
    assert!(ids(&hidden).contains(&"security.injectedCode".to_string()));

    let cache = model("User-agent: *\nDisallow: /a\n# *** Total size saved: 12%\n");
    assert!(ids(&cache).contains(&"parser.pluginOutput".to_string()));

    // UTF-16 arrives NUL-padded; the file is repaired so the rules still parse.
    let utf16: String = "User-agent: *\nDisallow: /private/\n".chars().flat_map(|c| [c, '\0']).collect();
    let m = model(&utf16);
    assert!(ids(&m).contains(&"parser.utf16".to_string()));
    assert_eq!(m.groups[0].rules[0].path, "/private/");
}

#[test]
fn directives_lost_in_comments_and_values() {
    let glued = model("User-agent: *\nDisallow: /a\n# END YOAST BLOCKDisallow: */cache/\n");
    assert!(glued.warnings.iter().any(|w| w.id == "parser.gluedComment" && w.line == Some(3)));
    // A comment that mentions a directive with a space before it is fine.
    let fine = model("User-agent: *\n# see Disallow: rules below\nDisallow: /a\n");
    assert!(!fine.warnings.iter().any(|w| w.id == "parser.gluedComment"));
    let commented = model("User-agent: *\n# Disallow: /old\nDisallow: /a\n");
    assert!(commented.warnings.iter().any(|w| w.id == "lint.commentedRule"));

    let hash = model("User-agent: *\nDisallow: /page#section\n");
    let w = hash.warnings.iter().find(|w| w.id == "parser.hashInValue").expect("reports the truncation");
    assert!(w.message.contains("/page#section") && w.message.contains("/page"));
    assert_eq!(hash.groups[0].rules[0].path, "/page");
    let spaced = model("User-agent: *\nDisallow: /page # comment\n");
    assert!(!spaced.warnings.iter().any(|w| w.id == "parser.hashInValue"));
}

#[test]
fn field_names_that_need_repair() {
    let invisible = model("\u{200b}User-agent: *\nDisallow: /a\n");
    assert_eq!(invisible.groups.len(), 1, "the zero-width character does not hide the group");
    assert!(invisible.warnings.iter().any(|w| w.id == "parser.invisibleChar"));

    let cyrillic = model("User-agent: *\n\u{0410}llow: /a\n");
    assert!(cyrillic.warnings.iter().any(|w| w.id == "parser.homoglyph"));
    assert_eq!(cyrillic.groups[0].rules.len(), 1);

    let colon = model("User-agent: *\nDisallow\u{ff1a}/a\n");
    assert!(colon.warnings.iter().any(|w| w.id == "parser.fullwidthColon"));
    assert_eq!(colon.groups[0].rules[0].path, "/a");

    let junk = model("User-agent: *\n//Disallow: /a\n");
    assert!(junk.warnings.iter().any(|w| w.id == "parser.junkBeforeField"));
    assert_eq!(junk.groups[0].rules.len(), 1);

    // Edit distance finds typos the fixed alias list never had.
    for typo in ["Disllow: /a", "Diasllow: /a", "Dissalow: /a"] {
        let m = model(&format!("User-agent: *\n{typo}\n"));
        assert_eq!(m.groups[0].rules.len(), 1, "{typo} is read as Disallow");
        assert!(m.warnings.iter().any(|w| w.id == "parser.misspelling"));
    }
    // "ai" is a directive of its own, not a typo of "allow".
    let ai = model("User-agent: *\nAI: https://x.test/policy\n");
    assert!(ai.warnings.iter().any(|w| w.id == "parser.aiDirective"));
    assert!(ai.groups[0].rules.is_empty());
}

#[test]
fn directives_that_are_not_rfc_9309() {
    let cases = [
        ("LLM-Policy: https://x.test/p.json", "parser.aiDirective"),
        ("Content-Signal: search=yes, ai-train=no", "parser.contentSignal"),
        ("Content-Signal: search=maybe", "parser.contentSignalInvalid"),
        ("Request-rate: 1/10s", "parser.nonStandardDirective"),
        ("Noarchive: /", "parser.metaRobots"),
        ("Noindex: /x", "parser.noindex"),
        ("Fnord: /x", "parser.unknownDirective"),
    ];
    for (line, id) in cases {
        let m = model(&format!("User-agent: *\nDisallow: /a\n{line}\n"));
        assert!(m.warnings.iter().any(|w| w.id == id), "{line} should report {id}");
    }
    let bare = model("User-agent: *\nDisallow: /a\nhttps://www.example.com/sitemap.xml\n");
    assert!(bare.warnings.iter().any(|w| w.id == "parser.bareUrl"));
    assert!(bare.sitemaps.is_empty(), "a bare URL is not read as a sitemap");
}

#[test]
fn rule_values_that_do_not_do_what_they_look_like() {
    let cases = [
        ("Disallow: /my folder/", "parser.spaceInPath"),
        ("Disallow: /\\.git", "parser.regexSyntax"),
        ("Disallow: /über-uns/", "parser.nonAsciiPath"),
        ("Disallow: *", "parser.wildcardOnly"),
        ("Sitemap: https://www.yourdomain.com/sitemap.xml", "parser.placeholder"),
    ];
    for (line, id) in cases {
        let m = model(&format!("User-agent: *\n{line}\n"));
        assert!(m.warnings.iter().any(|w| w.id == id), "{line} should report {id}");
    }
    // "/swagger.*.json" is a literal dot plus the wildcard, which is valid.
    let ok = model("User-agent: *\nDisallow: /swagger.*.json\n");
    assert!(!ok.warnings.iter().any(|w| w.id == "parser.regexSyntax"));
    let slow = model("User-agent: *\nCrawl-delay: 120\n");
    assert!(slow.warnings.iter().any(|w| w.id == "parser.crawlDelayHigh"));
    let fine = model("User-agent: *\nCrawl-delay: 5\n");
    assert!(!fine.warnings.iter().any(|w| w.id == "parser.crawlDelayHigh"));
}
