#![allow(dead_code)]
use susbot_core::config::Engine;
use susbot_core::i18n::Locale;
use susbot_core::model::Model;
use susbot_core::parser::parse;

pub fn en() -> Locale {
    Locale::english()
}

pub fn engine() -> Engine {
    Engine::default_engine()
}

pub fn model(text: &str) -> Model {
    parse(text, &en())
}

/// A model with a `User-agent: *` group and the given rule lines.
pub fn rules(lines: &[&str]) -> Model {
    model(&format!("User-agent: *\n{}", lines.join("\n")))
}

pub fn tokens(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

pub const SITE: &str = "https://www.example.com/robots.txt";
pub const KITCHEN_SINK: &str = include_str!("../../../../examples/kitchen-sink.robots.txt");
