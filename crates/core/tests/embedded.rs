//! The crate embeds copies of files that live elsewhere in the repository,
//! so it can be published on its own. In the repository, the copies must
//! match the originals; run the command in each message to refresh them.
//! Outside the repository (the published crate) there is nothing to compare.

use std::path::Path;

fn same(copy: &str, original: &str, fix: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let original = root.join(original);
    if !original.exists() {
        return;
    }
    let a = std::fs::read_to_string(root.join(copy)).unwrap();
    let b = std::fs::read_to_string(&original).unwrap();
    assert!(a == b, "{copy} differs from {}: {fix}", original.display());
}

#[test]
fn default_config_matches_the_repository() {
    same("data/default.toml", "../../config/default.toml", "cp config/default.toml crates/core/data/default.toml");
}

#[test]
fn english_dictionary_matches_the_repository() {
    same("data/en.json", "../../locales/en.json", "npm run i18n");
}

#[test]
fn licences_match_the_repository() {
    same("LICENSE.md", "../../LICENSE.md", "cp LICENSE.md crates/core/LICENSE.md");
    same("../cli/LICENSE.md", "../../LICENSE.md", "cp LICENSE.md crates/cli/LICENSE.md");
    same("../susbot/LICENSE.md", "../../LICENSE.md", "cp LICENSE.md crates/susbot/LICENSE.md");
    same("../python/LICENSE.md", "../../LICENSE.md", "cp LICENSE.md crates/python/LICENSE.md");
}
