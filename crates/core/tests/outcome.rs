//! A finding about one line opens with what really happens to the page, and
//! that is decided by the whole file: another line can block what a broken
//! line failed to block, or open what a rule closed.

mod common;
use common::*;
use susbot_core::checks::{absolute_rule_check, seo_traps};
use susbot_core::model::Warning;

fn finding(text: &str, id: &str) -> Warning {
    model(text).warnings.into_iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no {id}"))
}

#[test]
fn a_rule_before_any_group_says_what_happens_to_its_path() {
    let w = finding("Disallow: /private/\nUser-agent: *\nDisallow: /tmp/", "parser.ruleBeforeAgent");
    assert_eq!(w.message, "/private/ is open to every crawler. This line was meant to block it, but it comes before any User-agent line, so crawlers ignore it.");
    assert_eq!(w.line, Some(1));
}

#[test]
fn a_later_line_can_block_what_the_ignored_line_did_not() {
    let w = finding("Disallow: /private/\nUser-agent: *\nDisallow: /private/", "parser.ruleBeforeAgent");
    assert!(w.message.starts_with("/private/ is blocked for every crawler, by \"Disallow: /private/\" on line 3."), "{}", w.message);
}

#[test]
fn an_ignored_allow_leaves_the_page_blocked() {
    let w = finding("Allow: /shop/\nUser-agent: *\nDisallow: /", "parser.ruleBeforeAgent");
    assert_eq!(w.message, "/shop/ is blocked for every crawler, by \"Disallow: /\" on line 3. This line was meant to open it, but it comes before any User-agent line, so crawlers ignore it.");
}

#[test]
fn an_allow_that_wins_is_named() {
    let w = finding("Disallow: /a/b\nUser-agent: *\nDisallow: /a\nAllow: /a/b", "parser.ruleBeforeAgent");
    assert!(w.message.starts_with("/a/b is open to every crawler, because \"Allow: /a/b\" on line 4 allows it."), "{}", w.message);
}

#[test]
fn every_crawler_is_only_said_when_no_crawler_has_its_own_rules() {
    let w = finding("Disallow: /private/\nUser-agent: *\nDisallow: /tmp/\nUser-agent: Googlebot\nDisallow: /private/", "parser.ruleBeforeAgent");
    assert!(w.message.starts_with("/private/ is open to every crawler without its own rules in this file."), "{}", w.message);
}

#[test]
fn a_rule_in_a_named_group_is_judged_for_the_crawlers_it_names() {
    let w = finding("User-agent: *\nDisallow: /admin\nUser-agent: Googlebot\nUser-agent: Bingbot\nDisallow: admin", "parser.pathNoSlash");
    assert_eq!(w.message, "/admin is open to Googlebot, Bingbot. This line was meant to block it, but \"admin\" does not start with \"/\", so it matches no URL. Write \"/admin\".");
    let many = finding("User-agent: a\nUser-agent: b\nUser-agent: c\nUser-agent: d\nUser-agent: e\nDisallow: admin", "parser.pathNoSlash");
    assert!(many.message.starts_with("/admin is open to a, b, c and 2 more."), "{}", many.message);
}

#[test]
fn a_rule_without_a_value_has_no_path_to_report() {
    let w = finding("Disallow:\nUser-agent: *\nDisallow: /tmp/", "parser.ruleBeforeAgent");
    assert_eq!(w.message, "Crawlers ignore this Disallow line, because it comes before any User-agent line.");
}

#[test]
fn a_rule_glued_to_a_comment_says_what_happens_to_its_path() {
    let w = finding("User-agent: *\nDisallow: /tmp/ # see docs.Disallow: /cache/\n", "parser.gluedComment");
    assert!(w.message.starts_with("/cache/ is open to every crawler. This line was meant to block it, but a line break is missing"), "{}", w.message);
}

#[test]
fn an_absolute_url_says_what_happens_to_the_path_it_names() {
    let w = absolute_rule_check(&rules(&["Disallow: https://beta.example.com/checkout/", "Disallow: /checkout/"]), &en());
    assert!(w[0].message.starts_with("/checkout/ is blocked for every crawler, by \"Disallow: /checkout/\" on line 3."), "{}", w[0].message);
    let w = absolute_rule_check(&rules(&["Disallow: https://beta.example.com/checkout/"]), &en());
    assert!(w[0].message.starts_with("/checkout/ is open to every crawler. This line was meant to block it"), "{}", w[0].message);
}

#[test]
fn a_consequence_is_only_reported_when_it_is_real() {
    let ids = |lines: &[&str]| seo_traps(&rules(lines), &en()).into_iter().map(|w| w.id).collect::<Vec<_>>();
    assert!(ids(&["Disallow: /*.css$"]).contains(&"seo.cssJsBlock".to_string()));
    // Allow wins a tie, so the stylesheets are open and nothing is blocked.
    assert!(!ids(&["Disallow: /*.css$", "Allow: /*.css$"]).contains(&"seo.cssJsBlock".to_string()));
    assert!(ids(&["Disallow: /*?"]).contains(&"seo.queryStringBlock".to_string()));
    assert!(!ids(&["Disallow: /*?", "Allow: /*?page="]).contains(&"seo.queryStringBlock".to_string()));
    assert!(ids(&["Disallow: /images/"]).contains(&"seo.imageBlock".to_string()));
    assert!(!ids(&["Disallow: /images/", "Allow: /images/*.jpg"]).contains(&"seo.imageBlock".to_string()));
}

#[test]
fn an_overridden_rule_says_what_stays_open_or_blocked() {
    let w = seo_traps(&rules(&["Allow: /x", "Disallow: /x"]), &en());
    let over = w.iter().find(|w| w.id == "seo.overridden").unwrap();
    assert_eq!(over.message, "/x stays open, because \"Allow: /x\" on line 2 wins over \"Disallow: /x\" on every URL it matches.");
    // The Disallow lost, so it blocks none of the neighbours the trap would name.
    assert_eq!(w.iter().filter(|w| w.id == "seo.trailingSlash").map(|w| w.line.unwrap()).collect::<Vec<_>>(), [2]);
    let w = seo_traps(&rules(&["Allow: /a", "Disallow: /*a"]), &en());
    assert!(w.iter().any(|w| w.id == "seo.overridden" && w.message.starts_with("/a stays blocked, because \"Disallow: /*a\" on line 3 wins")), "{:?}", w.iter().map(|w| &w.message).collect::<Vec<_>>());
}

#[test]
fn a_user_agent_line_without_a_name_says_who_the_rules_still_apply_to() {
    let alone = finding("User-agent:\nDisallow: /a/", "parser.emptyUserAgent");
    assert_eq!(alone.message, "The rules under this line apply to no crawler, because the User-agent line names none.");
    let with_others = finding("User-agent: Googlebot\nUser-agent:\nDisallow: /a/", "parser.emptyUserAgent");
    assert_eq!(with_others.message, "This line adds no crawler to the group, because the User-agent value is empty. The rules under it apply to Googlebot.");
    let with_star = finding("User-agent: /bad\nUser-agent: *\nDisallow: /a/", "parser.invalidToken");
    assert!(with_star.message.ends_with("The rules under it apply to every crawler without its own rules."), "{}", with_star.message);
}

#[test]
fn an_empty_group_only_opens_the_site_when_no_other_group_binds_the_crawler() {
    let open = finding("User-agent: *\nDisallow: /a/\nUser-agent: Googlebot\n", "parser.emptyGroup");
    assert_eq!(open.message, "The crawlers this group names may read every page, because the group has no rules.");
    let bound = finding("User-agent: Googlebot\nDisallow: /a/\nUser-agent: Bingbot\nDisallow: /b/\nUser-agent: Googlebot\n", "parser.emptyGroup");
    assert_eq!(bound.message, "This group adds no rules. The crawlers it names follow the rules of their other groups in this file.");
}

#[test]
fn a_rule_cut_short_by_a_hash_says_what_happens_to_the_shorter_path() {
    let w = finding("User-agent: *\nDisallow: /page#section", "parser.hashInValue");
    assert!(w.message.starts_with("/page is blocked for every crawler, by \"Disallow: /page\" on line 2. This line was written as \"/page#section\""), "{}", w.message);
}
