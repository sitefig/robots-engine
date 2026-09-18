//! Platform fingerprinting from the paths a robots.txt disallows and the
//! comments it carries. Score = 2 per strong hit + 1 per weak hit + 3 per
//! comment hit; weak-only evidence needs two hits.

use crate::config::Engine;
use crate::model::Model;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    pub name: String,
    pub kind: String,
    pub confidence: String,
    pub score: u32,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Primary {
    pub name: String,
    pub kind: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Stack {
    pub primary: Option<Primary>,
    pub detections: Vec<Detection>,
}

pub fn detect_stack(model: &Model, engine: &Engine) -> Stack {
    let rules = model.each_rule();
    let comments: Vec<(u32, String, String)> = model.comment_lines().into_iter().map(|(n, t)| (n, t.to_lowercase(), t)).collect();
    let mut detections = Vec::new();
    for sig in &engine.cms {
        let mut strong_hits: Vec<usize> = Vec::new();
        let mut weak_hits: Vec<usize> = Vec::new();
        let mut comment_hits = 0u32;
        let mut evidence = Vec::new();
        let mut seen_lines: Vec<u32> = Vec::new();
        for r in &rules {
            let hit = |bucket: &mut Vec<usize>, index: usize, evidence: &mut Vec<Evidence>, seen: &mut Vec<u32>| {
                if !bucket.contains(&index) {
                    bucket.push(index);
                }
                if !seen.contains(&r.rule.line) {
                    seen.push(r.rule.line);
                    evidence.push(Evidence { line: r.rule.line, text: r.rule.label() });
                }
            };
            if let Some(si) = sig.strong.iter().position(|re| re.is_match(&r.lower).unwrap_or(false)) {
                hit(&mut strong_hits, si, &mut evidence, &mut seen_lines);
                continue;
            }
            if let Some(wi) = sig.weak.iter().position(|re| re.is_match(&r.lower).unwrap_or(false)) {
                hit(&mut weak_hits, wi, &mut evidence, &mut seen_lines);
            }
        }
        for (line, lower, text) in &comments {
            if sig.comments.iter().any(|re| re.is_match(lower).unwrap_or(false)) {
                comment_hits += 1;
                evidence.push(Evidence { line: *line, text: format!("# {text}") });
            }
        }
        let strong = strong_hits.len() as u32;
        let weak = weak_hits.len() as u32;
        let score = strong * 2 + weak + comment_hits * 3;
        if score == 0 || (strong == 0 && comment_hits == 0 && weak < 2) {
            continue;
        }
        let weak_only = strong == 0 && comment_hits == 0;
        let confidence = if weak_only {
            "low"
        } else if score >= 4 {
            "high"
        } else if score >= 2 {
            "medium"
        } else {
            "low"
        };
        detections.push(Detection { name: sig.name.clone(), kind: sig.kind.clone(), confidence: confidence.into(), score, evidence });
    }
    detections.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
    let platform_kinds = &engine.config.recon.cms.platform_kinds;
    let primary = detections.iter().find(|d| platform_kinds.contains(&d.kind) && d.confidence != "low").map(|d| Primary { name: d.name.clone(), kind: d.kind.clone(), confidence: d.confidence.clone() });
    Stack { primary, detections }
}
