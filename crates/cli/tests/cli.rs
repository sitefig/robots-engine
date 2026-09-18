//! End-to-end runs of the built binary; nothing here touches the network.
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_susbot"))
}

fn tmp(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("susbot-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

const KITCHEN: &str = "../../examples/kitchen-sink.robots.txt";

#[test]
fn audit_with_and_without_the_subcommand() {
    let out = bin().args(["audit", KITCHEN, "--site-url", "https://www.example.com/robots.txt", "--format", "json"]).output().unwrap();
    assert!(out.status.success());
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["summary"]["platform"], "WordPress");
    let legacy = bin().args([KITCHEN, "--fail-on", "error"]).output().unwrap();
    assert_eq!(legacy.status.code(), Some(1), "legacy form still audits and honours --fail-on");
    assert!(String::from_utf8_lossy(&legacy.stdout).contains("Recommended actions"));
    let cfg = bin().arg("--print-default-config").output().unwrap();
    assert!(cfg.status.success() && String::from_utf8_lossy(&cfg.stdout).contains("[crawlers]"));
}

#[test]
fn diff_formats_and_gate() {
    let old = tmp("old.txt", "User-agent: *\nDisallow: /admin/\n");
    let new = tmp("new.txt", "User-agent: *\nDisallow: /admin/\nUser-agent: GPTBot\nDisallow: /\n");
    let md = bin().args(["diff", old.to_str().unwrap(), new.to_str().unwrap(), "--domain", "example.com"]).output().unwrap();
    assert!(md.status.success());
    let text = String::from_utf8_lossy(&md.stdout);
    assert!(text.contains("**GPTBot**") && text.contains("**Blocked**"));
    let json = bin().args(["diff", old.to_str().unwrap(), new.to_str().unwrap(), "--format", "json"]).output().unwrap();
    let d: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(d["bot_changes"][0]["name"], "GPTBot");
    let social = bin().args(["diff", old.to_str().unwrap(), new.to_str().unwrap(), "--format", "social", "--diff-url", "https://x/y"]).output().unwrap();
    assert!(String::from_utf8_lossy(&social.stdout).contains("https://x/y"));
    let gate = bin().args(["diff", old.to_str().unwrap(), new.to_str().unwrap(), "--fail-on-change"]).output().unwrap();
    assert_eq!(gate.status.code(), Some(1));
    let same = bin().args(["diff", old.to_str().unwrap(), old.to_str().unwrap(), "--fail-on-change"]).output().unwrap();
    assert!(same.status.success());
}

#[test]
fn track_with_nothing_enabled_writes_a_summary() {
    let cfg = tmp("sites.json", r#"[{"domain":"example.invalid","enabled":false}]"#);
    let dir = std::env::temp_dir().join(format!("susbot-track-{}", std::process::id()));
    let summary = dir.join("summary.md");
    let out = bin().args(["track", "--config", cfg.to_str().unwrap(), "--data-dir", dir.join("data").to_str().unwrap(), "--summary-out", summary.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let s = std::fs::read_to_string(&summary).unwrap();
    assert!(s.contains("- checked: 0"));
}

#[test]
fn crawl_records_unreachable_domains_and_writes_a_summary() {
    let list = tmp("domains.csv", "1,nonexistent-host.invalid\n2,another.invalid\n# comment\n");
    let dir = std::env::temp_dir().join(format!("susbot-crawl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out_file = dir.join("out.jsonl.gz");
    let summary = dir.join("summary.json");
    let out = bin().args(["crawl", "--input", list.to_str().unwrap(), "--out", out_file.to_str().unwrap(), "--summary", summary.to_str().unwrap(), "--timeout", "2", "--concurrency", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&summary).unwrap()).unwrap();
    assert_eq!(s["domains"], 2);
    assert_eq!(s["unreachable"], 2);
    let bytes = std::fs::read(&out_file).unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b], "gzip magic");
}
