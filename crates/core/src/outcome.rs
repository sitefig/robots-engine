//! What really happens to a path, as a sentence a finding can open with.
//!
//! A finding about one line says what that line was meant to do. Whether the
//! page ends up hidden or shown is decided by the whole file: another rule can
//! block the path the broken line failed to block, or open the one it failed
//! to open. So the outcome is always matched against every rule that applies
//! to the crawlers the line speaks to, and the sentence names the rule that
//! decided it.

use crate::analyser::{evaluate_path, merged_rules};
use crate::i18n::Locale;
use crate::model::*;
use crate::params;

/// How many crawler names a sentence lists before it counts the rest.
const NAMES_SHOWN: usize = 3;

/// The crawlers a line speaks to.
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    /// Crawlers that follow the `*` group: every crawler without its own group.
    Default,
    /// The crawlers a group names, as written in the file, and the token the
    /// rules are merged under.
    Named { names: Vec<String>, token: String },
}

/// The scope of a group: `*` when the group names it, its crawlers otherwise.
/// A line outside every group, or in a group that names no crawler, is judged
/// by what crawlers in general get.
pub fn scope_of(group: Option<&Group>) -> Scope {
    let Some(group) = group else { return Scope::Default };
    let named: Vec<&Agent> = group.agents.iter().filter(|a| !a.token.is_empty()).collect();
    if named.is_empty() || named.iter().any(|a| a.token == "*") {
        return Scope::Default;
    }
    Scope::Named { names: named.iter().map(|a| a.raw.trim().to_string()).collect(), token: named[0].token.clone() }
}

/// Every rule the crawlers of a scope follow, merged across groups.
pub fn rules_for<'a>(groups: &'a [Group], scope: &Scope) -> Vec<&'a Rule> {
    let token = match scope {
        Scope::Default => "*",
        Scope::Named { token, .. } => token.as_str(),
    };
    let matching: Vec<&Group> = groups.iter().filter(|g| g.agents.iter().any(|a| a.token == token)).collect();
    merged_rules(&matching)
}

/// A concrete path a rule pattern stands for: wildcards and the end anchor
/// removed, and a leading slash, because that is what a crawler requests.
pub fn sample_path(pattern: &str) -> String {
    let plain = plain_path(pattern.trim());
    if plain.starts_with('/') {
        plain
    } else {
        format!("/{plain}")
    }
}

/// Whether a path is allowed for a scope, and the rule that decided it.
pub fn decide<'a>(groups: &'a [Group], scope: &Scope, path: &str) -> (bool, Option<&'a Rule>) {
    evaluate_path(&rules_for(groups, scope), path)
}

fn names_text(locale: &Locale, names: &[String]) -> String {
    let shown = names.iter().take(NAMES_SHOWN).cloned().collect::<Vec<_>>().join(", ");
    if names.len() > NAMES_SHOWN {
        locale.t("outcome.names.more", &params! {"names" => shown, "n" => names.len() - NAMES_SHOWN})
    } else {
        shown
    }
}

/// The sentence: "{path} is open to …", "… is open to …, because … allows it"
/// or "… is blocked for …, by …", for everyone, for crawlers without their own
/// rules, or for the crawlers a group names.
pub fn sentence(locale: &Locale, groups: &[Group], scope: &Scope, path: &str) -> String {
    let (allowed, rule) = decide(groups, scope, path);
    let state = match (allowed, rule) {
        (true, None) => "open",
        (true, Some(_)) => "allowed",
        (false, _) => "blocked",
    };
    let mut p = params! {"path" => path};
    if let Some(r) = rule {
        p.push(("winner", r.label()));
        p.push(("line", r.line.to_string()));
    }
    let who = match scope {
        Scope::Named { names, .. } => {
            p.push(("names", names_text(locale, names)));
            "named"
        }
        // "Every crawler" is only true when no crawler has rules of its own.
        Scope::Default if groups.iter().any(|g| g.agents.iter().any(|a| !a.token.is_empty() && a.token != "*")) => "default",
        Scope::Default => "all",
    };
    locale.t(&format!("outcome.{state}.{who}"), &p)
}
