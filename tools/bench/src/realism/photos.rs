//! Photographs of real places for the realism suite (T §3.2–3.3): openly licensed pictures from
//! Wikimedia Commons matched to each of the suite's biomes and scales, and a contact sheet laying
//! the suite's renders beside them. The pictures stay in `bench-out/realism/refs/photos` (never
//! committed or shipped); their pages, authors and licences in `photos.tsv` there.
//!
//! The fetching is the script's (`scripts/fetch-realism-refs.sh --photos`): `photo-queries`
//! gives each subject's search, the script saves the answers, `photo-pick` chooses the freely
//! licensed pictures and lists them to fetch.

use std::fmt::Write as _;
use std::path::Path;

/// The suite's biomes and what to search Commons for to find real places like them.
pub const SUBJECTS: &[(&str, &str)] = &[
    ("temperate_rainforest", "temperate rainforest"),
    ("broadleaf_forest", "beech forest"),
    ("mixed_forest", "mixed forest"),
    ("boreal_forest", "taiga"),
    ("tundra", "tundra"),
    ("temperate_plains", "prairie"),
    ("steppe", "steppe"),
    ("savanna", "savanna"),
    ("tropical_rainforest", "tropical rainforest"),
    ("hot_desert", "Mojave Desert"),
    ("dune_sea", "erg sand dunes"),
    ("alpine_rock", "scree slope mountain"),
    ("mediterranean_scrub", "garrigue"),
    ("alpine_meadow", "alpine meadow"),
    ("wetland", "marsh"),
    ("beach", "beach"),
    ("stream", "forest stream"),
];

/// The suite's scales and the words that find pictures taken at them.
pub const SCALES: &[(&str, &str)] = &[
    ("feet", "ground"),
    ("walk", ""),
    ("rise", "hillside"),
    ("hill", "panorama"),
];

/// Pictures kept for each subject and scale.
const KEEP: usize = 3;

fn encode(q: &str) -> String {
    q.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b' ' => "%20".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Each subject and scale with the Commons search to run for it, one a line (`key<TAB>url`).
pub fn queries() -> String {
    let mut s = String::new();
    for (key, words) in SUBJECTS {
        for (scale, more) in SCALES {
            let q = format!("{words} {more} filetype:bitmap").replace("  ", " ");
            let _ = writeln!(
                s,
                "{key}_{scale}\thttps://commons.wikimedia.org/w/api.php?action=query&format=json&\
                 formatversion=2&generator=search&gsrnamespace=6&gsrlimit=20&gsrsearch={}&\
                 prop=imageinfo&iiprop=url%7Cextmetadata%7Csize&iiurlwidth=1280",
                encode(q.trim())
            );
        }
    }
    s
}

/// Text without its HTML tags (Commons gives authors as markup).
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A licence that allows comparing and showing the picture with credit: public domain, CC0,
/// CC BY and CC BY-SA.
fn free(licence: &str) -> bool {
    let l = licence.to_ascii_lowercase();
    l.starts_with("cc0")
        || l.starts_with("public domain")
        || l.starts_with("pd")
        || l.starts_with("cc by ")
        || l.starts_with("cc by-sa ")
}

/// From the saved searches, the first freely licensed pictures of each subject wide enough to
/// compare: written to `photos.tsv` and returned as `url<TAB>path` lines for the script.
pub fn pick(refs: &Path) -> anyhow::Result<String> {
    let dir = refs.join("photos");
    let mut tsv = String::from("subject\tfile\tpage\tlicence\tauthor\n");
    let mut fetch = String::new();
    for (key, _) in SUBJECTS {
        for (scale, _) in SCALES {
            let subject = format!("{key}_{scale}");
            let Ok(text) = std::fs::read_to_string(dir.join(&subject).join("search.json")) else {
                continue;
            };
            let v: serde_json::Value = serde_json::from_str(&text)?;
            let mut pages: Vec<&serde_json::Value> = v["query"]["pages"]
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(i64::MAX));
            let mut kept = 0;
            for p in pages {
                let info = &p["imageinfo"][0];
                let meta = &info["extmetadata"];
                let licence = meta["LicenseShortName"]["value"].as_str().unwrap_or("");
                let width = info["width"].as_u64().unwrap_or(0);
                let (Some(thumb), Some(page)) =
                    (info["thumburl"].as_str(), info["descriptionurl"].as_str())
                else {
                    continue;
                };
                if !free(licence) || width < 1200 {
                    continue;
                }
                kept += 1;
                let file = format!("{subject}/{kept}.jpg");
                let author = plain(meta["Artist"]["value"].as_str().unwrap_or("unknown"));
                let _ = writeln!(tsv, "{subject}\t{file}\t{page}\t{licence}\t{author}");
                let _ = writeln!(fetch, "{thumb}\t{}", dir.join(&file).display());
                if kept == KEEP {
                    break;
                }
            }
        }
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("photos.tsv"), tsv)?;
    Ok(fetch)
}

/// The contact sheet: each of the suite's shots beside the photographs of its subject and scale,
/// with their credits, as a page to open in a browser (`bench-out/realism/sheet.html`).
pub fn sheet(out: &Path, refs: &Path) -> anyhow::Result<()> {
    let credits: Vec<Vec<String>> = std::fs::read_to_string(refs.join("photos/photos.tsv"))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect();
    let rel = |p: &Path| {
        pathdiff(p, out)
            .unwrap_or_else(|| p.display().to_string())
            .replace('\\', "/")
    };
    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><title>Realism suite</title><style>body{font:14px \
         sans-serif;margin:16px}img{height:220px;margin:2px}figure{display:inline-block;margin:4px;\
         vertical-align:top}figcaption{max-width:390px;font-size:11px}</style><h1>Realism suite: \
         the generated Earth beside real places</h1>",
    );
    for (key, words) in SUBJECTS {
        let _ = write!(html, "<h2>{key} ({words})</h2>");
        for (scale, _) in SCALES {
            let subject = format!("{key}_{scale}");
            let _ = write!(html, "<div><h3>{scale}</h3>");
            for light in ["day", "low"] {
                let shot = out.join("suite").join(format!("{subject}_{light}.png"));
                if shot.exists() {
                    let _ = write!(
                        html,
                        "<figure><img src=\"{}\"><figcaption>generated, {light}</figcaption></figure>",
                        rel(&shot)
                    );
                }
            }
            for c in credits.iter().filter(|c| c.first() == Some(&subject)) {
                let img = refs.join("photos").join(&c[1]);
                let _ = write!(
                    html,
                    "<figure><img src=\"{}\"><figcaption><a href=\"{}\">photograph</a>, {} ({})\
                     </figcaption></figure>",
                    rel(&img),
                    c[2],
                    c.get(4).map_or("", |s| s.as_str()),
                    c[3]
                );
            }
            html += "</div>";
        }
    }
    std::fs::write(out.join("sheet.html"), html)?;
    println!("contact sheet: {}", out.join("sheet.html").display());
    Ok(())
}

/// `path` relative to the directory `base` (both under the same root).
fn pathdiff(path: &Path, base: &Path) -> Option<String> {
    let p: Vec<_> = path.components().collect();
    let b: Vec<_> = base.components().collect();
    let common = p.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut out = std::path::PathBuf::new();
    for _ in common..b.len() {
        out.push("..");
    }
    for c in &p[common..] {
        out.push(c);
    }
    Some(out.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_free_licences_and_plain_credits() {
        assert!(free("CC BY-SA 4.0"));
        assert!(free("CC0"));
        assert!(free("Public domain"));
        assert!(!free("CC BY-NC 2.0"));
        assert!(!free("CC BY-ND 3.0"));
        assert!(!free("Fair use"));
        assert_eq!(plain("<a href=\"x\">Jane  Doe</a>"), "Jane Doe");
        assert_eq!(
            pathdiff(Path::new("a/b/refs/x.jpg"), Path::new("a/b/out")).as_deref(),
            Some("../refs/x.jpg")
        );
    }
}
