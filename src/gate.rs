//! The checks between build and publish.
//!
//! An exit code of 0 is not evidence that a build is good. The realistic failure
//! is a sync error that leaves the vault half-empty on the server: the generator
//! builds it happily and a three-page website replaces the blog.

use std::path::Path;

use crate::config::Gate;
use crate::fsutil;

#[derive(Debug, Default)]
pub struct Verdict {
    pub violations: Vec<String>,
    pub warnings: Vec<String>,
    pub pages: usize,
}

impl Verdict {
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }
}

/// `prev_pages` is the page count of the currently published release, if any.
pub fn evaluate(
    gate: &Gate,
    out_dir: &Path,
    content_dir: &Path,
    prev_pages: Option<usize>,
) -> Verdict {
    let mut verdict = Verdict {
        pages: fsutil::count_files_with_extension(out_dir, "html"),
        ..Default::default()
    };

    for required in &gate.require_files {
        if !out_dir.join(required).exists() {
            verdict
                .violations
                .push(format!("required file missing: {required}"));
        }
    }

    if verdict.pages < gate.min_pages {
        verdict.violations.push(format!(
            "page count {} below minimum {}",
            verdict.pages, gate.min_pages
        ));
    }

    if let Some(prev) = prev_pages {
        if prev > 0 && verdict.pages < prev {
            let drop = (prev - verdict.pages) as f64 / prev as f64;
            if drop > gate.max_page_drop {
                verdict.violations.push(format!(
                    "page count dropped {:.0}% ({prev} → {}), limit is {:.0}%",
                    drop * 100.0,
                    verdict.pages,
                    gate.max_page_drop * 100.0
                ));
            }
        }
    }

    let duplicates = fsutil::duplicate_basenames(content_dir, "md");
    if !duplicates.is_empty() {
        let message = format!(
            "duplicate basenames break wikilink resolution: {}",
            duplicates.join(", ")
        );
        if gate.forbid_duplicate_basenames {
            verdict.violations.push(message);
        } else {
            verdict.warnings.push(message);
        }
    }

    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(pages: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..pages {
            std::fs::write(dir.path().join(format!("p{i}.html")), "<html></html>").unwrap();
        }
        dir
    }

    #[test]
    fn a_half_synced_vault_does_not_reach_the_live_site() {
        let out = site(3);
        let content = tempfile::tempdir().unwrap();
        let gate = Gate {
            max_page_drop: 0.4,
            ..Default::default()
        };

        let verdict = evaluate(&gate, out.path(), content.path(), Some(46));
        assert!(!verdict.passed());
        assert!(
            verdict.violations[0].contains("dropped"),
            "{:?}",
            verdict.violations
        );
    }

    #[test]
    fn a_normal_edit_passes() {
        let out = site(46);
        let content = tempfile::tempdir().unwrap();
        let gate = Gate {
            min_pages: 5,
            ..Default::default()
        };
        assert!(evaluate(&gate, out.path(), content.path(), Some(45)).passed());
    }

    #[test]
    fn growth_is_never_a_violation() {
        let out = site(90);
        let content = tempfile::tempdir().unwrap();
        assert!(evaluate(&Gate::default(), out.path(), content.path(), Some(46)).passed());
    }

    #[test]
    fn required_files_must_exist() {
        let out = site(10);
        let content = tempfile::tempdir().unwrap();
        let gate = Gate {
            require_files: vec!["sitemap.xml".into()],
            ..Default::default()
        };
        let verdict = evaluate(&gate, out.path(), content.path(), None);
        assert!(!verdict.passed());
        assert!(verdict.violations[0].contains("sitemap.xml"));
    }

    #[test]
    fn the_first_build_has_nothing_to_compare_against() {
        let out = site(1);
        let content = tempfile::tempdir().unwrap();
        assert!(evaluate(&Gate::default(), out.path(), content.path(), None).passed());
    }
}
