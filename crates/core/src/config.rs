//! Rules configuration. `config/default.toml` is embedded and always applied;
//! a user TOML is deep-merged on top (tables and scalars override, arrays
//! replace). [`Engine`] is the compiled form: every regex parsed once, with
//! errors that name the offending entry.

use crate::model::Level;
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_TOML: &str = include_str!("../data/default.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub vendor: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RulesCfg {
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub levels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconChecks {
    pub cms: bool,
    pub cloud: bool,
    pub hosts: bool,
    pub api: bool,
    pub data: bool,
    pub extensions: bool,
    pub comments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checks {
    pub seo_traps: bool,
    pub sitemaps: bool,
    pub absolute_urls: bool,
    pub security: bool,
    pub recon: ReconChecks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crawler {
    pub name: String,
    pub tokens: Vec<String>,
    pub category: String,
    #[serde(default)]
    pub ua: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crawlers {
    pub categories: Vec<String>,
    pub ai_categories: Vec<String>,
    pub browser_ua: String,
    pub list: Vec<Crawler>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCategory {
    pub id: String,
    pub label: String,
    pub advice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySignature {
    pub category: String,
    pub severity: String,
    pub reason: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCfg {
    #[serde(default)]
    pub ignore: Vec<String>,
    pub categories: Vec<SecurityCategory>,
    pub signatures: Vec<SecuritySignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmsSignature {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub strong: Vec<String>,
    #[serde(default)]
    pub weak: Vec<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmsCfg {
    #[serde(default)]
    pub disabled: Vec<String>,
    pub platform_kinds: Vec<String>,
    pub signatures: Vec<CmsSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProvider {
    pub provider: String,
    pub kind: String,
    pub host: String,
    #[serde(default = "none_str")]
    pub bucket: String,
}

fn none_str() -> String {
    "none".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCfg {
    pub providers: Vec<CloudProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsCfg {
    pub env_labels: String,
    pub env_label_fallback: String,
    pub env_paths: String,
    pub env_path_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSignature {
    pub kind: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCfg {
    pub signatures: Vec<ApiSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelledPattern {
    pub pattern: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCfg {
    pub feed_words: String,
    pub feed_file: String,
    pub feed_hint: String,
    pub feed_file_wildcard: String,
    pub portal_words: String,
    pub search_segments: String,
    pub search_keys: Vec<String>,
    pub filter_keys: Vec<String>,
    pub tracking_keys: String,
    pub feed_kinds: Vec<LabelledPattern>,
    pub feed_fallback: String,
    pub portal_kinds: Vec<LabelledPattern>,
    pub portal_fallback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionGroup {
    pub exts: Vec<String>,
    pub category: String,
    pub label: String,
    pub risk: String,
    #[serde(default)]
    pub tech: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionsCfg {
    pub groups: Vec<ExtensionGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentsCfg {
    pub email: String,
    pub person: String,
    pub person_deny: String,
    pub jira: String,
    pub jira_deny: String,
    pub ticket_word: String,
    pub iso_date: String,
    pub slash_date: String,
    pub text_date: String,
    pub ip: String,
    pub vendors: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconCfg {
    pub cms: CmsCfg,
    pub cloud: CloudCfg,
    pub hosts: HostsCfg,
    pub api: ApiCfg,
    pub data: DataCfg,
    pub extensions: ExtensionsCfg,
    pub comments: CommentsCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub tool: Tool,
    #[serde(default)]
    pub rules: RulesCfg,
    pub checks: Checks,
    pub crawlers: Crawlers,
    pub security: SecurityCfg,
    pub recon: ReconCfg,
}

impl Config {
    /// The embedded defaults.
    pub fn default_config() -> Config {
        Config::merged(None).expect("embedded default.toml is valid")
    }

    /// Defaults with an optional user TOML merged on top.
    pub fn merged(user_toml: Option<&str>) -> Result<Config, String> {
        let mut base: serde_json::Value = basic_toml::from_str(DEFAULT_TOML).map_err(|e| format!("default config: {e}"))?;
        if let Some(text) = user_toml {
            if !text.trim().is_empty() {
                let user: serde_json::Value = basic_toml::from_str(text).map_err(|e| format!("config: {e}"))?;
                deep_merge(&mut base, user);
            }
        }
        serde_json::from_value(base).map_err(|e| format!("config: {e}"))
    }
}

/// Tables merge recursively; everything else (including arrays) is replaced.
fn deep_merge(base: &mut serde_json::Value, over: serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(existing) if existing.is_object() && v.is_object() => deep_merge(existing, v),
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

// ---------------------------------------------------------------- compiled form

fn compile(pattern: &str, what: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("{what}: invalid regex `{pattern}`: {e}"))
}

pub struct CompiledSecurity {
    pub category: String,
    pub severity: String,
    pub reason: String,
    pub regexes: Vec<Regex>,
}

pub struct CompiledCms {
    pub name: String,
    pub kind: String,
    pub strong: Vec<Regex>,
    pub weak: Vec<Regex>,
    pub comments: Vec<Regex>,
}

/// One way to extract a bucket name; see default.toml for the syntax.
#[derive(Debug, Clone)]
pub enum BucketPart {
    HostGroup(usize),
    Segment(usize),
    Path(Regex),
}

pub struct CompiledProvider {
    pub provider: String,
    pub kind: String,
    pub host: Regex,
    /// Alternatives, each a list of parts joined with " / ".
    pub bucket: Vec<Vec<BucketPart>>,
}

pub struct CompiledApi {
    pub kind: String,
    pub pattern: Regex,
}

pub struct CompiledLabelled {
    pub pattern: Regex,
    pub label: String,
}

pub struct CompiledData {
    pub feed_words: Regex,
    pub feed_file: Regex,
    pub feed_hint: Regex,
    pub feed_file_wildcard: Regex,
    pub portal_words: Regex,
    pub search_segments: Regex,
    pub search_keys: HashSet<String>,
    pub filter_keys: HashSet<String>,
    pub tracking_keys: Regex,
    pub feed_kinds: Vec<CompiledLabelled>,
    pub portal_kinds: Vec<CompiledLabelled>,
}

pub struct CompiledHosts {
    pub env_labels: Regex,
    pub env_label_fallback: Regex,
    pub env_paths: Regex,
    pub env_path_hint: Regex,
}

pub struct CompiledComments {
    pub email: Regex,
    pub person: Regex,
    pub person_deny: Regex,
    pub jira: Regex,
    pub jira_deny: Regex,
    pub ticket_word: Regex,
    pub iso_date: Regex,
    pub slash_date: Regex,
    pub text_date: Regex,
    pub ip: Regex,
    pub vendors: Regex,
    pub notes: Regex,
}

#[derive(Debug, Clone)]
pub struct ExtDef {
    pub category: String,
    pub label: String,
    pub risk: String,
    pub tech: Option<String>,
}

/// A configuration with every pattern compiled.
pub struct Engine {
    pub config: Config,
    pub security: Vec<CompiledSecurity>,
    pub security_ignore: Vec<Regex>,
    pub cms: Vec<CompiledCms>,
    pub cloud: Vec<CompiledProvider>,
    pub hosts: CompiledHosts,
    pub api: Vec<CompiledApi>,
    pub data: CompiledData,
    pub extensions: HashMap<String, ExtDef>,
    pub comments: CompiledComments,
    pub disabled_ids: HashSet<String>,
    pub level_overrides: HashMap<String, Level>,
}

impl Engine {
    pub fn default_engine() -> Engine {
        Engine::from_config(Config::default_config()).expect("default config compiles")
    }

    pub fn from_toml(user_toml: Option<&str>) -> Result<Engine, String> {
        Engine::from_config(Config::merged(user_toml)?)
    }

    pub fn from_config(config: Config) -> Result<Engine, String> {
        let mut security = Vec::new();
        for (i, s) in config.security.signatures.iter().enumerate() {
            let what = format!("security.signatures[{i}]");
            let mut regexes = Vec::new();
            if !s.keywords.is_empty() {
                let words = s.keywords.iter().map(|k| fancy_regex::escape(k).into_owned()).collect::<Vec<_>>().join("|");
                regexes.push(compile(&format!(r"(?:^|/)(?:{words})(?:/|$|\*|\.)"), &what)?);
            }
            if let Some(p) = &s.pattern {
                regexes.push(compile(p, &what)?);
            }
            if regexes.is_empty() {
                return Err(format!("{what}: needs keywords or a pattern"));
            }
            if Level::parse(&s.severity).is_none() && !matches!(s.severity.as_str(), "high" | "medium" | "low") {
                return Err(format!("{what}: severity must be high, medium or low"));
            }
            security.push(CompiledSecurity { category: s.category.clone(), severity: s.severity.clone(), reason: s.reason.clone(), regexes });
        }
        let security_ignore = config.security.ignore.iter().map(|p| compile(p, "security.ignore")).collect::<Result<_, _>>()?;

        let disabled: HashSet<&str> = config.recon.cms.disabled.iter().map(String::as_str).collect();
        let mut cms = Vec::new();
        for s in config.recon.cms.signatures.iter().filter(|s| !disabled.contains(s.name.as_str())) {
            let what = format!("recon.cms.signatures ({})", s.name);
            cms.push(CompiledCms {
                name: s.name.clone(),
                kind: s.kind.clone(),
                strong: s.strong.iter().map(|p| compile(p, &what)).collect::<Result<_, _>>()?,
                weak: s.weak.iter().map(|p| compile(p, &what)).collect::<Result<_, _>>()?,
                comments: s.comments.iter().map(|p| compile(p, &what)).collect::<Result<_, _>>()?,
            });
        }

        let mut cloud = Vec::new();
        for p in &config.recon.cloud.providers {
            let what = format!("recon.cloud.providers ({})", p.provider);
            cloud.push(CompiledProvider { provider: p.provider.clone(), kind: p.kind.clone(), host: compile(&p.host, &what)?, bucket: parse_bucket(&p.bucket, &what)? });
        }

        let h = &config.recon.hosts;
        let hosts = CompiledHosts {
            env_labels: compile(&h.env_labels, "recon.hosts.env_labels")?,
            env_label_fallback: compile(&h.env_label_fallback, "recon.hosts.env_label_fallback")?,
            env_paths: compile(&h.env_paths, "recon.hosts.env_paths")?,
            env_path_hint: compile(&h.env_path_hint, "recon.hosts.env_path_hint")?,
        };

        let api = config.recon.api.signatures.iter().map(|s| Ok(CompiledApi { kind: s.kind.clone(), pattern: compile(&s.pattern, "recon.api.signatures")? })).collect::<Result<Vec<_>, String>>()?;

        let d = &config.recon.data;
        let labelled = |list: &Vec<LabelledPattern>, what: &str| list.iter().map(|l| Ok(CompiledLabelled { pattern: compile(&l.pattern, what)?, label: l.label.clone() })).collect::<Result<Vec<_>, String>>();
        let data = CompiledData {
            feed_words: compile(&d.feed_words, "recon.data.feed_words")?,
            feed_file: compile(&d.feed_file, "recon.data.feed_file")?,
            feed_hint: compile(&d.feed_hint, "recon.data.feed_hint")?,
            feed_file_wildcard: compile(&d.feed_file_wildcard, "recon.data.feed_file_wildcard")?,
            portal_words: compile(&d.portal_words, "recon.data.portal_words")?,
            search_segments: compile(&d.search_segments, "recon.data.search_segments")?,
            search_keys: d.search_keys.iter().cloned().collect(),
            filter_keys: d.filter_keys.iter().cloned().collect(),
            tracking_keys: compile(&d.tracking_keys, "recon.data.tracking_keys")?,
            feed_kinds: labelled(&d.feed_kinds, "recon.data.feed_kinds")?,
            portal_kinds: labelled(&d.portal_kinds, "recon.data.portal_kinds")?,
        };

        let mut extensions = HashMap::new();
        for g in &config.recon.extensions.groups {
            for e in &g.exts {
                extensions.insert(e.to_lowercase(), ExtDef { category: g.category.clone(), label: g.label.clone(), risk: g.risk.clone(), tech: g.tech.clone() });
            }
        }

        let c = &config.recon.comments;
        let comments = CompiledComments {
            email: compile(&c.email, "recon.comments.email")?,
            person: compile(&c.person, "recon.comments.person")?,
            person_deny: compile(&c.person_deny, "recon.comments.person_deny")?,
            jira: compile(&c.jira, "recon.comments.jira")?,
            jira_deny: compile(&c.jira_deny, "recon.comments.jira_deny")?,
            ticket_word: compile(&c.ticket_word, "recon.comments.ticket_word")?,
            iso_date: compile(&c.iso_date, "recon.comments.iso_date")?,
            slash_date: compile(&c.slash_date, "recon.comments.slash_date")?,
            text_date: compile(&c.text_date, "recon.comments.text_date")?,
            ip: compile(&c.ip, "recon.comments.ip")?,
            vendors: compile(&c.vendors, "recon.comments.vendors")?,
            notes: compile(&c.notes, "recon.comments.notes")?,
        };

        let disabled_ids = config.rules.disabled.iter().cloned().collect();
        let mut level_overrides = HashMap::new();
        for (id, level) in &config.rules.levels {
            let l = Level::parse(level).ok_or_else(|| format!("rules.levels.{id}: level must be error, warning or info"))?;
            level_overrides.insert(id.clone(), l);
        }
        for c in &config.crawlers.list {
            if !config.crawlers.categories.contains(&c.category) {
                return Err(format!("crawlers.list ({}): unknown category {}", c.name, c.category));
            }
        }

        Ok(Engine { config, security, security_ignore, cms, cloud, hosts, api, data, extensions, comments, disabled_ids, level_overrides })
    }

    /// Category label key for a security category id.
    pub fn security_category(&self, id: &str) -> Option<&SecurityCategory> {
        self.config.security.categories.iter().find(|c| c.id == id)
    }
}

fn parse_bucket(spec: &str, what: &str) -> Result<Vec<Vec<BucketPart>>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec == "none" {
        return Ok(Vec::new());
    }
    let mut alternatives = Vec::new();
    for alt in spec.split('|') {
        let mut parts = Vec::new();
        for part in alt.split(" + ") {
            let part = part.trim();
            let p = if let Some(n) = part.strip_prefix("host:") {
                BucketPart::HostGroup(n.trim().parse().map_err(|_| format!("{what}: bad bucket spec `{part}`"))?)
            } else if let Some(n) = part.strip_prefix("segment:") {
                BucketPart::Segment(n.trim().parse().map_err(|_| format!("{what}: bad bucket spec `{part}`"))?)
            } else if let Some(re) = part.strip_prefix("path:") {
                BucketPart::Path(compile(re, what)?)
            } else {
                return Err(format!("{what}: bad bucket spec `{part}`"));
            };
            parts.push(p);
        }
        alternatives.push(parts);
    }
    Ok(alternatives)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_compile() {
        let e = Engine::default_engine();
        assert_eq!(e.config.crawlers.list.len(), 33);
        assert!(e.cms.len() > 40);
        assert!(e.cloud.len() > 30);
    }

    #[test]
    fn user_config_merges_and_validates() {
        let e = Engine::from_toml(Some("[checks]\nsecurity = false\n[rules]\ndisabled = [\"seo.caseSensitive\"]\n[rules.levels]\n\"seo.trailingSlash\" = \"info\"\n[[crawlers.list]]\nname = \"MyBot\"\ntokens = [\"mybot\"]\ncategory = \"seo\"\n")).unwrap();
        assert!(!e.config.checks.security);
        assert!(e.config.checks.seo_traps);
        assert!(e.disabled_ids.contains("seo.caseSensitive"));
        assert_eq!(e.level_overrides["seo.trailingSlash"], Level::Info);
        assert_eq!(e.config.crawlers.list.len(), 1, "arrays replace");
        assert!(Engine::from_toml(Some("[[security.signatures]]\ncategory = \"x\"\nseverity = \"high\"\nreason = \"r\"\npattern = \"(\"\n")).is_err());
        assert!(Engine::from_toml(Some("[rules.levels]\n\"x\" = \"loud\"\n")).is_err());
        assert!(Engine::from_toml(Some("not = [valid")).is_err());
    }
}
