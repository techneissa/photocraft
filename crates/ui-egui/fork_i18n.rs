//! techneissa's fork (FORK.md, fork/i18n.md): the fork's own interface text. `fork/i18n/<locale>.tsv`
//! holds the rows of fork features; the build writes craft-i18n's `photocraft.tsv` plus the fork rows
//! the official catalog lacks to `OUT_DIR/fork_i18n/<locale>.tsv` and embeds that file instead.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// `app` (craft-i18n's catalog for `code`) with the fork's rows appended, or `app` itself when the
/// fork has none for that locale. Rows the official catalog already defines are left out, so the
/// official translation wins and the catalog reader reports no duplicates.
pub fn overlay(code: &str, app: PathBuf) -> PathBuf {
    let fork = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fork/i18n").join(format!("{code}.tsv"));
    println!("cargo::rerun-if-changed={}", fork.display());
    let (Ok(extra), Ok(official)) = (std::fs::read_to_string(&fork), std::fs::read_to_string(&app)) else { return app };
    let key = |line: &str| {
        let mut cols = line.split('\t');
        Some((cols.next()?.to_string(), cols.next()?.to_string()))
    };
    let known: HashSet<_> = official.lines().filter(|l| !l.starts_with('#')).filter_map(key).collect();
    let mut text = official.trim_end_matches('\n').to_string();
    text.push_str("\n\n");
    for line in extra.lines() {
        if line.starts_with('#') || line.is_empty() || key(line).is_some_and(|k| !known.contains(&k)) {
            text.push_str(line);
            text.push('\n');
        }
    }
    let dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("fork_i18n");
    let out = dir.join(format!("{code}.tsv"));
    match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&out, text)) {
        Ok(()) => out,
        Err(e) => {
            println!("cargo::warning=fork i18n for {code}: {e}; its fork rows are left out");
            app
        }
    }
}
