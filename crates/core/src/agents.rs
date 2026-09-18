//! Per-crawler summaries from the configured crawler list.

use crate::analyser::{summarise, Summary, Verdict};
use crate::config::{Crawler, Engine};
use crate::i18n::Locale;
use crate::model::Model;

pub struct CrawlerSummary<'a> {
    pub crawler: &'a Crawler,
    pub summary: Summary,
}

pub fn summaries<'a>(model: &Model, engine: &'a Engine) -> Vec<CrawlerSummary<'a>> {
    engine.config.crawlers.list.iter().map(|c| CrawlerSummary { crawler: c, summary: summarise(model, &c.tokens) }).collect()
}

pub fn category_label(locale: &Locale, category: &str) -> String {
    locale.s(&format!("agents.category.{category}"))
}

pub fn verdict_text(locale: &Locale, v: Verdict) -> String {
    locale.s(&format!("enum.verdict.{}", v.as_str()))
}
