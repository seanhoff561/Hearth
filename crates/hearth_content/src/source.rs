//! Finding and parsing content files. A data pack is a directory of namespaces
//! (`<pack>/<namespace>/<domain>/**/*.ron|json`); later packs override earlier ones by id.
//! Files are RON (preferred for hand-authoring) or JSON; unknown fields are reported, never
//! silently ignored.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::diag::Report;
use crate::id::with_namespace;

/// One content file read from disk.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub namespace: String,
    pub text: String,
    /// Index of the pack it came from (later packs override earlier ones).
    pub pack: usize,
}

/// A file holding a list of entries of one domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryFile<T> {
    /// Schema version of the domain this file was written for.
    pub schema: u32,
    pub entries: Vec<T>,
}

fn namespaces(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir()
                && let Some(n) = p.file_name().and_then(|n| n.to_str())
            {
                out.push((n.to_owned(), p));
            }
        }
    }
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if matches!(p.extension().and_then(|e| e.to_str()), Some("ron" | "json")) {
            out.push(p);
        }
    }
}

fn read(path: PathBuf, namespace: &str, pack: usize, report: &mut Report) -> Option<SourceFile> {
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(SourceFile {
            path,
            namespace: namespace.to_owned(),
            text,
            pack,
        }),
        Err(e) => {
            report.error("io", Some(path), None, format!("cannot read file: {e}"));
            None
        }
    }
}

/// The file name (without extension) of a domain's reference table, which is not an entry file.
pub const REFERENCE: &str = "reference";

/// All files of a list domain (`<ns>/<domain>.ron|json` and `<ns>/<domain>/**`), in pack
/// order then path order.
pub fn domain_files(packs: &[PathBuf], domain: &str, report: &mut Report) -> Vec<SourceFile> {
    let mut out = Vec::new();
    for (i, root) in packs.iter().enumerate() {
        for (ns, dir) in namespaces(root) {
            let mut files = Vec::new();
            for ext in ["ron", "json"] {
                let single = dir.join(format!("{domain}.{ext}"));
                if single.is_file() {
                    files.push(single);
                }
            }
            collect(&dir.join(domain), &mut files);
            // A domain's reference table (`materials/reference.ron`) is a singleton of its own.
            files.retain(|p| p.file_stem().and_then(|s| s.to_str()) != Some(REFERENCE));
            out.extend(files.into_iter().filter_map(|p| read(p, &ns, i, report)));
        }
    }
    out
}

/// Files of a singleton domain (`<ns>/<name>.ron|json`), in pack order.
pub fn singleton_files(packs: &[PathBuf], name: &str, report: &mut Report) -> Vec<SourceFile> {
    let mut out = Vec::new();
    for (i, root) in packs.iter().enumerate() {
        for (ns, dir) in namespaces(root) {
            for ext in ["ron", "json"] {
                let p = dir.join(format!("{name}.{ext}"));
                if p.is_file()
                    && let Some(f) = read(p, &ns, i, report)
                {
                    out.push(f);
                }
            }
        }
    }
    out
}

fn ron_options() -> ron::Options {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .with_default_extension(ron::extensions::Extensions::UNWRAP_NEWTYPES)
}

/// Parses a file into `T`, reporting syntax errors with positions and unknown fields as errors.
pub fn parse<T: DeserializeOwned>(file: &SourceFile, report: &mut Report) -> Option<T> {
    let mut unknown: Vec<String> = Vec::new();
    let result = with_namespace(&file.namespace, || -> Result<T, (String, Option<usize>)> {
        if file.path.extension().and_then(|e| e.to_str()) == Some("json") {
            let mut de = serde_json::Deserializer::from_str(&file.text);
            let v: T = serde_ignored::deserialize(&mut de, |p| unknown.push(p.to_string()))
                .map_err(|e| (e.to_string(), Some(e.line())))?;
            de.end().map_err(|e| (e.to_string(), Some(e.line())))?;
            Ok(v)
        } else {
            let mut de = ron::Deserializer::from_str_with_options(&file.text, &ron_options())
                .map_err(|e| (e.code.to_string(), Some(e.span.start.line)))?;
            let v: T = serde_ignored::deserialize(&mut de, |p| unknown.push(p.to_string()))
                .map_err(|e| {
                    let s = de.span_error(e);
                    (s.code.to_string(), Some(s.span.start.line))
                })?;
            de.end().map_err(|e| {
                let s = de.span_error(e);
                (s.code.to_string(), Some(s.span.start.line))
            })?;
            Ok(v)
        }
    });
    for field in &unknown {
        let key = field.rsplit('.').next().unwrap_or(field);
        report.error(
            "unknown-field",
            Some(file.path.clone()),
            find_key_line(&file.text, key),
            format!("unknown field `{field}` (typo, or a field of a newer schema?)"),
        );
    }
    match result {
        Ok(v) => Some(v),
        Err((msg, line)) => {
            report.error("parse", Some(file.path.clone()), line, msg);
            None
        }
    }
}

/// Line (1-based) of the first line mentioning `key` as a field name.
pub fn find_key_line(text: &str, key: &str) -> Option<usize> {
    let ron = format!("{key}:");
    let json = format!("\"{key}\"");
    text.lines()
        .position(|l| {
            let t = l.trim_start();
            t.starts_with(&ron) || t.starts_with(&json)
        })
        .map(|i| i + 1)
}

/// Line (1-based) where the entry with `id` (bare or qualified) is declared.
pub fn find_id_line(text: &str, id: &str) -> Option<usize> {
    let bare = id.split_once(':').map_or(id, |(_, p)| p);
    text.lines()
        .position(|l| {
            let t = l.trim_start();
            (t.starts_with("id:") || t.starts_with("\"id\"") || t.starts_with("(id:"))
                && (t.contains(&format!("\"{bare}\"")) || t.contains(&format!("\"{id}\"")))
        })
        .map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Thing {
        id: String,
        mass_kg: f32,
        #[serde(default)]
        tag: Option<String>,
    }

    fn file(name: &str, text: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(name),
            namespace: "hearth".into(),
            text: text.into(),
            pack: 0,
        }
    }

    #[test]
    fn parses_ron_and_json_and_reports_unknown_fields() {
        let mut r = Report::default();
        let f: EntryFile<Thing> = parse(
            &file("a.ron", "(\n schema: 1,\n entries: [\n  (id: \"x\", mass_kg: 2, tag: \"t\"),\n  (id: \"y\", mass_kg: 1.5, colour: 3),\n ],\n)"),
            &mut r,
        )
        .unwrap();
        assert_eq!(f.entries.len(), 2);
        assert_eq!(f.entries[0].mass_kg, 2.0, "integers read as floats");
        assert_eq!(f.entries[0].tag.as_deref(), Some("t"), "implicit Some");
        assert_eq!(f.entries[1].id, "y");
        assert_eq!(r.errors(), 1, "unknown field reported: {:?}", r.diags);
        let j: EntryFile<Thing> = parse(
            &file(
                "b.json",
                r#"{"schema": 1, "entries": [{"id": "z", "mass_kg": 3}]}"#,
            ),
            &mut r,
        )
        .unwrap();
        assert_eq!(j.entries[0].mass_kg, 3.0);
    }

    #[test]
    fn syntax_errors_have_lines() {
        let mut r = Report::default();
        let bad: Option<EntryFile<Thing>> = parse(
            &file(
                "c.ron",
                "(\n schema: 1,\n entries: [\n  (id: \"x\" mass_kg: 2),\n ],\n)",
            ),
            &mut r,
        );
        assert!(bad.is_none());
        assert_eq!(r.diags[0].line, Some(4), "{:?}", r.diags);
    }

    #[test]
    fn finds_entry_lines() {
        let text = "(\n  entries: [\n    (\n      id: \"flint\",\n    ),\n  ],\n)";
        assert_eq!(find_id_line(text, "hearth:flint"), Some(4));
        assert_eq!(find_key_line(text, "entries"), Some(2));
    }
}
