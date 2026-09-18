//! Semantic diff between two robots.txt analyses: crawler verdict flips,
//! security findings that appeared or went away, issues that appeared or went
//! away, sitemap changes, plus a unified line diff of the text. Both sides
//! must be analysed with the same options (no fetch info on either), or fetch
//! findings show up as changes.

use crate::analyser::Verdict;
use crate::analysis::Analysis;
use crate::report::Report;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotDiff {
    pub name: String,
    pub category: String,
    pub old_verdict: Verdict,
    pub new_verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityDiff {
    pub path: String,
    pub category: String,
    pub severity: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueDiff {
    pub id: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDiff {
    /// The directives differ. Comment-only edits (some sites rotate a joke in
    /// the header) and blank lines do not count; `text_changed` does.
    pub is_changed: bool,
    /// The raw text differs, comments included.
    pub text_changed: bool,
    pub bot_changes: Vec<BotDiff>,
    pub new_security_findings: Vec<SecurityDiff>,
    pub resolved_security_findings: Vec<SecurityDiff>,
    pub new_issues: Vec<IssueDiff>,
    pub resolved_issues: Vec<IssueDiff>,
    pub added_sitemaps: Vec<String>,
    pub removed_sitemaps: Vec<String>,
    pub old_rules_count: usize,
    pub new_rules_count: usize,
    pub unified_text_diff: String,
}

fn verdict_word(v: Verdict) -> &'static str {
    match v {
        Verdict::Open => "Open",
        Verdict::Partial => "Restricted",
        Verdict::Blocked => "Blocked",
    }
}

impl SemanticDiff {
    /// A crawler verdict flipped, a high-severity path appeared, or a new error was introduced.
    pub fn has_high_impact(&self) -> bool {
        !self.bot_changes.is_empty() || self.new_security_findings.iter().any(|s| s.severity == "high") || self.new_issues.iter().any(|i| i.level == "error")
    }

    /// Crawler flips in a category, e.g. "ai-training".
    pub fn bot_changes_in(&self, category: &str) -> Vec<&BotDiff> {
        self.bot_changes.iter().filter(|b| b.category == category).collect()
    }

    /// Short, ready-to-share text (social media, Slack, Discord).
    pub fn to_social_post(&self, domain: &str, diff_url: Option<&str>) -> String {
        let mut out = format!("robots.txt change at {domain}\n\n");
        if !self.bot_changes.is_empty() {
            for b in &self.bot_changes {
                let icon = match b.new_verdict {
                    Verdict::Blocked => "\u{1F6D1}",
                    Verdict::Open => "\u{2705}",
                    Verdict::Partial => "\u{26A0}\u{FE0F}",
                };
                out.push_str(&format!("{icon} {}: {} \u{2192} {}\n", b.name, verdict_word(b.old_verdict), verdict_word(b.new_verdict)));
            }
            out.push('\n');
        }
        if !self.new_security_findings.is_empty() {
            out.push_str(&format!("\u{26A0}\u{FE0F} {} newly listed sensitive path(s)\n\n", self.new_security_findings.len()));
        }
        if self.bot_changes.is_empty() && self.new_security_findings.is_empty() {
            out.push_str(&format!("{} rules before, {} after; no crawler verdict changed.\n\n", self.old_rules_count, self.new_rules_count));
        }
        if let Some(url) = diff_url {
            out.push_str(&format!("Diff: {url}\n"));
        }
        out.push_str("Audited by sus.bot\n");
        out
    }

    /// Markdown for a job summary, a PR comment or a report.
    pub fn to_markdown(&self, domain: &str) -> String {
        let mut out = format!("### robots.txt changes for `{domain}`\n\n");
        if !self.is_changed {
            out.push_str("No changes.\n");
            return out;
        }
        out.push_str(&format!("{} rules before, {} after.\n\n", self.old_rules_count, self.new_rules_count));
        if !self.bot_changes.is_empty() {
            out.push_str("#### Crawler access changes\n\n| Crawler | Category | Before | After |\n| --- | --- | --- | --- |\n");
            for b in &self.bot_changes {
                out.push_str(&format!("| **{}** | {} | {} | **{}** |\n", b.name, b.category, verdict_word(b.old_verdict), verdict_word(b.new_verdict)));
            }
            out.push('\n');
        }
        if !self.new_security_findings.is_empty() {
            out.push_str("#### Newly listed sensitive paths\n\n| Path | Category | Severity | Why |\n| --- | --- | --- | --- |\n");
            for s in &self.new_security_findings {
                out.push_str(&format!("| `{}` | {} | {} | {} |\n", s.path.replace('|', "\\|"), s.category, s.severity, s.reason));
            }
            out.push('\n');
        }
        if !self.resolved_security_findings.is_empty() {
            out.push_str(&format!("#### No longer listed\n\n{}\n\n", self.resolved_security_findings.iter().map(|s| format!("- `{}` ({})", s.path, s.category)).collect::<Vec<_>>().join("\n")));
        }
        if !self.new_issues.is_empty() {
            out.push_str("#### New issues\n\n");
            for i in &self.new_issues {
                out.push_str(&format!("- **{}** {}\n", i.level, i.message));
            }
            out.push('\n');
        }
        if !self.resolved_issues.is_empty() {
            out.push_str("#### Resolved issues\n\n");
            for i in &self.resolved_issues {
                out.push_str(&format!("- ~~{}~~\n", i.message));
            }
            out.push('\n');
        }
        if !self.added_sitemaps.is_empty() || !self.removed_sitemaps.is_empty() {
            out.push_str("#### Sitemaps\n\n");
            for s in &self.added_sitemaps {
                out.push_str(&format!("- added <{s}>\n"));
            }
            for s in &self.removed_sitemaps {
                out.push_str(&format!("- removed <{s}>\n"));
            }
            out.push('\n');
        }
        if !self.unified_text_diff.is_empty() {
            out.push_str("<details><summary>Text diff</summary>\n\n```diff\n");
            out.push_str(&self.unified_text_diff);
            out.push_str("```\n\n</details>\n\n");
        }
        out
    }
}

/// Compare two analyses built with the same options.
pub fn diff_analyses(old: &Analysis, new: &Analysis) -> SemanticDiff {
    diff_reports(&old.report, &old.text, &new.report, &new.text)
}

/// The lines that carry meaning: comments stripped, whitespace trimmed, blanks dropped.
pub fn directive_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn diff_reports(old_rep: &Report, old_text: &str, new_rep: &Report, new_text: &str) -> SemanticDiff {
    let text_changed = old_text.trim() != new_text.trim();
    let is_changed = directive_lines(old_text) != directive_lines(new_text);
    let mut bot_changes = Vec::new();
    for new_c in &new_rep.crawlers {
        if let Some(old_c) = old_rep.crawlers.iter().find(|c| c.name == new_c.name) {
            if old_c.verdict != new_c.verdict {
                bot_changes.push(BotDiff { name: new_c.name.clone(), category: new_c.category.clone(), old_verdict: old_c.verdict, new_verdict: new_c.verdict });
            }
        }
    }
    let sec = |f: &crate::security::Finding| SecurityDiff { path: f.path.clone(), category: f.category.clone(), severity: f.severity.clone(), reason: f.reason.clone() };
    let same_sec = |a: &crate::security::Finding, b: &crate::security::Finding| a.path == b.path && a.category == b.category;
    let new_security_findings = new_rep.security.iter().filter(|nf| !old_rep.security.iter().any(|of| same_sec(of, nf))).map(sec).collect();
    let resolved_security_findings = old_rep.security.iter().filter(|of| !new_rep.security.iter().any(|nf| same_sec(of, nf))).map(sec).collect();
    // Issues are keyed by id and message (not line), so a line shift is not a change.
    let iss = |w: &crate::model::Warning| IssueDiff { id: w.id.clone(), level: w.level.as_str().to_string(), message: w.message.clone() };
    let same_iss = |a: &crate::model::Warning, b: &crate::model::Warning| a.id == b.id && a.message == b.message;
    let new_issues = new_rep.issues.iter().filter(|ni| !old_rep.issues.iter().any(|oi| same_iss(oi, ni))).map(iss).collect();
    let resolved_issues = old_rep.issues.iter().filter(|oi| !new_rep.issues.iter().any(|ni| same_iss(oi, ni))).map(iss).collect();
    let old_sitemaps: Vec<&str> = old_rep.sitemaps.iter().map(|s| s.url.as_str()).collect();
    let new_sitemaps: Vec<&str> = new_rep.sitemaps.iter().map(|s| s.url.as_str()).collect();
    SemanticDiff {
        is_changed,
        text_changed,
        bot_changes,
        new_security_findings,
        resolved_security_findings,
        new_issues,
        resolved_issues,
        added_sitemaps: new_sitemaps.iter().filter(|s| !old_sitemaps.contains(s)).map(|s| s.to_string()).collect(),
        removed_sitemaps: old_sitemaps.iter().filter(|s| !new_sitemaps.contains(s)).map(|s| s.to_string()).collect(),
        old_rules_count: old_rep.summary.rules,
        new_rules_count: new_rep.summary.rules,
        unified_text_diff: if text_changed { unified_diff(old_text, new_text) } else { String::new() },
    }
}

/// Unified line diff (LCS-based, no hunk headers, three lines of context).
pub fn unified_diff(old_text: &str, new_text: &str) -> String {
    let a: Vec<&str> = old_text.lines().collect();
    let b: Vec<&str> = new_text.lines().collect();
    let (n, m) = (a.len(), b.len());
    // LCS table (files are small; robots.txt is capped at 512 KiB, typically a few hundred lines).
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Op {
        Keep,
        Del,
        Add,
    }
    let mut ops: Vec<(Op, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push((Op::Keep, a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            ops.push((Op::Del, a[i]));
            i += 1;
        } else {
            ops.push((Op::Add, b[j]));
            j += 1;
        }
    }
    while i < n {
        ops.push((Op::Del, a[i]));
        i += 1;
    }
    while j < m {
        ops.push((Op::Add, b[j]));
        j += 1;
    }
    // Keep only changed lines plus three lines of context around them.
    const CONTEXT: usize = 3;
    let changed: Vec<bool> = ops.iter().map(|(op, _)| *op != Op::Keep).collect();
    let mut show = vec![false; ops.len()];
    for (k, c) in changed.iter().enumerate() {
        if *c {
            for s in k.saturating_sub(CONTEXT)..(k + CONTEXT + 1).min(ops.len()) {
                show[s] = true;
            }
        }
    }
    let mut out = String::new();
    let mut last_shown = None;
    for (k, (op, line)) in ops.iter().enumerate() {
        if !show[k] {
            continue;
        }
        if let Some(prev) = last_shown {
            if k > prev + 1 {
                out.push_str("...\n");
            }
        }
        last_shown = Some(k);
        let prefix = match op {
            Op::Keep => ' ',
            Op::Del => '-',
            Op::Add => '+',
        };
        out.push(prefix);
        out.push_str(line);
        out.push('\n');
    }
    out
}
