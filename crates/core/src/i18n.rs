//! Message formatting. A locale is a flat map of dotted keys to strings, or to
//! plural objects keyed by CLDR category (`one`, `few`, `many`, `other`).
//! `{name}` placeholders are substituted; unknown placeholders are left in
//! place; a missing key falls back to English, then to the key itself, so a
//! plain-text value in a config file (not a key) is shown as written.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const DEFAULT_LANG: &str = "en";
const EN_JSON: &str = include_str!("../data/en.json");

#[derive(Debug, Clone)]
pub enum Entry {
    Text(String),
    Plural(HashMap<String, String>),
}

pub type Dict = HashMap<String, Entry>;

static ENGLISH: LazyLock<Dict> = LazyLock::new(|| parse_dict(EN_JSON).expect("embedded en.json is valid"));

/// Parameters for a message: name and value. Build with [`params!`].
pub type Params<'a> = [(&'a str, String)];

/// `params!{ "n" => 3, "path" => p }` builds a `Vec<(&str, String)>`.
#[macro_export]
macro_rules! params {
    ($($k:expr => $v:expr),* $(,)?) => {
        vec![$(($k, ($v).to_string())),*]
    };
}

pub fn parse_dict(json: &str) -> Result<Dict, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("locale is not valid JSON: {e}"))?;
    let obj = value.as_object().ok_or("locale must be a JSON object")?;
    let mut dict = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        let entry = match v {
            Value::String(s) => Entry::Text(s.clone()),
            Value::Object(forms) => Entry::Plural(
                forms
                    .iter()
                    .filter_map(|(cat, s)| s.as_str().map(|s| (cat.clone(), s.to_string())))
                    .collect(),
            ),
            _ => return Err(format!("locale key {k} must be a string or a plural object")),
        };
        dict.insert(k.clone(), entry);
    }
    Ok(dict)
}

pub struct Locale {
    lang: String,
    dict: Dict,
}

impl Locale {
    pub fn english() -> Locale {
        Locale { lang: DEFAULT_LANG.to_string(), dict: Dict::new() }
    }

    /// A locale from a JSON dictionary. `lang` drives plural selection.
    pub fn from_json(lang: &str, json: &str) -> Result<Locale, String> {
        let lang = if lang.is_empty() { DEFAULT_LANG } else { lang };
        Ok(Locale { lang: lang.to_string(), dict: parse_dict(json)? })
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// True when the key exists in this locale or in English.
    pub fn has(&self, key: &str) -> bool {
        self.dict.contains_key(key) || ENGLISH.contains_key(key)
    }

    fn category(&self, n: f64) -> &'static str {
        crate::plural::category(&self.lang, n)
    }

    fn pick<'a>(&self, entry: &'a Entry, n: Option<f64>) -> Option<&'a str> {
        match entry {
            Entry::Text(s) => Some(s),
            Entry::Plural(forms) => {
                let cat = n.map(|n| self.category(n)).unwrap_or("other");
                forms.get(cat).or_else(|| forms.get("other")).map(String::as_str)
            }
        }
    }

    /// Translate `key`, substituting `{name}` placeholders from `params`.
    pub fn t(&self, key: &str, params: &Params) -> String {
        let n = params.iter().find(|(k, _)| *k == "n").and_then(|(_, v)| v.parse::<f64>().ok());
        let template = self
            .dict
            .get(key)
            .and_then(|e| self.pick(e, n))
            .or_else(|| ENGLISH.get(key).and_then(|e| self.pick(e, n)));
        match template {
            Some(t) => substitute(t, params),
            None => key.to_string(),
        }
    }

    /// Translate without parameters.
    pub fn s(&self, key: &str) -> String {
        self.t(key, &[])
    }
}

/// Replace `{name}` with the parameter value in one pass, so values are never re-scanned.
fn substitute(template: &str, params: &Params) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if after[..end].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && end > 0 => {
                let name = &after[..end];
                match params.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// One decimal for kibibytes, no locale-specific separators.
pub fn kib(bytes: usize) -> String {
    let v = (bytes as f64 / 102.4).round() / 10.0;
    if v.fract() == 0.0 { format!("{}", v as u64) } else { format!("{v:.1}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_and_placeholders() {
        let en = Locale::english();
        assert_eq!(en.t("parser.unknownDirective", &params! {"field" => "foo"}), "Unknown directive \"foo\" is ignored.");
        assert_eq!(en.s("nope-missing"), "nope-missing");
        assert_eq!(en.t("md.groups", &params! {"n" => 1}), "1 group");
        assert_eq!(en.t("md.groups", &params! {"n" => 2}), "2 groups");
    }

    #[test]
    fn fallback_and_plurals() {
        let cs = Locale::from_json("cs", r#"{"x.n": {"one": "{n} skupina", "few": "{n} skupiny", "other": "{n} skupin"}, "parser.bom": "BOM"}"#).unwrap();
        assert_eq!(cs.t("x.n", &params! {"n" => 1}), "1 skupina");
        assert_eq!(cs.t("x.n", &params! {"n" => 3}), "3 skupiny");
        assert_eq!(cs.t("x.n", &params! {"n" => 7}), "7 skupin");
        assert_eq!(cs.s("parser.bom"), "BOM");
        assert_eq!(cs.s("parser.notFieldValue"), Locale::english().s("parser.notFieldValue"));
        assert_eq!(substitute("{a} and {b} {c}", &params! {"a" => "{b}", "b" => "B"}), "{b} and B {c}");
    }
}
