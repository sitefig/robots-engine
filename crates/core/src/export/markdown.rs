//! Client audit as Markdown, plus a small Markdown-to-HTML renderer for the
//! subset this module emits. Every sentence comes from the dictionary (md.*).

use crate::config::Engine;
use crate::i18n::{kib, Locale};
use crate::model::Level;
use crate::params;
use crate::report::{report_host, Report};
use crate::security::category_info;

fn esc(v: &str) -> String {
    v.replace('|', "\\|").replace("\r\n", " ").replace('\n', " ")
}

fn code(v: &str) -> String {
    format!("`{}`", v.replace('`', "'"))
}

fn table(header: &[String], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = vec![format!("| {} |", header.iter().map(|h| esc(h)).collect::<Vec<_>>().join(" | ")), format!("| {} |", header.iter().map(|_| "---").collect::<Vec<_>>().join(" | "))];
    for r in rows {
        out.push(format!("| {} |", r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")));
    }
    out.join("\n")
}

fn bullets(items: &[String]) -> String {
    items.iter().filter(|i| !i.is_empty()).map(|i| format!("- {i}")).collect::<Vec<_>>().join("\n")
}

struct Md<'a> {
    l: &'a Locale,
    e: &'a Engine,
}

impl<'a> Md<'a> {
    fn plural(&self, key: &str, n: usize) -> String {
        self.l.t(key, &params! {"n" => n})
    }
    fn line_ref(&self, n: Option<u32>) -> String {
        match n {
            Some(n) => self.l.t("md.lineRef", &params! {"n" => n}),
            None => self.l.s("md.file"),
        }
    }
    fn verdict(&self, v: crate::analyser::Verdict) -> String {
        self.l.s(&format!("enum.verdict.{}", v.as_str()))
    }

    fn at_a_glance(&self, r: &Report) -> String {
        let s = &r.summary;
        let src = &r.source;
        let fetched = match src.http_status {
            Some(status) => self.l.t("md.glance.fetched", &params! {"status" => status, "kib" => kib(src.bytes), "via" => if src.fetched_via == "proxy" { self.l.s("md.glance.viaProxy") } else { self.l.s("md.glance.viaDirect") }}),
            None if src.fetched_via == "example" => self.l.s("md.glance.example"),
            None => self.l.s("md.glance.pasted"),
        };
        let policy = if s.default_policy.has_star_group {
            self.l.t("md.glance.policy", &params! {"verdict" => self.verdict(s.default_policy.verdict), "disallow" => s.default_policy.disallow_rules, "allow" => s.default_policy.allow_rules})
        } else {
            self.l.s("md.glance.noStar")
        };
        bullets(&[
            self.l.t("md.glance.source", &params! {"source" => src.final_url.as_ref().map(|u| format!("<{u}>")).unwrap_or_else(|| "robots.txt".into()), "fetched" => fetched}),
            self.l.t("md.glance.rules", &params! {"rules" => s.rules, "groups" => self.plural("md.groups", s.groups), "sitemaps" => self.plural("md.sitemaps", s.sitemaps)}),
            self.l.t("md.glance.default", &params! {"policy" => policy}),
            self.l.t("md.glance.issues", &params! {"errors" => s.issues.errors, "warnings" => s.issues.warnings, "notes" => s.issues.notes}),
            self.l.t("md.glance.security", &params! {"paths" => self.plural("md.sensitivePaths", s.security_findings)}),
            self.l.t("md.glance.ai", &params! {"blocked" => s.ai_training.blocked, "total" => s.ai_training.total}),
            s.platform.as_ref().map(|p| self.l.t("md.glance.platform", &params! {"name" => p})).unwrap_or_default(),
        ])
    }

    fn crawlers(&self, r: &Report) -> String {
        let mut parts = Vec::new();
        let ai: Vec<Vec<String>> = r
            .crawlers
            .iter()
            .filter(|c| c.category.starts_with("ai-"))
            .map(|c| vec![c.name.clone(), c.category_label.clone(), self.verdict(c.verdict), match (&c.group_used, c.own_group) {
                (None, _) => self.l.s("md.none"),
                (Some(g), true) => code(g),
                (Some(_), false) => code("*"),
            }])
            .collect();
        parts.push(table(&[self.l.s("md.table.crawler"), self.l.s("md.table.type"), self.l.s("md.table.access"), self.l.s("md.table.groupUsed")], &ai));
        for cat in self.e.config.crawlers.categories.iter().filter(|c| !self.e.config.crawlers.ai_categories.contains(c)) {
            let list: Vec<_> = r.crawlers.iter().filter(|c| &c.category == cat).collect();
            if list.is_empty() {
                continue;
            }
            let label = &list[0].category_label;
            let blocked: Vec<&str> = list.iter().filter(|c| c.verdict == crate::analyser::Verdict::Blocked).map(|c| c.name.as_str()).collect();
            let restricted: Vec<&str> = list.iter().filter(|c| c.verdict == crate::analyser::Verdict::Partial).map(|c| c.name.as_str()).collect();
            if blocked.is_empty() && restricted.is_empty() {
                parts.push(self.l.t("md.crawlers.allAllowed", &params! {"label" => label}));
            } else {
                let mut bits = Vec::new();
                if !blocked.is_empty() {
                    bits.push(self.l.t("md.crawlers.blocked", &params! {"list" => blocked.join(", ")}));
                }
                if !restricted.is_empty() {
                    bits.push(self.l.t("md.crawlers.restricted", &params! {"list" => restricted.join(", ")}));
                }
                parts.push(self.l.t("md.crawlers.some", &params! {"label" => label, "bits" => bits.join("; ")}));
            }
        }
        parts.join("\n\n")
    }

    fn issues(&self, r: &Report) -> String {
        let actionable: Vec<_> = r.issues.iter().filter(|i| i.level != Level::Info).collect();
        let notes = r.issues.len() - actionable.len();
        if actionable.is_empty() {
            let mut s = self.l.s("md.issues.none");
            if notes > 0 {
                s.push(' ');
                s.push_str(&self.l.t("md.issues.notesInFull", &params! {"notes" => self.plural("md.informationalNotes", notes)}));
            }
            return s;
        }
        let rows: Vec<Vec<String>> = actionable.iter().map(|i| vec![self.line_ref(i.line), self.l.s(&format!("enum.level.{}", i.level.as_str())), self.l.s(&format!("enum.kind.{}", i.kind.as_str())), i.message.clone()]).collect();
        let mut out = vec![table(&[self.l.s("md.table.where"), self.l.s("md.table.level"), self.l.s("md.table.kind"), self.l.s("md.table.issue")], &rows)];
        if notes > 0 {
            out.push(self.l.t("md.issues.plusNotes", &params! {"notes" => self.plural("md.informationalNotes", notes)}));
        }
        out.join("\n\n")
    }

    fn security(&self, r: &Report) -> String {
        if r.security.is_empty() {
            return self.l.s("md.security.none");
        }
        let shown: Vec<_> = r.security.iter().filter(|f| f.severity != "low").collect();
        let low = r.security.len() - shown.len();
        let rows: Vec<Vec<String>> = shown.iter().map(|f| vec![self.line_ref(Some(f.line)), code(&format!("Disallow: {}", f.path)), category_info(self.e, self.l, &f.category).0, self.l.s(&format!("enum.severity.{}", f.severity)), f.reason.clone()]).collect();
        let mut out = vec![self.l.s("md.security.intro"), table(&[self.l.s("md.table.where"), self.l.s("md.table.rule"), self.l.s("md.table.category"), self.l.s("md.table.severity"), self.l.s("md.table.why")], &rows)];
        if low > 0 {
            out.push(self.l.t("md.security.plusLow", &params! {"findings" => self.plural("md.lowFindings", low)}));
        }
        out.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n")
    }

    fn recon(&self, r: &Report) -> String {
        let rec = &r.recon;
        let mut items: Vec<String> = Vec::new();
        let list = |v: Vec<String>| v.join(", ");
        if let Some(p) = &rec.stack.primary {
            items.push(self.l.t("md.recon.platform", &params! {"name" => p.name, "confidence" => self.l.s(&format!("enum.confidence.{}", p.confidence))}));
        }
        let primary_name = rec.stack.primary.as_ref().map(|p| p.name.as_str());
        let others: Vec<String> = rec.stack.detections.iter().filter(|d| Some(d.name.as_str()) != primary_name).map(|d| format!("{} ({})", d.name, self.l.s(&format!("recon.cms.kind.{}", d.kind)))).collect();
        if !others.is_empty() {
            items.push(self.l.t("md.recon.also", &params! {"list" => list(others)}));
        }
        if !rec.tech.is_empty() {
            items.push(self.l.t("md.recon.tech", &params! {"list" => rec.tech.join(", ")}));
        }
        if !rec.cloud.is_empty() {
            items.push(self.l.t("md.recon.cloud", &params! {"list" => list(rec.cloud.iter().map(|c| match &c.bucket { Some(b) => format!("{} ({})", c.provider, code(b)), None => c.provider.clone() }).collect())}));
        }
        let env_hosts: Vec<String> = rec.hosts.hosts.iter().filter(|h| h.env.is_some()).map(|h| code(&h.host)).collect();
        if !env_hosts.is_empty() {
            items.push(self.l.t("md.recon.envHosts", &params! {"list" => list(env_hosts)}));
        }
        let other_hosts: Vec<String> = rec.hosts.hosts.iter().filter(|h| h.env.is_none()).map(|h| code(&h.host)).collect();
        if !other_hosts.is_empty() {
            items.push(self.l.t("md.recon.otherHosts", &params! {"list" => list(other_hosts)}));
        }
        if !rec.hosts.paths.is_empty() {
            items.push(self.l.t("md.recon.envPaths", &params! {"list" => list(rec.hosts.paths.iter().map(|p| code(&p.path)).collect())}));
        }
        if !rec.api.is_empty() {
            items.push(self.l.t("md.recon.api", &params! {"list" => list(rec.api.iter().map(|a| code(&a.path)).collect())}));
        }
        if !rec.data.feeds.is_empty() {
            items.push(self.l.t("md.recon.feeds", &params! {"list" => list(rec.data.feeds.iter().map(|f| code(&f.path)).collect())}));
        }
        if !rec.data.portals.is_empty() {
            items.push(self.l.t("md.recon.portals", &params! {"list" => list(rec.data.portals.iter().map(|f| code(&f.path)).collect())}));
        }
        if !rec.data.search.paths.is_empty() {
            items.push(self.l.t("md.recon.search", &params! {"list" => list(rec.data.search.paths.iter().map(|f| code(&f.path)).collect())}));
        }
        let risky: Vec<String> = rec.extensions.iter().filter(|e| e.risk == "high").map(|e| code(&format!(".{}", e.ext))).collect();
        if !risky.is_empty() {
            items.push(self.l.t("md.recon.riskyExt", &params! {"list" => list(risky)}));
        }
        let of = |kind: &str| -> Vec<String> { rec.comments.iter().filter(|c| c.kind == kind).map(|c| c.value.clone()).collect() };
        let mut bits: Vec<String> = Vec::new();
        for (kind, key) in [("email", "md.comments.emails"), ("person", "md.comments.names"), ("ticket", "md.comments.tickets"), ("ip", "md.comments.ips")] {
            let v = of(kind);
            if !v.is_empty() {
                bits.push(format!("{} ({})", self.plural(key, v.len()), v.join(", ")));
            }
        }
        let dates = of("date");
        if !dates.is_empty() {
            bits.push(self.plural("md.comments.dates", dates.len()));
        }
        let uniq = |v: Vec<String>| {
            let mut out: Vec<String> = Vec::new();
            for x in v {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            out
        };
        let vendors = uniq(of("vendor"));
        if !vendors.is_empty() {
            bits.push(self.l.t("md.comments.vendors", &params! {"list" => vendors.join(", ")}));
        }
        let notes = uniq(of("note"));
        if !notes.is_empty() {
            bits.push(self.l.t("md.comments.notes", &params! {"list" => notes.join(", ")}));
        }
        if !bits.is_empty() {
            items.push(self.l.t("md.recon.comments", &params! {"list" => bits.join("; ")}));
        }
        if items.is_empty() { self.l.s("md.recon.nothing") } else { bullets(&items) }
    }

    fn sitemaps(&self, r: &Report) -> String {
        if r.sitemaps.is_empty() {
            return self.l.s("md.sitemaps.none");
        }
        bullets(&r.sitemaps.iter().map(|s| format!("<{}>{}", s.url, if s.notes.is_empty() { String::new() } else { format!(" — {}", s.notes.join(" ")) })).collect::<Vec<_>>())
    }
}

/// Rule-based action list derived from what was found. Issues are matched by id.
pub fn recommended_actions(r: &Report, engine: &Engine, locale: &Locale) -> Vec<String> {
    let m = Md { l: locale, e: engine };
    let mut acts = Vec::new();
    let by_id = |ids: &[&str]| -> Vec<&crate::model::Warning> { r.issues.iter().filter(|i| ids.contains(&i.id.as_str())).collect() };
    let lines = |list: &[&crate::model::Warning]| -> Vec<u32> {
        let mut v: Vec<u32> = list.iter().filter_map(|i| i.line).collect();
        v.sort_unstable();
        v
    };
    let where_ = |list: &[&crate::model::Warning]| -> String {
        let ls = lines(list);
        if ls.is_empty() {
            return String::new();
        }
        let joined = |n: usize| ls.iter().take(n).map(|l| l.to_string()).collect::<Vec<_>>().join(", ");
        if ls.len() > 6 {
            locale.t("md.whereMore", &params! {"lines" => joined(6)})
        } else {
            locale.t("md.where", &params! {"n" => ls.len(), "lines" => joined(ls.len())})
        }
    };
    if r.source.redirect_limit {
        acts.push(locale.s("md.actions.redirectChain"));
    }
    if r.source.http_status.map(|s| s >= 500).unwrap_or(false) {
        acts.push(locale.s("md.actions.serverError"));
    }
    let errors: Vec<_> = r.issues.iter().filter(|i| i.level == Level::Error).collect();
    if !errors.is_empty() {
        acts.push(locale.t("md.actions.syntaxErrors", &params! {"errors" => m.plural("md.syntaxErrors", errors.len()), "where" => where_(&errors)}));
    }
    let slash = by_id(&["seo.trailingSlash"]);
    if !slash.is_empty() {
        acts.push(locale.t("md.actions.trailingSlash", &params! {"rules" => m.plural("md.prefixRules", slash.len()), "where" => where_(&slash)}));
    }
    let self_ = by_id(&["seo.selfBlock"]);
    if !self_.is_empty() {
        acts.push(locale.t("md.actions.selfBlock", &params! {"where" => where_(&self_)}));
    }
    let dead = by_id(&["seo.redundant", "seo.duplicate", "seo.overridden"]);
    if !dead.is_empty() {
        acts.push(locale.t("md.actions.deadRules", &params! {"rules" => m.plural("md.rules", dead.len()), "where" => where_(&dead)}));
    }
    let abs = by_id(&["lint.absoluteUrl"]);
    if !abs.is_empty() {
        acts.push(locale.t("md.actions.absoluteUrl", &params! {"rules" => m.plural("md.absoluteRules", abs.len()), "where" => where_(&abs)}));
    }
    let noslash = by_id(&["parser.pathNoSlash"]);
    if !noslash.is_empty() {
        acts.push(locale.t("md.actions.noSlash", &params! {"paths" => m.plural("md.paths", noslash.len()), "where" => where_(&noslash)}));
    }
    let proto = by_id(&["sitemap.protocolMismatch"]);
    if !proto.is_empty() {
        acts.push(locale.t("md.actions.protocol", &params! {"where" => where_(&proto)}));
    }
    let cross = by_id(&["sitemap.crossDomain"]);
    if !cross.is_empty() {
        acts.push(locale.t("md.actions.crossDomain", &params! {"sitemaps" => m.plural("md.sitemaps", cross.len()), "where" => where_(&cross)}));
    }
    let relative = by_id(&["parser.sitemapInvalid"]);
    if !relative.is_empty() {
        acts.push(locale.t("md.actions.relativeSitemap", &params! {"lines" => m.plural("md.sitemapLines", relative.len()), "where" => where_(&relative)}));
    }
    if r.sitemaps.is_empty() && !r.rules.is_empty() {
        acts.push(locale.s("md.actions.addSitemap"));
    }
    if !by_id(&["parser.noindex"]).is_empty() {
        acts.push(locale.s("md.actions.noindex"));
    }
    if !by_id(&["parser.crawlDelayNonStandard"]).is_empty() {
        acts.push(locale.s("md.actions.crawlDelay"));
    }
    if r.summary.default_policy.has_star_group && r.summary.default_policy.verdict == crate::analyser::Verdict::Blocked {
        acts.push(locale.s("md.actions.starBlocked"));
    }
    let mut cats: Vec<&str> = Vec::new();
    for f in r.security.iter().filter(|f| f.severity != "low") {
        if !cats.contains(&f.category.as_str()) {
            cats.push(&f.category);
        }
    }
    for c in cats {
        let paths: Vec<&str> = r.security.iter().filter(|f| f.category == c).map(|f| f.path.as_str()).collect();
        let (label, advice) = category_info(engine, locale, c);
        let more = if paths.len() > 5 { locale.t("md.actions.more", &params! {"n" => paths.len() - 5}) } else { String::new() };
        acts.push(locale.t("md.actions.security", &params! {"label" => label, "advice" => advice, "paths" => paths.iter().take(5).map(|p| code(p)).collect::<Vec<_>>().join(", "), "more" => more}));
    }
    if r.ai_status.callout.state == "warning" {
        let open: Vec<&str> = r.ai_status.groups.first().map(|g| g.crawlers.iter().filter(|c| c.verdict != crate::analyser::Verdict::Blocked).map(|c| c.name.as_str()).collect()).unwrap_or_default();
        acts.push(locale.t("md.actions.aiUneven", &params! {"list" => open.join(", ")}));
    } else if r.ai_status.callout.state == "info" && r.summary.ai_training.total > 0 {
        acts.push(locale.s("md.actions.aiDecide"));
    }
    let buckets: Vec<String> = r.recon.cloud.iter().filter(|c| c.kind == "storage").map(|c| code(c.bucket.as_deref().unwrap_or(&c.host))).collect();
    if !buckets.is_empty() {
        acts.push(locale.t("md.actions.buckets", &params! {"list" => buckets.join(", ")}));
    }
    let env_hosts: Vec<String> = r.recon.hosts.hosts.iter().filter(|h| h.env.is_some()).map(|h| code(&h.host)).collect();
    if !env_hosts.is_empty() {
        acts.push(locale.t("md.actions.envHosts", &params! {"list" => env_hosts.join(", ")}));
    }
    if r.recon.comments.iter().any(|c| ["email", "person", "ip", "ticket"].contains(&c.kind.as_str())) {
        acts.push(locale.s("md.actions.personal"));
    }
    if !by_id(&["parser.bom"]).is_empty() {
        acts.push(locale.s("md.actions.bom"));
    }
    if acts.is_empty() {
        acts.push(locale.s("md.actions.none"));
    }
    acts
}

pub fn audit_markdown(r: &Report, engine: &Engine, locale: &Locale) -> String {
    let m = Md { l: locale, e: engine };
    let host = report_host(r).unwrap_or_else(|| locale.s("md.pasted"));
    let date: String = r.generated_at.chars().take(10).collect();
    let actions = recommended_actions(r, engine, locale).iter().enumerate().map(|(i, a)| format!("{}. {a}", i + 1)).collect::<Vec<_>>().join("\n");
    let sections = [
        format!("# {}", locale.t("md.title", &params! {"host" => host})),
        format!("_{}_", locale.t("md.byline", &params! {"date" => date, "tool" => r.tool.name, "vendor" => r.tool.vendor, "url" => r.tool.url})),
        format!("## {}", locale.s("md.h.glance")),
        m.at_a_glance(r),
        format!("## {}", locale.s("md.h.actions")),
        actions,
        format!("## {}", locale.s("md.h.crawlers")),
        m.crawlers(r),
        format!("## {}", locale.s("md.h.issues")),
        m.issues(r),
        format!("## {}", locale.s("md.h.security")),
        m.security(r),
        format!("## {}", locale.s("md.h.recon")),
        m.recon(r),
        format!("## {}", locale.s("md.h.sitemaps")),
        m.sitemaps(r),
    ];
    sections.join("\n\n") + "\n"
}

// ---------------------------------------------------------------- Markdown -> HTML

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn inline(text: &str) -> String {
    use fancy_regex::Regex;
    use std::sync::LazyLock;
    static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());
    static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*]+)\*\*").unwrap());
    static EM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|[\s(])_([^_]+)_(?=[\s.,;:)]|$)").unwrap());
    static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)\s]+)\)").unwrap());
    static AUTOLINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&lt;(https?://[^&\s]+)&gt;").unwrap());
    let mut s = escape_html(text);
    s = CODE.replace_all(&s, "<code>$1</code>").into_owned();
    s = BOLD.replace_all(&s, "<strong>$1</strong>").into_owned();
    s = EM.replace_all(&s, "$1<em>$2</em>").into_owned();
    s = LINK.replace_all(&s, "<a href=\"$2\">$1</a>").into_owned();
    s = AUTOLINK.replace_all(&s, "<a href=\"$1\">$1</a>").into_owned();
    s
}

fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cur.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn is_block_start(line: &str) -> bool {
    line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") || line.starts_with('|') || line.starts_with("- ") || ordered(line)
}

fn ordered(line: &str) -> bool {
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    !digits.is_empty() && line[digits.len()..].starts_with(". ")
}

/// Renders the Markdown subset produced by `audit_markdown`.
pub fn markdown_to_html(md: &str) -> String {
    let lines: Vec<&str> = md.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if (1..=3).contains(&hashes) && line[hashes..].starts_with(' ') {
            out.push(format!("<h{hashes}>{}</h{hashes}>", inline(line[hashes + 1..].trim())));
            i += 1;
            continue;
        }
        if line.starts_with('|') {
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].starts_with('|') {
                rows.push(split_row(lines[i]));
                i += 1;
            }
            let header = &rows[0];
            let body = if rows.len() > 2 { &rows[2..] } else { &[][..] };
            out.push(format!(
                "<table><thead><tr>{}</tr></thead><tbody>{}</tbody></table>",
                header.iter().map(|c| format!("<th>{}</th>", inline(c))).collect::<String>(),
                body.iter().map(|r| format!("<tr>{}</tr>", r.iter().map(|c| format!("<td>{}</td>", inline(c))).collect::<String>())).collect::<String>()
            ));
            continue;
        }
        if line.starts_with("- ") {
            let mut items = Vec::new();
            while i < lines.len() && lines[i].starts_with("- ") {
                items.push(inline(&lines[i][2..]));
                i += 1;
            }
            out.push(format!("<ul>{}</ul>", items.iter().map(|t| format!("<li>{t}</li>")).collect::<String>()));
            continue;
        }
        if ordered(line) {
            let mut items = Vec::new();
            while i < lines.len() && ordered(lines[i]) {
                let l = lines[i];
                let start = l.find(". ").unwrap() + 2;
                items.push(inline(&l[start..]));
                i += 1;
            }
            out.push(format!("<ol>{}</ol>", items.iter().map(|t| format!("<li>{t}</li>")).collect::<String>()));
            continue;
        }
        let mut para = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() && !is_block_start(lines[i]) {
            para.push(lines[i]);
            i += 1;
        }
        out.push(format!("<p>{}</p>", inline(&para.join(" "))));
    }
    out.join("\n")
}

const DOC_CSS: &str = "
body{font:16px/1.5 system-ui,sans-serif;max-width:52rem;margin:2rem auto;padding:0 1rem;color:#1b1b19;background:#fff}
h1{font-size:1.8rem}h2{font-size:1.3rem;margin-top:2rem;border-bottom:1px solid #ddd;padding-bottom:.25rem}
table{border-collapse:collapse;width:100%;font-size:.9rem;margin:1rem 0}th,td{border:1px solid #ddd;padding:.35rem .5rem;text-align:left;vertical-align:top}
th{background:#f3f3f0}code{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:.9em;background:#f3f3f0;padding:.05em .3em;border-radius:3px}
a{color:#1f5f8b}li{margin:.25rem 0}
";

/// A standalone HTML document of the audit.
pub fn audit_html(r: &Report, engine: &Engine, locale: &Locale) -> String {
    let host = report_host(r).unwrap_or_else(|| locale.s("md.pasted"));
    format!(
        "<!doctype html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{}</style>\n</head>\n<body>\n{}\n</body>\n</html>\n",
        escape_html(locale.lang()),
        escape_html(&locale.t("md.title", &params! {"host" => host})),
        DOC_CSS,
        markdown_to_html(&audit_markdown(r, engine, locale))
    )
}
