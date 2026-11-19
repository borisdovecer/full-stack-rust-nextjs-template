//! The system docs (`docs/system/*.md`) served to the Help page.
//!
//! `docs/system/README.md` is the index: each `- [Title](file.md)` line is one page, in order.
//! Only files the index lists are served, by their file stem (`architecture.md` is
//! `architecture`). Release builds carry the folder inside the binary, so a server shows the
//! docs of the commit it was built from; development reads the folder, so edits show at once.

use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};

use app_core::config::{AppEnv, HelpSource};

use super::dto::HelpEntry;

static EMBEDDED: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../docs/system");
const SOURCE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/system");
const INDEX: &str = "README.md";

#[derive(Debug, Clone)]
pub enum HelpDocs {
    Disk(PathBuf),
    Embedded,
}

impl HelpDocs {
    pub fn new(source: HelpSource, env: AppEnv) -> Self {
        let disk = Path::new(SOURCE_DIR);
        match source {
            HelpSource::Embedded => HelpDocs::Embedded,
            HelpSource::Disk => HelpDocs::Disk(disk.to_path_buf()),
            HelpSource::Auto if !env.is_production() && disk.is_dir() => {
                HelpDocs::Disk(disk.to_path_buf())
            }
            HelpSource::Auto => HelpDocs::Embedded,
        }
    }

    fn read(&self, file: &str) -> Option<String> {
        match self {
            HelpDocs::Disk(dir) => std::fs::read_to_string(dir.join(file)).ok(),
            HelpDocs::Embedded => EMBEDDED
                .get_file(file)
                .and_then(|f| f.contents_utf8())
                .map(str::to_string),
        }
    }

    pub fn index(&self) -> Vec<HelpEntry> {
        self.read(INDEX)
            .map(|t| parse_index(&t))
            .unwrap_or_default()
    }

    /// The markdown for a slug the index lists; `None` for anything else.
    pub fn page(&self, slug: &str) -> Option<String> {
        if !is_slug(slug) || !self.index().iter().any(|e| e.slug == slug) {
            return None;
        }
        self.read(&format!("{slug}.md"))
    }
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Reads `- [Title](file.md)` lines. Anything else in the index is prose and is skipped.
fn parse_index(text: &str) -> Vec<HelpEntry> {
    let mut out: Vec<HelpEntry> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("- [") else {
            continue;
        };
        let Some((title, rest)) = rest.split_once("](") else {
            continue;
        };
        let Some((file, _)) = rest.split_once(')') else {
            continue;
        };
        let Some(slug) = file.strip_suffix(".md") else {
            continue;
        };
        if is_slug(slug) && !out.iter().any(|e| e.slug == slug) {
            out.push(HelpEntry {
                slug: slug.to_string(),
                title: title.trim().to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_lines_become_entries_in_order() {
        let text = "# Docs\n\nIntro text.\n\n- [Architecture](architecture.md) - how it fits\n\
                    - [Auth](auth.md)\n- [Outside](../secret.md)\n- [Nested](a/b.md)\n\
                    - [Upper](Upper.md)\n- [Dup](auth.md)\n- [Link](https://example.com)\n";
        let got = parse_index(text);
        assert_eq!(
            got,
            vec![
                HelpEntry {
                    slug: "architecture".into(),
                    title: "Architecture".into()
                },
                HelpEntry {
                    slug: "auth".into(),
                    title: "Auth".into()
                },
            ]
        );
    }

    #[test]
    fn embedded_and_disk_serve_the_same_index() {
        let disk = HelpDocs::Disk(PathBuf::from(SOURCE_DIR));
        assert!(!HelpDocs::Embedded.index().is_empty());
        assert_eq!(disk.index(), HelpDocs::Embedded.index());
    }

    #[test]
    fn only_indexed_slugs_are_served() {
        let docs = HelpDocs::Embedded;
        let first = docs.index().remove(0);
        assert!(docs.page(&first.slug).is_some());
        assert!(docs.page("README").is_none());
        assert!(docs.page("../../Cargo").is_none());
        assert!(docs.page("not-listed").is_none());
    }
}
