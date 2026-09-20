mod common;
use common::*;
use susbot_core::recon::*;
use url::Url;

#[test]
fn cms_detection() {
    let e = engine();
    let s = cms::detect_stack(&rules(&["Disallow: /checkouts/", "Disallow: /carts/", "Disallow: /account/", "Disallow: /recommendations/products", "Disallow: /collections/*sort_by*"]), &e);
    assert_eq!((s.primary.as_ref().unwrap().name.as_str(), s.primary.as_ref().unwrap().confidence.as_str()), ("Shopify", "high"));
    assert!(s.detections.iter().all(|d| d.name != "WordPress"));
    let s = cms::detect_stack(&rules(&["Disallow: /wp-admin/", "Disallow: /wp-includes/", "Disallow: /xmlrpc.php", "Disallow: /wp-json/", "Disallow: /*add-to-cart=*", "Disallow: /my-account/", "Disallow: /_next/"]), &e);
    let names: Vec<&str> = s.detections.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(s.primary.as_ref().unwrap().name, "WordPress");
    assert!(names.contains(&"WooCommerce") && names.contains(&"Next.js"));
    assert_eq!(s.detections.iter().find(|d| d.name == "Next.js").unwrap().kind, "framework");
    assert_eq!(cms::detect_stack(&rules(&["Disallow: /catalogsearch/", "Disallow: /checkout/cart/", "Disallow: /customer/account/"]), &e).primary.unwrap().name, "Magento / Adobe Commerce");
    assert_eq!(cms::detect_stack(&rules(&["Disallow: /core/", "Disallow: /user/register/", "Disallow: /admin/"]), &e).primary.unwrap().name, "Drupal");
    let back: Vec<String> = cms::detect_stack(&rules(&["Disallow: /telescope/", "Disallow: /horizon/", "Disallow: /static/admin/"]), &e).detections.into_iter().map(|d| d.name).collect();
    assert!(back.contains(&"Laravel".to_string()) && back.contains(&"Django".to_string()));
    assert_eq!(cms::detect_stack(&model("# Squarespace Robots Txt\nUser-agent: *\nDisallow: /config"), &e).primary.unwrap().name, "Squarespace");
    assert!(cms::detect_stack(&rules(&["Disallow: /cart"]), &e).detections.is_empty());
    assert!(cms::detect_stack(&rules(&["Disallow: /admin/"]), &e).primary.is_none());
    assert!(cms::detect_stack(&rules(&["Disallow: /admin/", "Disallow: /Admin/"]), &e).detections.iter().all(|d| d.name != "Craft CMS"));
    assert_eq!(cms::detect_stack(&rules(&["Disallow: /admin/", "Disallow: /modules/"]), &e).detections.iter().find(|d| d.name == "Drupal").unwrap().confidence, "low");
    let d = &cms::detect_stack(&rules(&["Disallow: /wp-admin/", "Disallow: /wp-admin/"]), &e).detections[0];
    assert_eq!(d.evidence.iter().map(|x| x.line).collect::<Vec<_>>(), [2, 3]);
    let disabled = susbot_core::Engine::from_toml(Some("[recon.cms]\ndisabled = [\"WordPress\"]\n")).unwrap();
    assert!(cms::detect_stack(&rules(&["Disallow: /wp-admin/", "Disallow: /wp-json/"]), &disabled).detections.is_empty());
}

#[test]
fn cloud_classification() {
    let e = engine();
    let c = |u: &str| cloud::classify_host(&Url::parse(u).unwrap(), &e);
    let s3 = c("https://my-company-assets.s3.amazonaws.com/sitemaps/sitemap.xml").unwrap();
    assert_eq!((s3.provider.as_str(), s3.kind.as_str(), s3.host.as_str(), s3.bucket.as_deref()), ("AWS S3", "storage", "my-company-assets.s3.amazonaws.com", Some("my-company-assets")));
    assert_eq!(c("https://s3.eu-west-1.amazonaws.com/bucket-b/x.xml").unwrap().bucket.as_deref(), Some("bucket-b"));
    assert_eq!(c("https://d111111abcdef8.cloudfront.net/sitemaps/").unwrap().provider, "Amazon CloudFront");
    assert_eq!(c("https://storage.googleapis.com/company-bucket/sitemap.xml").unwrap().bucket.as_deref(), Some("company-bucket"));
    assert_eq!(c("https://company.blob.core.windows.net/public/sitemap.xml").unwrap().bucket.as_deref(), Some("company / public"));
    assert_eq!(c("https://acme.fra1.digitaloceanspaces.com/x").unwrap().bucket.as_deref(), Some("acme"));
    assert_eq!(c("https://cdn.shopify.com/s/files/1/0001/0002/0003/files/sitemap.xml").unwrap().bucket.as_deref(), Some("0001/0002/0003"));
    assert!(c("https://example.com/sitemap.xml").is_none());
    let m = model("# old copy at https://legacy.s3.amazonaws.com/site/\nUser-agent: *\nDisallow: /a/\nSitemap: https://my-assets.s3.amazonaws.com/a.xml\nSitemap: https://my-assets.s3.amazonaws.com/b.xml\nSitemap: https://cdn.shopify.com/s/files/1/0001/0002/0003/files/sitemap.xml");
    let found = cloud::find_cloud_assets(&m, &e, &en());
    assert_eq!(found.iter().map(|f| (f.provider.as_str(), f.bucket.as_deref().unwrap_or(""), f.source.as_str())).collect::<Vec<_>>(), [("AWS S3", "my-assets", "sitemap"), ("Shopify CDN", "0001/0002/0003", "sitemap"), ("AWS S3", "legacy", "comment")]);
    assert!(found[0].risk.contains("probed"));
}

#[test]
fn leaked_hosts() {
    let e = engine();
    assert_eq!(hosts::environment_tag("staging.example.com", &e).as_deref(), Some("staging"));
    assert_eq!(hosts::environment_tag("dev-portal.example.com", &e).as_deref(), Some("dev-portal"));
    assert_eq!(hosts::environment_tag("www.example.com", &e), None);
    assert_eq!(hosts::environment_tag("api.eu.example.com", &e).as_deref(), Some("api"));
    assert_eq!(hosts::environment_tag("old-site.example.com", &e).as_deref(), Some("old-site"));
    let m = model(&["# migrated from old-site.example.com, ask ops", "User-agent: *", "Disallow: https://beta.example.com/checkout/", "Disallow: /v2-redesign/", "Disallow: /new-site/", "Disallow: /blog/", "Host: dev-portal.example.com", "Sitemap: https://www.example.com/sitemap.xml", "Sitemap: https://staging.example.com/sitemap.xml", "Sitemap: https://cdn-vendor.net/sitemap.xml"].join("\n"));
    let h = hosts::find_leaked_hosts(&m, Some(SITE), &e, &en());
    let by = |name: &str| h.hosts.iter().find(|x| x.host == name).unwrap();
    assert!(!h.hosts.iter().any(|x| x.host == "www.example.com"));
    assert_eq!((by("staging.example.com").relation.as_str(), by("staging.example.com").env.as_deref()), ("subdomain", Some("staging")));
    assert_eq!(by("beta.example.com").sources[0].source, "Disallow");
    assert_eq!(by("dev-portal.example.com").sources[0].source, "Host");
    assert_eq!(by("old-site.example.com").sources[0].source, "comment");
    assert_eq!((by("cdn-vendor.net").relation.as_str(), by("cdn-vendor.net").env.as_deref()), ("external", None));
    assert_eq!(h.paths.iter().map(|p| (p.path.as_str(), p.hint.as_str())).collect::<Vec<_>>(), [("/v2-redesign/", "v2-redesign"), ("/new-site/", "new-site")]);
    let m = model("# ask dave@company.com, docs at https://wiki.corp.example.com/x\nUser-agent: *\nDisallow: /a/");
    let h = hosts::find_leaked_hosts(&m, Some(SITE), &e, &en());
    assert_eq!(h.hosts.iter().map(|x| x.host.as_str()).collect::<Vec<_>>(), ["wiki.corp.example.com"]);
    assert_eq!(h.hosts[0].sources.len(), 1);
    let m = model("User-agent: *\nDisallow: /a/\nSitemap: https://b.s3.amazonaws.com/s.xml\nSitemap: https://cdn-vendor.net/s.xml");
    let r = recon(&m, Some(SITE), &e, &en(), None);
    assert_eq!(r.hosts.hosts.iter().map(|x| x.host.as_str()).collect::<Vec<_>>(), ["cdn-vendor.net"]);
    assert_eq!(r.cloud.len(), 1);
}

#[test]
fn api_endpoints() {
    let e = engine();
    let found = api::find_api_endpoints(&rules(&["Disallow: /api/v1/", "Disallow: /api/internal/", "Disallow: /v1/users/", "Disallow: /swagger/", "Disallow: /swagger-ui.html", "Disallow: /openapi.json", "Disallow: /api-docs/", "Disallow: /graphql", "Disallow: /graphiql", "Disallow: /actuator/", "Disallow: /xmlrpc.php", "Disallow: /blog/"]), &e);
    let kind = |p: &str| found.iter().find(|f| f.path == p).map(|f| f.kind.as_str());
    for (p, k) in [("/graphql", "graphql"), ("/graphiql", "graphql"), ("/swagger/", "docs"), ("/swagger-ui.html", "docs"), ("/openapi.json", "docs"), ("/api-docs/", "docs"), ("/api/v1/", "gateway"), ("/api/internal/", "gateway"), ("/v1/users/", "gateway"), ("/actuator/", "ops"), ("/xmlrpc.php", "rpc")] {
        assert_eq!(kind(p), Some(k), "{p}");
    }
    assert_eq!(kind("/blog/"), None);
    assert!(api::find_api_endpoints(&rules(&["Disallow: /v2/"]), &e).is_empty());
    assert_eq!(found.iter().find(|f| f.path == "/api/v1/").unwrap().version.as_deref(), Some("v1"));
    assert_eq!(found[0].kind, "graphql");
}

#[test]
fn data_feeds() {
    let d = data::find_data_feeds(&rules(&["Disallow: /*.csv$", "Disallow: /feeds/google-merchant.xml", "Disallow: /exports/inventory-daily.json", "Disallow: /partner-discount/", "Disallow: /vip-wholesale/", "Disallow: /influencer-landing/", "Disallow: /*?search=", "Disallow: /catalogsearch/*", "Disallow: /*?sort_by=", "Disallow: /*?utm_source=", "Disallow: /blog/"]), &engine(), &en());
    assert_eq!(d.feeds.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["/*.csv$", "/feeds/google-merchant.xml", "/exports/inventory-daily.json"]);
    assert_eq!(d.feeds[1].kind, "Shopping / merchant feed");
    assert_eq!(d.feeds[2].kind, "Inventory or stock data");
    assert_eq!(d.portals.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["/partner-discount/", "/vip-wholesale/", "/influencer-landing/"]);
    assert_eq!(d.search.paths.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["/*?search=", "/catalogsearch/*"]);
    assert_eq!(d.search.params.iter().map(|p| (p.name.as_str(), p.role.as_str())).collect::<Vec<_>>(), [("search", "search"), ("sort_by", "filter"), ("utm_source", "tracking")]);
}

#[test]
fn file_extensions() {
    let ext = extensions::find_extensions(&rules(&["Disallow: /*.pdf$", "Disallow: /*.sql$", "Disallow: /*.bak$", "Disallow: /*.log$", "Disallow: /*.zip$", "Disallow: /*.php", "Disallow: /docs/manual.pdf", "Disallow: /static/*.js.map", "Disallow: /blog/"]), &engine(), &en());
    let by = |x: &str| ext.iter().find(|e| e.ext == x);
    assert_eq!((by("pdf").unwrap().count, by("pdf").unwrap().risk.as_str()), (2, "medium"));
    assert_eq!(by("sql").unwrap().risk, "high");
    assert_eq!(by("bak").unwrap().category, "backup");
    assert_eq!(by("log").unwrap().category, "logs");
    assert_eq!(by("map").unwrap().category, "source");
    assert_eq!(by("zip").unwrap().category, "archives");
    assert_eq!(by("php").unwrap().tech.as_deref(), Some("PHP"));
    assert_eq!(ext[0].risk, "high");
    assert_eq!(extensions::tech_hints(&ext), ["PHP"]);
    assert!(by("js").is_none());
}

#[test]
fn comment_metadata() {
    let m = model(&["# Updated by dave@company.com on 2024-03-12", "# Maintained by Jane Doe for the SEO team", "# Generated automatically by Akamai Edge / Fastly", "# Temporary fix for TICKET-4921: do not index until campaign launch", "# Unhide after 2099-10-01 launch event", "# origin 10.0.0.12, see https://wiki.corp.example.com/robots", "# encoding UTF-8, see RFC-9309", "User-agent: *", "Disallow: /tmp/ # ticket #1234"].join("\n"));
    let found = comments::mine_comments(&m, &engine(), &en(), Some((2026, 9, 17)));
    let of = |kind: &str| -> Vec<&str> { found.iter().filter(|f| f.kind == kind).map(|f| f.value.as_str()).collect() };
    assert_eq!(of("email"), ["dave@company.com"]);
    assert_eq!(of("person"), ["Jane Doe"]);
    assert_eq!(of("vendor").iter().map(|v| v.to_lowercase()).collect::<Vec<_>>(), ["akamai", "fastly"]);
    assert_eq!(of("ticket"), ["TICKET-4921", "ticket #1234"]);
    assert!(of("date").contains(&"2024-03-12"));
    let future = found.iter().find(|f| f.kind == "date" && f.value == "2099-10-01").unwrap();
    assert!(future.note.as_ref().unwrap().contains("upcoming") && future.note.as_ref().unwrap().contains("years ahead"));
    assert_eq!(of("ip"), ["10.0.0.12"]);
    assert!(!of("date").iter().any(|v| v.contains("0.0.12")));
    assert_eq!(found.iter().find(|f| f.kind == "ip").unwrap().note.as_deref(), Some("private range"));
    assert_eq!(of("url"), ["https://wiki.corp.example.com/robots"]);
    assert!(of("note").contains(&"temporary") && of("note").contains(&"unhide"));
    assert!(!of("ticket").contains(&"UTF-8") && !of("ticket").contains(&"RFC-9309"));
}

#[test]
fn aggregator_and_counts() {
    let e = engine();
    let m = model("# by ops@example.com\nUser-agent: *\nDisallow: /wp-admin/\nDisallow: /wp-json/\nDisallow: /api/v1/\nDisallow: /*.sql$\nDisallow: /partner/\nDisallow: /staging/\nSitemap: https://b.s3.amazonaws.com/s.xml");
    let r = recon(&m, Some(SITE), &e, &en(), None);
    let c = counts(&r);
    assert_eq!(r.stack.primary.unwrap().name, "WordPress");
    assert_eq!((c.cloud, c.api, c.extensions, c.data, c.hosts, c.comments), (1, 1, 1, 1, 1, 1));
    assert!(recon(&model(""), None, &e, &en(), None).stack.detections.is_empty());
    let off = susbot_core::Engine::from_toml(Some("[checks.recon]\ncms = false\ncomments = false\n")).unwrap();
    let r = recon(&m, Some(SITE), &off, &en(), None);
    assert!(r.stack.detections.is_empty() && r.comments.is_empty() && r.cloud.len() == 1);
}

#[test]
fn the_tool_that_wrote_the_file() {
    let e = engine();
    let m = model("# Start Yoast block\nUser-agent: *\nDisallow: /a\n# End Yoast block\n");
    let r = susbot_core::recon::generators::find_generators(&m, &e);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].name, "Yoast SEO");
    assert_eq!(r[0].line, 1);
    // Each tool is named once, however many comments it leaves.
    let shopify = model("# we use Shopify as our ecommerce platform\nUser-agent: *\nDisallow: /cart\n# we use Shopify as our ecommerce platform\n");
    assert_eq!(susbot_core::recon::generators::find_generators(&shopify, &e).len(), 1);
    assert!(susbot_core::recon::generators::find_generators(&model("User-agent: *\nDisallow: /a\n"), &e).is_empty());
}
