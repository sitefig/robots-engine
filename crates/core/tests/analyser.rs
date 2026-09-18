mod common;
use common::*;
use susbot_core::analyser::*;

fn allowed(text: &str, toks: &[&str], path: &str) -> bool {
    check_access(&model(text), &tokens(toks), path).allowed
}

#[test]
fn path_matching() {
    assert!(path_matches("/foo", "/foo/bar"));
    assert!(!path_matches("/foo", "/fo"));
    assert!(path_matches("/*.php", "/a/b.php?x=1"));
    assert!(path_matches("/*.php$", "/a/b.php"));
    assert!(!path_matches("/*.php$", "/a/b.php?x=1"));
    assert!(path_matches("/*.php$", "/a.php/b.php"));
    assert!(path_matches("/a*b*c", "/aXXbYYc"));
    assert!(path_matches("/$", "/"));
    assert!(!path_matches("/$", "/x"));
    assert!(path_matches("/a.b", "/a.b"));
    assert!(!path_matches("/a.b", "/aXb"));
}

#[test]
fn path_normalisation() {
    assert_eq!(normalise_path(""), "/");
    assert_eq!(normalise_path("foo"), "/foo");
    assert_eq!(normalise_path("/foo?x=1"), "/foo?x=1");
    assert_eq!(normalise_path("https://e.com/a/b?c=1#frag"), "/a/b?c=1");
    assert_eq!(normalise_encoding("/café"), "/caf%C3%A9");
    assert_eq!(normalise_encoding("/caf%c3%a9"), "/caf%C3%A9");
    assert_eq!(normalise_encoding("/%7Euser"), "/~user");
    assert_eq!(normalise_encoding("/a%2Fb"), "/a%2Fb");
    assert_eq!(normalise_encoding("/a b"), "/a%20b");
    assert_eq!(normalise_encoding("/100%"), "/100%");
    assert_eq!(normalise_encoding("/*.php$"), "/*.php$");
}

#[test]
fn precedence() {
    let t = "User-agent: *\nDisallow: /folder\nAllow: /folder/page";
    assert!(allowed(t, &[], "/folder/page"));
    assert!(allowed(t, &[], "/folder/page2"));
    assert!(!allowed(t, &[], "/folder/other"));
    assert!(allowed(t, &[], "/elsewhere"));
    assert!(allowed("User-agent: *\nDisallow: /\nAllow: /", &[], "/anything"));
    assert!(allowed("User-agent: *\nAllow: /\nDisallow: /", &[], "/anything"));
    assert!(allowed("User-agent: *\nDisallow:", &[], "/x"));
    let r = check_access(&model(""), &tokens(&["googlebot"]), "/x");
    assert!(r.allowed && r.token.is_none());
}

#[test]
fn group_selection() {
    let m = model("\nUser-agent: *\nDisallow: /\n\nUser-agent: Googlebot\nDisallow: /g\n\nUser-agent: Googlebot-Image\nDisallow: /img\n");
    assert_eq!(select_groups(&m, &tokens(&["googlebot-image", "googlebot"])).token.as_deref(), Some("googlebot-image"));
    assert_eq!(select_groups(&m, &tokens(&["googlebot-news", "googlebot"])).token.as_deref(), Some("googlebot"));
    assert_eq!(select_groups(&m, &tokens(&["bingbot"])).token.as_deref(), Some("*"));
    assert!(check_access(&m, &tokens(&["googlebot"]), "/").allowed);
    assert!(!check_access(&m, &tokens(&["googlebot"]), "/g/x").allowed);
    assert!(check_access(&m, &tokens(&["googlebot-image", "googlebot"]), "/g/x").allowed);
    assert!(!check_access(&m, &tokens(&["googlebot-image", "googlebot"]), "/img").allowed);
    assert!(!check_access(&m, &tokens(&["bingbot"]), "/").allowed);
    let m = model("User-agent: *\nDisallow: /\n\nUser-agent: GPTBot\nDisallow:");
    assert!(check_access(&m, &tokens(&["gptbot"]), "/").allowed);
    assert!(!check_access(&m, &tokens(&["ccbot"]), "/").allowed);
    let m = model("User-agent: bot\nDisallow: /a\n\nUser-agent: bot\nDisallow: /b");
    assert!(!check_access(&m, &tokens(&["bot"]), "/a").allowed);
    assert!(!check_access(&m, &tokens(&["bot"]), "/b").allowed);
    assert!(check_access(&m, &tokens(&["bot"]), "/c").allowed);
    let m = model("User-agent: GoogleBot/2.1\nDisallow: /x");
    assert!(!check_access(&m, &tokens(&["googlebot"]), "/x").allowed);
    assert!(!check_access(&m, &tokens(&["Googlebot"]), "/x").allowed);
    let m = model("User-agent: *\nCrawl-delay: 10\n\nUser-agent: bingbot\nCrawl-delay: 2\nDisallow: /a");
    assert_eq!(check_access(&m, &tokens(&["bingbot"]), "/").crawl_delay, Some(2.0));
    assert_eq!(check_access(&m, &tokens(&["other"]), "/").crawl_delay, Some(10.0));
}

#[test]
fn summaries() {
    let v = |t: &str| summarise(&model(t), &tokens(&["x"])).verdict;
    assert_eq!(v("User-agent: *\nDisallow: /"), Verdict::Blocked);
    assert_eq!(v("User-agent: *\nDisallow: /*"), Verdict::Blocked);
    assert_eq!(v("User-agent: *\nDisallow: /\nAllow: /pub"), Verdict::Partial);
    assert_eq!(v("User-agent: *\nDisallow: /admin"), Verdict::Partial);
    assert_eq!(v("User-agent: *\nDisallow: /$"), Verdict::Partial);
    assert_eq!(v("User-agent: *\nAllow: /"), Verdict::Open);
    assert_eq!(v(""), Verdict::Open);
    let s = summarise(&model("User-agent: *\nDisallow: /admin\nDisallow: /tmp\nAllow: /tmp/ok"), &tokens(&["x"]));
    assert_eq!((s.disallow_count, s.allow_count, s.root_allowed, s.token.as_deref(), s.specific), (2, 1, true, Some("*"), false));
}

#[test]
fn encoding_and_robots_txt() {
    let t = "User-agent: *\nDisallow: /café\nDisallow: /%7Eprivate\nDisallow: /a%2Fb";
    assert!(!allowed(t, &[], "/caf%C3%A9/x"));
    assert!(!allowed(t, &[], "/café/x"));
    assert!(!allowed(t, &[], "/~private"));
    assert!(!allowed(t, &[], "/%7eprivate"));
    assert!(allowed(t, &[], "/a/b"));
    assert!(!allowed(t, &[], "/a%2Fb"));
    let t = "User-agent: *\nDisallow: /caf%C3%A9\nAllow: /café/";
    assert!(allowed(t, &[], "/café/page"));
    assert!(!allowed(t, &[], "/café"));
    let r = check_access(&model("User-agent: *\nDisallow: /"), &[], "/robots.txt");
    assert!(r.allowed && r.always);
    assert!(!check_access(&model("User-agent: *\nDisallow: /"), &[], "/robots.txt?x").allowed);
    assert!(!check_access(&model("User-agent: *\nDisallow: /a"), &[], "/b").always);
}

#[test]
fn clean_params() {
    let m = model("User-agent: *\nDisallow: /a\nClean-param: utm_source&utm_medium /articles/\nClean-param: ref");
    let c = apply_clean_params(&m, "/articles/x?utm_source=a&id=1&utm_medium=b");
    assert_eq!((c.path.as_str(), c.removed.clone()), ("/articles/x?id=1", tokens(&["utm_source", "utm_medium"])));
    assert_eq!(apply_clean_params(&m, "/other?utm_source=a&ref=b").path, "/other?utm_source=a");
    assert_eq!(apply_clean_params(&m, "/other?ref=b").path, "/other");
    assert_eq!(apply_clean_params(&m, "/other").removed.len(), 0);
    assert_eq!(apply_clean_params(&model(""), "/x?a=1").path, "/x?a=1");
}
