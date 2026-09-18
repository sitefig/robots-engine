//! Browser bindings. Everything crosses the boundary as JSON strings so the
//! page needs nothing but `JSON.parse`.

use susbot_core::{Analysis as CoreAnalysis, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Analysis {
    inner: CoreAnalysis,
}

#[wasm_bindgen]
impl Analysis {
    /// `options` is the JSON form of `susbot_core::Options`.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str, options_json: &str) -> Result<Analysis, JsError> {
        let options: Options = if options_json.trim().is_empty() { Options::default() } else { serde_json::from_str(options_json).map_err(|e| JsError::new(&format!("options: {e}")))? };
        let inner = CoreAnalysis::new(text, options).map_err(|e| JsError::new(&e))?;
        Ok(Analysis { inner })
    }

    #[wasm_bindgen(js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.inner.report_json()
    }

    #[wasm_bindgen(js_name = reportJsonPretty)]
    pub fn report_json_pretty(&self) -> String {
        self.inner.report_json_pretty()
    }

    /// `tokens_json` is a JSON array of product tokens, most specific first.
    #[wasm_bindgen(js_name = checkAccess)]
    pub fn check_access(&self, tokens_json: &str, path: &str) -> Result<String, JsError> {
        let tokens: Vec<String> = serde_json::from_str(tokens_json).map_err(|e| JsError::new(&format!("tokens: {e}")))?;
        Ok(serde_json::to_string(&self.inner.check_access(&tokens, path)).unwrap())
    }

    #[wasm_bindgen(js_name = cleanParams)]
    pub fn clean_params(&self, path: &str) -> String {
        serde_json::to_string(&self.inner.clean_params(path)).unwrap()
    }

    pub fn markdown(&self) -> String {
        self.inner.markdown()
    }

    pub fn html(&self) -> String {
        self.inner.html()
    }

    /// JSON object of tab id to CSV text.
    #[wasm_bindgen(js_name = csvTabs)]
    pub fn csv_tabs(&self) -> String {
        let map: serde_json::Map<String, serde_json::Value> = self.inner.csv_tabs().into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect();
        serde_json::Value::Object(map).to_string()
    }

    #[wasm_bindgen(js_name = taggedCsv)]
    pub fn tagged_csv(&self) -> String {
        self.inner.tagged_csv()
    }

    pub fn tsv(&self, tab: &str) -> Option<String> {
        self.inner.tsv(tab)
    }

    /// JSON object of tab id to translated label.
    #[wasm_bindgen(js_name = tabLabels)]
    pub fn tab_labels(&self) -> String {
        let map: serde_json::Map<String, serde_json::Value> = self.inner.tab_labels().into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect();
        serde_json::Value::Object(map).to_string()
    }

    pub fn filename(&self) -> String {
        self.inner.filename()
    }

    /// Per-line kinds of the parsed file (`[{ n, kind }]`), for the raw view.
    pub fn lines(&self) -> String {
        let lines: Vec<serde_json::Value> = self.inner.model.lines.iter().map(|l| serde_json::json!({ "n": l.n, "kind": l.kind })).collect();
        serde_json::Value::Array(lines).to_string()
    }

    /// The crawler list of the active config, for the tester and the access check.
    pub fn crawlers(&self) -> String {
        serde_json::to_string(&self.inner.engine.config.crawlers).unwrap()
    }
}

/// The embedded default configuration, TOML text.
#[wasm_bindgen(js_name = defaultConfig)]
pub fn default_config() -> String {
    susbot_core::config::DEFAULT_TOML.to_string()
}

/// Empty string when the user TOML merges and compiles; otherwise the error.
#[wasm_bindgen(js_name = validateConfig)]
pub fn validate_config(user_toml: &str) -> String {
    match susbot_core::Engine::from_toml(Some(user_toml)) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

#[wasm_bindgen]
pub fn version() -> String {
    susbot_core::VERSION.to_string()
}
