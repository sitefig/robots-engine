//! The top-level entry point: text plus options in, everything out. The
//! browser bindings and the CLI are thin wrappers around this.

use crate::analyser::{apply_clean_params, check_access, Access, Cleaned};
use crate::checks::{apply_rule_overrides, run_checks};
use crate::config::Engine;
use crate::export::{csv, markdown};
use crate::fetch::{fetch_warnings, FetchInfo};
use crate::i18n::Locale;
use crate::model::Model;
use crate::parser::parse;
use crate::report::{build_report, Report, ReportInput};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Options {
    /// Origin the file was served from (or assumed for examples); enables origin-dependent checks.
    #[serde(rename = "siteUrl", default)]
    pub site_url: Option<String>,
    /// How the file was fetched; `None` for pasted text.
    #[serde(default)]
    pub fetch: Option<FetchInfo>,
    /// User TOML merged over the defaults.
    #[serde(default)]
    pub config: Option<String>,
    /// Language code of the locale dictionary.
    #[serde(default)]
    pub lang: Option<String>,
    /// Locale dictionary as JSON; English when absent.
    #[serde(default)]
    pub locale: Option<String>,
    /// ISO 8601 timestamp for `generatedAt` and the date notes.
    #[serde(default)]
    pub now: Option<String>,
    /// Absolute URL of the published report schema.
    #[serde(rename = "schemaUrl", default)]
    pub schema_url: Option<String>,
}

pub struct Analysis {
    pub engine: Arc<Engine>,
    pub locale: Arc<Locale>,
    pub model: Model,
    pub text: String,
    pub report: Report,
}

/// (year, month, day) from the start of an ISO timestamp.
pub fn date_of(iso: &str) -> Option<(i32, u32, u32)> {
    let mut it = iso.split(|c| c == '-' || c == 'T').take(3);
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

impl Analysis {
    /// Build the engine and locale from the options, then analyse.
    pub fn new(text: &str, options: Options) -> Result<Analysis, String> {
        let engine = Engine::from_toml(options.config.as_deref())?;
        let locale = match (&options.lang, &options.locale) {
            (_, Some(json)) => Locale::from_json(options.lang.as_deref().unwrap_or("en"), json)?,
            (Some(lang), None) if lang != "en" => Locale::from_json(lang, "{}")?,
            _ => Locale::english(),
        };
        Analysis::with_engine(text, options, Arc::new(engine), Arc::new(locale))
    }

    /// Analyse with an engine and locale built once and shared (batch jobs, threads).
    /// `options.config`, `lang` and `locale` are ignored here.
    pub fn with_engine(text: &str, options: Options, engine: Arc<Engine>, locale: Arc<Locale>) -> Result<Analysis, String> {
        // Crawlers ignore the body of non-2xx responses and give up after five redirects.
        let usable = options.fetch.as_ref().map(|f| f.usable()).unwrap_or(true);
        let text = if usable { text.to_string() } else { String::new() };
        let site_url = options.site_url.clone().or_else(|| options.fetch.as_ref().map(|f| f.final_url.clone()));

        let mut model = parse(&text, &locale);
        model.warnings.extend(run_checks(&model, site_url.as_deref(), &engine, &locale));
        if let Some(f) = &options.fetch {
            model.warnings.extend(fetch_warnings(f, &locale));
        }
        apply_rule_overrides(&mut model.warnings, &engine);
        let engine_ref: &Engine = &engine;
        let locale_ref: &Locale = &locale;

        let now = options.now.clone().unwrap_or_else(|| "1970-01-01T00:00:00.000Z".into());
        let report = build_report(
            ReportInput {
                model: &model,
                text: &text,
                fetch: options.fetch.as_ref(),
                site_url: site_url.as_deref(),
                schema_url: options.schema_url.as_deref().unwrap_or("schema/report.schema.json"),
                generated_at: &now,
                today: date_of(&now),
            },
            engine_ref,
            locale_ref,
        );
        Ok(Analysis { engine, locale, model, text, report })
    }

    pub fn report_json(&self) -> String {
        serde_json::to_string(&self.report).expect("report serialises")
    }

    pub fn report_json_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.report).expect("report serialises")
    }

    pub fn check_access(&self, tokens: &[String], path: &str) -> Access {
        check_access(&self.model, tokens, path)
    }

    pub fn clean_params(&self, path: &str) -> Cleaned {
        apply_clean_params(&self.model, path)
    }

    pub fn markdown(&self) -> String {
        markdown::audit_markdown(&self.report, &self.engine, &self.locale)
    }

    pub fn html(&self) -> String {
        markdown::audit_html(&self.report, &self.engine, &self.locale)
    }

    pub fn recommended_actions(&self) -> Vec<String> {
        markdown::recommended_actions(&self.report, &self.engine, &self.locale)
    }

    pub fn csv_tabs(&self) -> Vec<(String, String)> {
        csv::csv_tabs(&self.report, &self.engine, &self.locale)
    }

    pub fn tagged_csv(&self) -> String {
        csv::tagged_csv(&self.report, &self.engine, &self.locale)
    }

    pub fn tsv(&self, tab: &str) -> Option<String> {
        csv::tsv_for(&self.report, &self.engine, &self.locale, tab)
    }

    pub fn tab_labels(&self) -> Vec<(String, String)> {
        csv::tab_labels(&self.locale)
    }

    pub fn filename(&self) -> String {
        crate::report::report_filename(&self.report)
    }
}
