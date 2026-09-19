//! The `susbot._susbot` extension module. Values cross the boundary as JSON
//! strings, exactly as in the browser bindings; `python/susbot/__init__.py`
//! turns them into dicts and gives the API its keyword arguments.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use susbot_core::{Analysis as CoreAnalysis, Options};

fn value_error(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// One analysed robots.txt file.
#[pyclass(module = "susbot._susbot", frozen)]
struct Analysis {
    inner: CoreAnalysis,
}

#[pymethods]
impl Analysis {
    /// `options_json` is the JSON form of `susbot_core::Options`.
    #[new]
    fn new(py: Python<'_>, text: &str, options_json: &str) -> PyResult<Self> {
        let options: Options = if options_json.trim().is_empty() { Options::default() } else { serde_json::from_str(options_json).map_err(|e| value_error(format!("options: {e}")))? };
        let inner = py.detach(|| CoreAnalysis::new(text, options)).map_err(value_error)?;
        Ok(Analysis { inner })
    }

    fn report_json(&self, pretty: bool) -> String {
        if pretty { self.inner.report_json_pretty() } else { self.inner.report_json() }
    }

    fn check_access(&self, tokens: Vec<String>, path: &str) -> String {
        serde_json::to_string(&self.inner.check_access(&tokens, path)).unwrap()
    }

    fn clean_params(&self, path: &str) -> String {
        serde_json::to_string(&self.inner.clean_params(path)).unwrap()
    }

    fn markdown(&self) -> String {
        self.inner.markdown()
    }

    fn html(&self) -> String {
        self.inner.html()
    }

    fn recommended_actions(&self) -> Vec<String> {
        self.inner.recommended_actions()
    }

    fn csv_tabs(&self) -> Vec<(String, String)> {
        self.inner.csv_tabs()
    }

    fn tab_labels(&self) -> Vec<(String, String)> {
        self.inner.tab_labels()
    }

    fn tagged_csv(&self) -> String {
        self.inner.tagged_csv()
    }

    fn tsv(&self, tab: &str) -> Option<String> {
        self.inner.tsv(tab)
    }

    fn filename(&self) -> String {
        self.inner.filename()
    }

    fn text(&self) -> &str {
        &self.inner.text
    }

    fn crawlers_json(&self) -> String {
        serde_json::to_string(&self.inner.engine.config.crawlers).unwrap()
    }
}

/// Semantic diff of two analyses as JSON.
#[pyfunction]
fn diff_json(old: &Analysis, new: &Analysis) -> String {
    serde_json::to_string(&susbot_core::diff::diff_analyses(&old.inner, &new.inner)).unwrap()
}

/// The semantic diff rendered as Markdown for `domain`.
#[pyfunction]
fn diff_markdown(old: &Analysis, new: &Analysis, domain: &str) -> String {
    susbot_core::diff::diff_analyses(&old.inner, &new.inner).to_markdown(domain)
}

/// The embedded default configuration, TOML text.
#[pyfunction]
fn default_config() -> &'static str {
    susbot_core::config::DEFAULT_TOML
}

/// Raises ValueError when the user TOML does not merge and compile.
#[pyfunction]
fn validate_config(user_toml: &str) -> PyResult<()> {
    susbot_core::Engine::from_toml(Some(user_toml)).map(|_| ()).map_err(value_error)
}

/// Runs the command-line tool with `argv` (program name first) and returns
/// its exit code. The GIL is released, so other Python threads keep running.
#[pyfunction]
fn run_cli(py: Python<'_>, argv: Vec<String>) -> u8 {
    py.detach(|| susbot_cli::run_with(argv))
}

#[pymodule]
fn _susbot(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", susbot_core::VERSION)?;
    m.add_class::<Analysis>()?;
    m.add_function(wrap_pyfunction!(diff_json, m)?)?;
    m.add_function(wrap_pyfunction!(diff_markdown, m)?)?;
    m.add_function(wrap_pyfunction!(default_config, m)?)?;
    m.add_function(wrap_pyfunction!(validate_config, m)?)?;
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    Ok(())
}
