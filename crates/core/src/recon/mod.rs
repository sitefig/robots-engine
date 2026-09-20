//! Reconnaissance: what the file gives away beyond its rules. Each module is
//! a pure function of the model and the compiled config; `recon` runs the
//! enabled ones and returns one object the UI renders as cards.

pub mod api;
pub mod cloud;
pub mod cms;
pub mod comments;
pub mod data;
pub mod extensions;
pub mod generators;
pub mod hosts;

use crate::config::Engine;
use crate::i18n::Locale;
use crate::model::Model;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Recon {
    pub stack: cms::Stack,
    pub generators: Vec<generators::Generator>,
    pub tech: Vec<String>,
    pub cloud: Vec<cloud::CloudAsset>,
    pub hosts: hosts::Hosts,
    pub api: Vec<api::ApiEndpoint>,
    pub data: data::DataFeeds,
    pub extensions: Vec<extensions::Extension>,
    pub comments: Vec<comments::CommentFinding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Counts {
    pub stack: usize,
    pub generators: usize,
    pub cloud: usize,
    pub hosts: usize,
    pub api: usize,
    pub data: usize,
    pub extensions: usize,
    pub comments: usize,
}

impl Counts {
    pub fn total(&self) -> usize {
        self.stack + self.generators + self.cloud + self.hosts + self.api + self.data + self.extensions + self.comments
    }
    pub fn categories(&self) -> usize {
        [self.stack, self.generators, self.cloud, self.hosts, self.api, self.data, self.extensions, self.comments].iter().filter(|c| **c > 0).count()
    }
}

pub fn recon(model: &Model, site_url: Option<&str>, engine: &Engine, locale: &Locale, today: Option<(i32, u32, u32)>) -> Recon {
    let c = &engine.config.checks.recon;
    let extensions = if c.extensions { extensions::find_extensions(model, engine, locale) } else { Vec::new() };
    let mut hosts = if c.hosts { hosts::find_leaked_hosts(model, site_url, engine, locale) } else { hosts::Hosts::default() };
    // Cloud provider hosts belong to the cloud card, not the hosts card.
    hosts.hosts.retain(|h| url::Url::parse(&format!("https://{}/", h.host)).ok().and_then(|u| cloud::classify_host(&u, engine)).is_none());
    Recon {
        stack: if c.cms { cms::detect_stack(model, engine) } else { cms::Stack::default() },
        generators: if c.generators { generators::find_generators(model, engine) } else { Vec::new() },
        cloud: if c.cloud { cloud::find_cloud_assets(model, engine, locale) } else { Vec::new() },
        hosts,
        api: if c.api { api::find_api_endpoints(model, engine) } else { Vec::new() },
        data: if c.data { data::find_data_feeds(model, engine, locale) } else { data::DataFeeds::default() },
        tech: extensions::tech_hints(&extensions),
        extensions,
        comments: if c.comments { comments::mine_comments(model, engine, locale, today) } else { Vec::new() },
    }
}

pub fn counts(r: &Recon) -> Counts {
    Counts {
        stack: r.stack.detections.len(),
        generators: r.generators.len(),
        cloud: r.cloud.len(),
        hosts: r.hosts.hosts.len() + r.hosts.paths.len(),
        api: r.api.len(),
        data: r.data.feeds.len() + r.data.portals.len() + r.data.search.paths.len(),
        extensions: r.extensions.len(),
        comments: r.comments.len(),
    }
}
