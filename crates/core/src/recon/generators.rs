//! The tool that wrote the file. Most robots.txt files are generated, and the
//! generator leaves its name in a comment: an SEO plugin, a shop platform, a
//! hosting panel. Knowing the writer says who to ask for a change.

use crate::config::Engine;
use crate::model::Model;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Generator {
    pub name: String,
    pub url: Option<String>,
    pub line: u32,
    /// The comment that named it.
    pub comment: String,
}

pub fn find_generators(model: &Model, engine: &Engine) -> Vec<Generator> {
    let comments: Vec<(u32, String, String)> = model.comment_lines().into_iter().map(|(n, t)| (n, t.to_lowercase(), t)).collect();
    let mut out: Vec<Generator> = Vec::new();
    for sig in &engine.generators {
        for (line, lower, text) in &comments {
            if sig.comments.iter().any(|re| re.is_match(lower).unwrap_or(false)) {
                out.push(Generator { name: sig.name.clone(), url: sig.url.clone(), line: *line, comment: text.clone() });
                break;
            }
        }
    }
    out.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.name.cmp(&b.name)));
    out
}
