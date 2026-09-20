mod common;
use common::*;
use susbot_core::security::*;

fn scan(paths: &[&str]) -> Vec<Finding> {
    let lines: Vec<String> = paths.iter().map(|p| format!("Disallow: {p}")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    find_sensitive_paths(&rules(&refs), &engine(), &en(), None)
}

#[test]
fn categories_and_severities() {
    let f = scan(&["/admin", "/wp-admin/", "/cpanel/", "/phpmyadmin", "/Administrator/", "/admin*"]);
    assert_eq!(f.len(), 6);
    assert!(f.iter().all(|x| x.category == "admin" && x.severity == "medium"));
    let f = scan(&["/staging/", "/dev/", "/beta/", "/v2/", "/uat"]);
    assert_eq!(f.len(), 5);
    assert_eq!(f.iter().find(|x| x.path == "/v2/").unwrap().severity, "low");
    assert_eq!(f.iter().find(|x| x.path == "/staging/").unwrap().severity, "medium");
    assert!(f.iter().all(|x| x.category == "staging"));
    let f = scan(&["/backup/", "/*.sql", "/.env", "/.git/", "/db.zip", "/wp-config.php", "/secrets/", "/*.sql$"]);
    assert_eq!(f.len(), 8);
    assert!(f.iter().all(|x| x.severity == "high"));
    assert_eq!(f.iter().find(|x| x.path == "/*.sql").unwrap().category, "backups");
    assert_eq!(f.iter().find(|x| x.path == "/.env").unwrap().category, "secrets");
    assert_eq!(f.iter().find(|x| x.path == "/.git/").unwrap().category, "secrets");
    let f = scan(&["/api/internal/", "/actuator", "/graphql", "/phpinfo.php", "/debug/"]);
    assert_eq!(f.len(), 5);
    assert!(f.iter().all(|x| x.category == "api"));
    let f = scan(&["/uploads/", "/invoices/", "/private/"]);
    assert_eq!(f.len(), 3);
    assert!(f.iter().all(|x| x.category == "data" && x.severity == "low"));
}

#[test]
fn benign_ordering_and_highest_wins() {
    assert!(scan(&["/search", "/tag/", "/cart", "/checkout", "/feed", "/blog/", "/"]).is_empty());
    assert!(scan(&["/*"]).is_empty());
    assert!(find_sensitive_paths(&model("User-agent: *\nAllow: /admin"), &engine(), &en(), None).is_empty());
    assert!(scan(&["/administration-guide", "/adminlte-demo"]).is_empty());
    let f = find_sensitive_paths(&model("User-agent: a\nUser-agent: b\nDisallow: /dev/\nDisallow: /.env\n\nUser-agent: *\nDisallow: /uploads/"), &engine(), &en(), None);
    assert_eq!(f.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(), ["/.env", "/dev/", "/uploads/"]);
    assert_eq!(f[0].agents, ["a", "b"]);
    assert_eq!(f[0].line, 4);
    assert_eq!(f[2].agents, ["*"]);
    let f = scan(&["/admin/backup.sql"]);
    assert_eq!((f.len(), f[0].severity.as_str(), f[0].category.as_str()), (1, "high", "backups"));
    assert_eq!(f[0].reason, "Backup or database dump location");
    let e = engine();
    assert_eq!(categories_in(&scan(&["/uploads/", "/.env", "/admin"]), &e), ["admin", "secrets", "data"]);
    for c in &e.config.security.categories {
        let (label, advice) = category_info(&e, &en(), &c.id);
        assert!(!label.is_empty() && !advice.is_empty() && !label.contains('.'));
    }
}

#[test]
fn configurable_signatures_and_ignores() {
    let e = susbot_core::Engine::from_toml(Some("[security]\nignore = ['^/dev/']\n[[security.categories]]\nid = \"custom\"\nlabel = \"Custom area\"\nadvice = \"Lock it down.\"\n[[security.signatures]]\ncategory = \"custom\"\nseverity = \"high\"\nreason = \"Our thing\"\nkeywords = [\"treasure\"]\n")).unwrap();
    let f = find_sensitive_paths(&rules(&["Disallow: /treasure/", "Disallow: /dev/", "Disallow: /admin/"]), &e, &en(), None);
    assert_eq!(f.iter().map(|x| (x.path.as_str(), x.category.as_str(), x.reason.as_str())).collect::<Vec<_>>(), [("/treasure/", "custom", "Our thing")]);
    assert_eq!(category_info(&e, &en(), "custom"), ("Custom area".to_string(), "Lock it down.".to_string()));
    let off = susbot_core::Engine::from_toml(Some("[checks]\nsecurity = false\n")).unwrap();
    assert!(find_sensitive_paths(&rules(&["Disallow: /.env"]), &off, &en(), None).is_empty());
}

#[test]
fn a_platforms_own_paths_are_findings_but_not_alarms() {
    let e = engine();
    let m = model("User-agent: *\nDisallow: /wp-admin/\nDisallow: /backup/\n");
    let plain = find_sensitive_paths(&m, &e, &en(), None);
    assert_eq!(plain.iter().find(|f| f.path == "/wp-admin/").unwrap().severity, "medium");

    let on_wordpress = find_sensitive_paths(&m, &e, &en(), Some("WordPress"));
    let wp = on_wordpress.iter().find(|f| f.path == "/wp-admin/").unwrap();
    assert_eq!(wp.severity, "info", "every WordPress site publishes this path");
    assert!(wp.reason.contains("WordPress"));
    // A path that is not part of the platform keeps its severity.
    assert_eq!(on_wordpress.iter().find(|f| f.path == "/backup/").unwrap().severity, "high");
    // The lowest severity sorts last.
    assert_eq!(on_wordpress[0].path, "/backup/");
}
