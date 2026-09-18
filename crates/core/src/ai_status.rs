//! The AI scraping status card: how each AI crawler fares, a headline and a callout.

use crate::agents::{category_label, summaries, CrawlerSummary};
use crate::analyser::Verdict;
use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::Model;
use crate::params;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Counts {
    pub open: usize,
    pub partial: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlerStatus {
    pub name: String,
    pub verdict: Verdict,
    #[serde(rename = "groupUsed")]
    pub group_used: Option<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiGroup {
    pub category: String,
    pub label: String,
    pub counts: Counts,
    pub crawlers: Vec<CrawlerStatus>,
    #[serde(skip)]
    pub specific: Vec<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Callout {
    pub state: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiStatus {
    pub headline: String,
    pub callout: Callout,
    pub groups: Vec<AiGroup>,
}

/// Short explanation for one crawler's pill tooltip.
pub fn explain(s: &CrawlerSummary, locale: &Locale) -> String {
    let how = match (&s.summary.token, s.summary.specific) {
        (None, _) => locale.s("ai.noGroup"),
        (Some(t), true) => locale.t("ai.ownGroup", &params! {"token" => t}),
        (Some(_), false) => locale.s("ai.starGroup"),
    };
    match &s.crawler.note {
        Some(n) => format!("{how} {}", locale.s(n)),
        None => how,
    }
}

pub fn ai_status(model: &Model, engine: &Engine, locale: &Locale) -> AiStatus {
    let all = summaries(model, engine);
    let groups: Vec<AiGroup> = engine
        .config
        .crawlers
        .ai_categories
        .iter()
        .map(|category| {
            let members: Vec<&CrawlerSummary> = all.iter().filter(|s| &s.crawler.category == category).collect();
            let mut counts = Counts::default();
            for s in &members {
                match s.summary.verdict {
                    Verdict::Open => counts.open += 1,
                    Verdict::Partial => counts.partial += 1,
                    Verdict::Blocked => counts.blocked += 1,
                }
            }
            AiGroup {
                category: category.clone(),
                label: category_label(locale, category),
                counts,
                crawlers: members.iter().map(|s| CrawlerStatus { name: s.crawler.name.clone(), verdict: s.summary.verdict, group_used: s.summary.token.clone(), explanation: explain(s, locale) }).collect(),
                specific: members.iter().map(|s| s.summary.specific).collect(),
            }
        })
        .collect();
    let headline = groups.iter().map(|g| locale.t("ai.headline", &params! {"label" => g.label, "blocked" => g.counts.blocked, "total" => g.crawlers.len()})).collect::<Vec<_>>().join(" · ");
    let callout = match groups.first() {
        Some(training) => training_callout(training, locale),
        None => Callout { state: "info".into(), text: locale.s("ai.callout.none") },
    };
    AiStatus { headline, callout, groups }
}

fn training_callout(g: &AiGroup, locale: &Locale) -> Callout {
    let total = g.crawlers.len();
    let c = |state: &str, text: String| Callout { state: state.into(), text };
    if total == 0 {
        return c("info", locale.s("ai.callout.none"));
    }
    if g.counts.blocked == total {
        return c("ok", locale.s("ai.callout.allBlocked"));
    }
    if g.counts.blocked == 0 && g.counts.partial == 0 {
        return c("info", locale.s("ai.callout.noneRestricted"));
    }
    if g.counts.blocked == 0 && g.specific.iter().all(|s| !s) {
        return c("info", locale.s("ai.callout.noneNamed"));
    }
    let open: Vec<&str> = g.crawlers.iter().filter(|s| s.verdict != Verdict::Blocked).map(|s| s.name.as_str()).collect();
    c("warning", locale.t("ai.callout.uneven", &params! {"list" => open.join(", ")}))
}
