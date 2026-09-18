//! Human metadata inside "#" comments: emails, names, vendors, ticket IDs,
//! dates, URLs, IPs, and notes about intent.

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::Model;
use crate::params;
use crate::url_util::{extract_urls, is_private_ip};
use serde::Serialize;

pub const KINDS: [&str; 8] = ["email", "person", "ticket", "date", "ip", "url", "vendor", "note"];

#[derive(Debug, Clone, Serialize)]
pub struct CommentFinding {
    pub kind: String,
    pub value: String,
    pub line: u32,
    pub comment: String,
    pub note: Option<String>,
}

/// Days from `today` to a civil date, both as (y, m, d).
fn days_between(from: (i32, u32, u32), to: (i32, u32, u32)) -> i64 {
    fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y } as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }
    days_from_civil(to.0, to.1, to.2) - days_from_civil(from.0, from.1, from.2)
}

fn date_note(date: Option<(i32, u32, u32)>, today: Option<(i32, u32, u32)>, locale: &Locale) -> Option<String> {
    let (date, today) = (date?, today?);
    let days = days_between(today, date);
    if days > 365 {
        Some(locale.t("recon.comments.date.yearsAhead", &params! {"n" => (days as f64 / 365.0).round() as i64}))
    } else if days > 0 {
        Some(locale.t("recon.comments.date.daysAhead", &params! {"n" => days}))
    } else if days < -365 {
        Some(locale.t("recon.comments.date.yearsOld", &params! {"n" => (-days as f64 / 365.0).round() as i64}))
    } else {
        None
    }
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// Best-effort parse of the textual dates the regex accepts ("Oct 1, 2099", "1 October 2099", "October 2099").
fn parse_text_date(s: &str) -> Option<(i32, u32, u32)> {
    let lower = s.to_lowercase();
    let year: i32 = lower.split(|c: char| !c.is_ascii_digit()).filter(|t| t.len() == 4).last()?.parse().ok()?;
    let month = MONTHS.iter().position(|m| lower.contains(m)).map(|i| i as u32 + 1)?;
    let day = lower.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty() && t.len() <= 2).find_map(|t| t.parse::<u32>().ok()).filter(|d| (1..=31).contains(d)).unwrap_or(1);
    Some((year, month, day))
}

pub fn mine_comments(model: &Model, engine: &Engine, locale: &Locale, today: Option<(i32, u32, u32)>) -> Vec<CommentFinding> {
    let c = &engine.comments;
    let mut out: Vec<CommentFinding> = Vec::new();
    for (line, text) in model.comment_lines() {
        let mut push = |kind: &str, value: &str, note: Option<String>| {
            let value = value.trim().to_string();
            if out.iter().any(|f| f.kind == kind && f.value.to_lowercase() == value.to_lowercase() && f.line == line) {
                return;
            }
            out.push(CommentFinding { kind: kind.into(), value, line, comment: text.clone(), note });
        };
        for m in c.email.find_iter(&text).flatten() {
            push("email", m.as_str(), None);
        }
        for caps in c.person.captures_iter(&text).flatten() {
            let v = caps.get(1).unwrap().as_str().trim();
            let whole = caps.get(0).unwrap();
            let rest = &text[whole.start() + whole.as_str().find(v).unwrap_or(0)..];
            if c.email.find(rest).ok().flatten().map(|m| m.start() == 0).unwrap_or(false) {
                continue; // "updated by dave@company.com" is an email, not a name
            }
            if !c.person_deny.is_match(v).unwrap_or(false) {
                push("person", v, None);
            }
        }
        for caps in c.jira.captures_iter(&text).flatten() {
            let v = caps.get(1).unwrap().as_str();
            if !c.jira_deny.is_match(v).unwrap_or(false) {
                push("ticket", v, None);
            }
        }
        for m in c.ticket_word.find_iter(&text).flatten() {
            push("ticket", m.as_str(), None);
        }
        for caps in c.iso_date.captures_iter(&text).flatten() {
            let (y, mo, d) = (caps.get(1).unwrap().as_str().parse().ok(), caps.get(2).unwrap().as_str().parse().ok(), caps.get(3).unwrap().as_str().parse().ok());
            let date = match (y, mo, d) {
                (Some(y), Some(mo), Some(d)) => Some((y, mo, d)),
                _ => None,
            };
            push("date", caps.get(0).unwrap().as_str(), date_note(date, today, locale));
        }
        for m in c.slash_date.find_iter(&text).flatten() {
            push("date", m.as_str(), None);
        }
        for m in c.text_date.find_iter(&text).flatten() {
            push("date", m.as_str(), date_note(parse_text_date(m.as_str()), today, locale));
        }
        for m in c.ip.find_iter(&text).flatten() {
            let note = if is_private_ip(m.as_str()) { locale.s("recon.comments.ip.private") } else { locale.s("recon.comments.ip.public") };
            push("ip", m.as_str(), Some(note));
        }
        for u in extract_urls(&text) {
            push("url", u.as_str(), None);
        }
        for caps in c.vendors.captures_iter(&text).flatten() {
            push("vendor", caps.get(1).unwrap().as_str(), None);
        }
        if let Ok(Some(caps)) = c.notes.captures(&text) {
            let v = caps.get(1).unwrap().as_str().to_lowercase();
            push("note", &v, None);
        }
    }
    let pos = |k: &str| KINDS.iter().position(|x| *x == k).unwrap_or(usize::MAX);
    out.sort_by(|a, b| pos(&a.kind).cmp(&pos(&b.kind)).then(a.line.cmp(&b.line)));
    out
}
