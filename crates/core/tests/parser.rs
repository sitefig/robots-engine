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
