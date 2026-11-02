//! `photocraft-cli models list|fetch`: the command-line face of `photocraft_ml::download`
//! (Preferences ▸ AI Models is the other).

use std::io::Write;

use photocraft_ml::{Model, install_dir, model_location};

use crate::Result;
use crate::args::{ModelsAction, ModelsArgs};

/// The models a `--model` value names.
fn chosen(name: &str) -> Vec<Model> {
    match name {
        "birefnet" => vec![Model::BirefnetLite],
        "lama" => vec![Model::Lama],
        // The models PhotoCraft can download (SAM 3 has no public download).
        _ => Model::ALL.into_iter().filter(|m| m.downloadable()).collect(),
    }
}

/// Moves the models of the old `PhotoCraft/models` folder into the shared folder (`photocraft_ml::migrate_old_models`),
/// saying on standard error what moved and what could not. Never fails the command. Skipped in this crate's own
/// tests, which must not touch the real folders.
fn migrate_old_models() {
    if cfg!(test) {
        return;
    }
    let report = photocraft_ml::migrate_old_models();
    for line in report.done {
        eprintln!("photocraft-cli: {line}");
    }
    for line in report.failed {
        eprintln!("photocraft-cli: warning: {line}");
    }
}

pub(crate) fn models(a: &ModelsArgs, out: &mut dyn Write) -> Result {
    migrate_old_models();
    match &a.action {
        ModelsAction::List => {
            for model in Model::ALL {
                // Where it is installed, else where a download would put it.
                let found = model_location(model);
                let path = found.as_ref().map(|l| l.dir.clone()).or_else(|| install_dir(model));
                let path = path
                    .map_or_else(|| format!("{} (models directory unavailable)", model.file_name()), |dir| dir.join(model.file_name()).display().to_string());
                let installed = found.map_or_else(|| "no".to_string(), |l| format!("yes ({})", l.source.label()));
                writeln!(out, "{}\t{}\t{path}\tinstalled: {installed}", model.label(), model.licence())?;
            }
        }
        ModelsAction::Fetch(f) => {
            for model in chosen(&f.model) {
                // Progress goes to stderr in 10% steps, so stdout stays one line per model.
                let shown = std::cell::Cell::new(0u64);
                let progress = |done: u64, total: u64| {
                    let pct = done.saturating_mul(100).checked_div(total).unwrap_or(0);
                    if pct >= shown.get() + 10 || (done == total && shown.get() < 100) {
                        shown.set(pct);
                        eprintln!("{}: {pct}%", model.label());
                    }
                };
                let path = photocraft_ml::download(model, &progress, &|| false).map_err(|e| std::io::Error::other(format!("{}: {e}", model.label())))?;
                writeln!(out, "Installed {} at {}", model.label(), path.display())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::invoke;

    #[test]
    fn model_names_pick_the_models() {
        assert_eq!(chosen("all"), vec![Model::BirefnetLite, Model::Lama], "SAM 3 has no public download");
        assert_eq!(chosen("birefnet"), vec![Model::BirefnetLite]);
        assert_eq!(chosen("lama"), vec![Model::Lama]);
    }

    #[test]
    fn rejects_invalid_arguments_without_network() {
        for words in [
            vec!["models"],
            vec!["models", "unknown"],
            vec!["models", "fetch", "extra"],
            vec!["models", "list", "--model", "all"],
            vec!["models", "fetch", "--model"],
            vec!["models", "fetch", "--model", "other"],
            vec!["models", "fetch", "--unknown"],
            vec!["models", "fetch", "--model", "lama", "--model", "all"],
        ] {
            assert_eq!(invoke(&words).code, 2, "{words:?}");
        }
    }

    #[test]
    fn list_reports_model_metadata_and_loader_status() {
        let run = invoke(&["models", "list"]);
        assert_eq!(run.code, 0, "{}", run.stderr);
        assert_eq!(run.stdout.lines().count(), Model::ALL.len());
        for model in Model::ALL {
            let status = model_location(model).map_or_else(|| "no".to_string(), |l| format!("yes ({})", l.source.label()));
            let line = run.stdout.lines().find(|line| line.contains(model.label())).expect("a line per model");
            assert!(line.contains(model.licence()) && line.contains(model.file_name()));
            assert!(line.ends_with(&format!("installed: {status}")));
        }
    }
}
