//! susbot-core: the robots.txt analysis engine behind sus.bot.
//!
//! Pure library, no I/O. Text goes in; a parsed model, findings and a
//! normalised report come out. The same crate is compiled to WebAssembly for
//! the browser (`susbot-wasm`) and into a CLI for CI (`susbot-cli`).
//!
//! Every rule the engine applies comes from a TOML configuration
//! (`config/default.toml` is embedded; a user config merges on top), and every
//! human-readable string comes from a locale dictionary (`locales/en.json` is
//! embedded; other languages are loaded at runtime). Findings carry a stable
//! `id` (the dictionary key) next to the translated `message`.

pub mod agents;
pub mod ai_status;
pub mod analyser;
pub mod analysis;
pub mod checks;
pub mod config;
pub mod diff;
pub mod export;
pub mod fetch;
pub mod i18n;
pub mod model;
pub mod parser;
pub mod plural;
pub mod recon;
pub mod report;
pub mod security;
pub mod url_util;

pub use analysis::{Analysis, Options};
pub use config::{Config, Engine};
pub use i18n::Locale;
pub use model::Model;

/// The engine version, shared by the CLI and the report.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
