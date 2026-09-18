mod common;
use common::*;
use susbot_core::analyser::Verdict;
use susbot_core::diff::{diff_analyses, unified_diff};
use susbot_core::fetch::FetchInfo;
use susbot_core::{Analysis, Options};

fn a(text: &str) -> Analysis {
    Analysis::new(text, Options { site_url: Some(SITE.into()), ..Default::default() }).unwrap()
}

#[test]
fn unchanged_text_is_no_change() {
    let d = diff_analyses(&a("User-agent: *\nDisallow: /a\n"), &a("User-agent: *\nDisallow: /a\n\n"));
    assert!(!d.is_changed && d.bot_changes.is_empty() && d.unified_text_diff.is_empty());
    assert!(diff_analyses(&a(""), &a("")).to_markdown("x").contains("No changes"));
    // A rotating comment is not a change; the raw text still differs.
    let d = diff_analyses(&a("# joke of the day: A\nUser-agent: *\nDisallow: /a\n"), &a("# joke of the day: B\n\nUser-agent: *   # trailing\nDisallow: /a\n"));
    assert!(!d.is_changed && d.text_changed);
    assert!(diff_analyses(&a("User-agent: *\nDisallow: /a\n"), &a("User-agent: *\nDisallow: /b\n")).is_changed);
}

#[test]
fn crawler_flips_and_security_paths() {
    let old = a("User-agent: *\nDisallow: /admin/\n");
    let new = a("User-agent: *\nDisallow: /admin/\nDisallow: /.env\n\nUser-agent: GPTBot\nDisallow: /\n");
    let d = diff_analyses(&old, &new);
    assert!(d.is_changed);
    let gpt = d.bot_changes.iter().find(|b| b.name == "GPTBot").unwrap();
    assert_eq!((gpt.old_verdict, gpt.new_verdict, gpt.category.as_str()), (Verdict::Partial, Verdict::Blocked, "ai-training"));
    assert_eq!(d.bot_changes_in("ai-training").len(), 1);
    assert_eq!(d.new_security_findings.iter().map(|s| (s.path.as_str(), s.severity.as_str())).collect::<Vec<_>>(), [("/.env", "high")]);
    assert!(d.resolved_security_findings.is_empty());
    assert!(d.has_high_impact());
    let md = d.to_markdown("example.com");
    assert!(md.contains("| **GPTBot** | ai-training | Restricted | **Blocked** |"));
    assert!(md.contains("`/.env`") && md.contains("```diff"));
    let social = d.to_social_post("example.com", Some("https://x/commit/1"));
    assert!(social.contains("GPTBot: Restricted → Blocked") && social.contains("https://x/commit/1"));
    // the reverse direction resolves the finding
    let back = diff_analyses(&new, &old);
    assert_eq!(back.resolved_security_findings.len(), 1);
    assert_eq!(back.bot_changes[0].new_verdict, Verdict::Partial);
}

#[test]
fn issues_keyed_by_message_not_line_and_sitemaps() {
    let old = a("User-agent: *\nDisallow: /shop\nSitemap: https://www.example.com/a.xml\n");
    let new = a("# comment moved the lines down\n\nUser-agent: *\nDisallow: /shop\nSitemap: https://www.example.com/b.xml\n");
    let d = diff_analyses(&old, &new);
    assert!(d.is_changed);
    assert!(!d.new_issues.iter().any(|i| i.id == "seo.trailingSlash"), "same trap on another line is not new");
    assert_eq!(d.added_sitemaps, ["https://www.example.com/b.xml"]);
    assert_eq!(d.removed_sitemaps, ["https://www.example.com/a.xml"]);
    assert!(!d.has_high_impact());
}

#[test]
fn fetch_findings_do_not_count_when_both_sides_share_options() {
    let f = FetchInfo { source: "cli".into(), robots_url: "https://example.com/robots.txt".into(), final_url: SITE.into(), status: 200, status_text: None, content_type: None, bytes: 0, truncated: false, redirects: vec![], redirect_limit: false, text: String::new() };
    let text = "User-agent: *\nDisallow: /a\n";
    let with = Analysis::new(text, Options { site_url: Some(SITE.into()), fetch: Some(f), ..Default::default() }).unwrap();
    let without = a(text);
    assert!(with.report.issues.iter().any(|i| i.id.starts_with("fetch.")));
    // Diffing text-identical analyses is never a change, whatever the fetch side added.
    assert!(!diff_analyses(&without, &with).is_changed);
}

#[test]
fn unified_diff_is_minimal_with_context() {
    let d = unified_diff("a\nb\nc\nd\ne\nf\ng\nh\n", "a\nb\nc\nd\nX\nf\ng\nh\n");
    assert_eq!(d, " b\n c\n d\n-e\n+X\n f\n g\n h\n");
    let d = unified_diff("a\nb\n", "a\nb\nc\nd\n");
    assert_eq!(d, " a\n b\n+c\n+d\n");
    let far = unified_diff("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n", "1\nx\n3\n4\n5\n6\n7\n8\n9\n10\n11\ny\n");
    assert!(far.contains("...\n"), "{far}");
}
