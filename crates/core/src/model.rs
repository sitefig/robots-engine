//! The parsed robots.txt model and the shared finding shape.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    Info,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Info => "info",
        }
    }
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "error" => Some(Level::Error),
            "warning" => Some(Level::Warning),
            "info" => Some(Level::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleType {
    Allow,
    Disallow,
}

impl RuleType {
    /// The directive name as written in a file.
    pub fn label(self) -> &'static str {
        match self {
            RuleType::Allow => "Allow",
            RuleType::Disallow => "Disallow",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: RuleType,
    pub path: String,
    pub line: u32,
}

impl Rule {
    pub fn label(&self) -> String {
        format!("{}: {}", self.rule_type.label(), self.path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub raw: String,
    pub token: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub agents: Vec<Agent>,
    pub rules: Vec<Rule>,
    #[serde(rename = "crawlDelay")]
    pub crawl_delay: Option<f64>,
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sitemap {
    pub url: String,
    pub line: u32,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostDirective {
    pub value: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanParam {
    pub params: Vec<String>,
    pub path: String,
    pub line: u32,
    pub valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    #[serde(rename = "blank")]
    Blank,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "invalid")]
    Invalid,
    #[serde(rename = "user-agent")]
    UserAgent,
    #[serde(rename = "rule")]
    Rule,
    #[serde(rename = "crawl-delay")]
    CrawlDelay,
    #[serde(rename = "sitemap")]
    Sitemap,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "clean-param")]
    CleanParam,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub n: u32,
    pub raw: String,
    pub kind: LineKind,
    pub field: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningKind {
    #[serde(rename = "syntax")]
    Syntax,
    #[serde(rename = "seo-trap")]
    SeoTrap,
    #[serde(rename = "sitemap")]
    Sitemap,
    #[serde(rename = "fetch")]
    Fetch,
    #[serde(rename = "lint")]
    Lint,
}

impl WarningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WarningKind::Syntax => "syntax",
            WarningKind::SeoTrap => "seo-trap",
            WarningKind::Sitemap => "sitemap",
            WarningKind::Fetch => "fetch",
            WarningKind::Lint => "lint",
        }
    }
}

/// One finding. `id` is the dictionary key of `message` and is stable across
/// languages; exports and tests match on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    pub level: Level,
    pub kind: WarningKind,
    pub id: String,
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unknown {
    pub field: String,
    pub value: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub groups: Vec<Group>,
    pub sitemaps: Vec<Sitemap>,
    pub host: Option<HostDirective>,
    #[serde(rename = "cleanParams")]
    pub clean_params: Vec<CleanParam>,
    pub lines: Vec<Line>,
    pub warnings: Vec<Warning>,
    pub unknown: Vec<Unknown>,
    pub size: usize,
}

/// Longest comment text the recon detectors look at, in characters.
pub const MAX_COMMENT_CHARS: usize = 2000;

/// A rule with its group context, for the recon modules.
pub struct RuleRef<'a> {
    pub rule: &'a Rule,
    pub group: &'a Group,
    pub lower: String,
}

impl Model {
    /// Every non-empty rule with a lowercased path.
    pub fn each_rule(&self) -> Vec<RuleRef<'_>> {
        let mut out = Vec::new();
        for group in &self.groups {
            for rule in &group.rules {
                if rule.path.is_empty() {
                    continue;
                }
                out.push(RuleRef { rule, group, lower: rule.path.to_lowercase() });
            }
        }
        out
    }

    /// Comment text per line, including comments after a directive. Each
    /// comment is capped at [`MAX_COMMENT_CHARS`]: a real comment is one
    /// short line, while a server that answers with an HTML page produces
    /// "comments" of hundreds of kilobytes that the detector regexes cannot
    /// scan in reasonable time.
    pub fn comment_lines(&self) -> Vec<(u32, String)> {
        self.lines
            .iter()
            .filter_map(|l| {
                let i = l.raw.find('#')?;
                let text = l.raw[i + 1..].trim();
                if text.is_empty() {
                    return None;
                }
                let capped: String = text.chars().take(MAX_COMMENT_CHARS).collect();
                Some((l.n, capped))
            })
            .collect()
    }
}

/// Path with wildcards and the end anchor removed, for keyword matching.
pub fn plain_path(path: &str) -> String {
    path.strip_suffix('$').unwrap_or(path).replace('*', "")
}

/// Path segments of a rule path, lowercased, wildcards stripped, query dropped.
pub fn segments(path: &str) -> Vec<String> {
    let p = plain_path(&path.to_lowercase());
    let p = p.split('?').next().unwrap_or("");
    p.split('/').filter(|s| !s.is_empty()).map(String::from).collect()
}

/// True when a path is an absolute http(s) URL.
pub fn is_absolute_url(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// True when a Disallow pattern matches every path ("/", "/*", "/**").
pub fn whole_site(path: &str) -> bool {
    path.starts_with('/') && path[1..].chars().all(|c| c == '*')
}
