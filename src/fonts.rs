//! Web fonts, the way CSS does it: a scene can name any Google Fonts family,
//! and the first time a family isn't installed the server fetches it and
//! caches it in the data dir's `fonts/`, so each family downloads once.
//!
//! The cache keeps a tab of itself in `fonts/index.json`: every font file
//! (named by its SHA-256) with the family it belongs to, where it came from
//! and when. On startup the index restores each file under its family name,
//! which Google's per-weight files don't always carry themselves.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::store::{sha256_hex, write_atomic};
use crate::text;

const CSS_API: &str = "https://fonts.googleapis.com/css2";
const FONT_HOST: &str = "https://fonts.gstatic.com/";
const INDEX: &str = "index.json";

/// The cache index: font file name → what it is.
pub type Index = BTreeMap<String, Entry>;

/// One cached font file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Family the file is registered under.
    pub family: String,
    /// Where it was downloaded from.
    pub url: String,
    /// When it was downloaded, seconds since the Unix epoch.
    pub fetched: u64,
}

/// What `ensure` had to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Already installed or cached; nothing downloaded.
    Available,
    /// Downloaded this many files from Google Fonts.
    Fetched(usize),
}

/// Reads the cache index in `dir`; empty when there is none yet.
pub fn index(dir: &Path) -> Index {
    std::fs::read(dir.join(INDEX))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Makes `family` available for text, fetching it from Google Fonts into
/// `dir` when it isn't installed or cached yet.
///
/// # Errors
/// A malformed family name, a family Google Fonts doesn't have, or a
/// network or disk failure.
pub async fn ensure(family: &str, dir: &Path) -> Result<Outcome> {
    if text::families().iter().any(|f| f == family) {
        return Ok(Outcome::Available);
    }
    // Another server process on the same data dir may have fetched it
    // since this one started: load it from the cache instead.
    let cached: Vec<_> = index(dir)
        .into_iter()
        .filter(|(_, e)| e.family == family)
        .filter_map(|(name, _)| std::fs::read(dir.join(name)).ok())
        .map(|bytes| (bytes, Some(family.to_owned())))
        .collect();
    if !cached.is_empty() {
        text::add_fonts(cached)?;
        return Ok(Outcome::Available);
    }
    if family.is_empty()
        || family.len() > 64
        || !family
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ')
    {
        bail!("bad fontFamily {family:?}: use letters, digits and spaces");
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let urls = font_urls(&http, family).await?;
    std::fs::create_dir_all(dir)?;
    let mut index = index(dir);
    let fetched = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut files = Vec::with_capacity(urls.len());
    for url in urls {
        let bytes = http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec();
        let name = format!("{}.ttf", sha256_hex(&bytes));
        std::fs::write(dir.join(&name), &bytes)?;
        index.insert(
            name,
            Entry {
                family: family.to_owned(),
                url,
                fetched,
            },
        );
        files.push((bytes, Some(family.to_owned())));
    }
    let n = files.len();
    text::add_fonts(files)?;
    write_atomic(&dir.join(INDEX), &serde_json::to_vec_pretty(&index)?)?;
    eprintln!("fetched font {family}: {n} file(s) into {}", dir.display());
    Ok(Outcome::Fetched(n))
}

/// The family's TTF URLs: its weight axis if it has one, else each static
/// weight it has (Google returns the subset of a weight list that exists),
/// else its single style.
async fn font_urls(http: &reqwest::Client, family: &str) -> Result<Vec<String>> {
    let name = family.replace(' ', "+");
    let queries = [
        format!("{name}:wght@100..900"),
        format!("{name}:wght@100;200;300;400;500;600;700;800;900"),
        name.clone(),
    ];
    for query in queries {
        let resp = http.get(format!("{CSS_API}?family={query}")).send().await?;
        if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            continue; // no such axis or weights; try the next form
        }
        let css = resp.error_for_status()?.text().await?;
        let urls = css_font_urls(&css);
        if !urls.is_empty() {
            return Ok(urls);
        }
    }
    Err(anyhow!(
        "font {family} isn't installed and isn't on Google Fonts"
    ))
}

/// `url(...)` sources in a Google Fonts stylesheet, from Google's font host only.
fn css_font_urls(css: &str) -> Vec<String> {
    let mut urls: Vec<String> = css
        .split("url(")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .map(|u| u.trim_matches(['\'', '"']).to_owned())
        .filter(|u| u.starts_with(FONT_HOST))
        .collect();
    urls.dedup();
    urls
}

/// Loads every cached font in `dir` under the family its index records.
/// Files the index doesn't know are left to `text::load_fonts`.
///
/// # Errors
/// When a cached file can't be read or isn't a font.
pub fn load_cache(dir: &Path) -> Result<()> {
    let files = index(dir)
        .into_iter()
        .filter_map(|(name, e)| {
            let path = dir.join(&name);
            path.exists().then_some((path, e.family))
        })
        .map(|(path, family)| {
            std::fs::read(&path)
                .with_context(|| format!("reading {}", path.display()))
                .map(|bytes| (bytes, Some(family)))
        })
        .collect::<Result<Vec<_>>>()?;
    text::add_fonts(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylesheet_urls_come_only_from_googles_font_host() {
        let css = "@font-face { src: url(https://fonts.gstatic.com/s/anton/v27/a.ttf) format('truetype'); }\n\
                   @font-face { src: url('https://fonts.gstatic.com/s/anton/v27/b.ttf'); }\n\
                   @font-face { src: url(https://evil.example/c.ttf); }";
        assert_eq!(
            css_font_urls(css),
            [
                "https://fonts.gstatic.com/s/anton/v27/a.ttf",
                "https://fonts.gstatic.com/s/anton/v27/b.ttf"
            ]
        );
    }

    #[tokio::test]
    async fn installed_families_need_no_download_and_bad_names_are_refused() {
        let dir = std::env::temp_dir().join(format!("keyline-mcp-fonts-{}", std::process::id()));
        assert_eq!(ensure("Inter", &dir).await.unwrap(), Outcome::Available);
        let err = ensure("Evil/../Font", &dir).await.unwrap_err().to_string();
        assert!(err.contains("letters, digits and spaces"), "{err}");
    }

    #[test]
    fn the_index_restores_cached_files_under_their_family() {
        // A cached file whose own name differs from the family it was fetched as.
        let dir = std::env::temp_dir().join(format!("keyline-mcp-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = include_bytes!("../fonts/InterVariable.ttf");
        let name = format!("{}.ttf", sha256_hex(bytes));
        std::fs::write(dir.join(&name), bytes).unwrap();
        let entry = Entry {
            family: "Cached Sans".into(),
            url: "https://fonts.gstatic.com/x.ttf".into(),
            fetched: 1,
        };
        let index = Index::from([(name, entry)]);
        std::fs::write(dir.join(INDEX), serde_json::to_vec(&index).unwrap()).unwrap();

        load_cache(&dir).unwrap();
        assert!(text::families().contains(&"Cached Sans".to_string()));
    }

    #[tokio::test]
    async fn a_family_another_process_cached_loads_without_a_download() {
        // As if a second server on the same data dir fetched it after this
        // one started: in the index, not yet registered here.
        let dir = std::env::temp_dir().join(format!("keyline-mcp-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = include_bytes!("../fonts/InterVariable.ttf");
        let name = format!("{}.ttf", sha256_hex(bytes));
        std::fs::write(dir.join(&name), bytes).unwrap();
        let entry = Entry {
            family: "Shared Sans".into(),
            url: "https://fonts.gstatic.com/y.ttf".into(),
            fetched: 1,
        };
        std::fs::write(
            dir.join(INDEX),
            serde_json::to_vec(&Index::from([(name, entry)])).unwrap(),
        )
        .unwrap();
        assert!(!text::families().contains(&"Shared Sans".to_string()));
        assert_eq!(
            ensure("Shared Sans", &dir).await.unwrap(),
            Outcome::Available
        );
        assert!(text::families().contains(&"Shared Sans".to_string()));
    }

    /// Needs the network: the first request downloads, the second is served
    /// from the cache and the index records every file.
    #[tokio::test]
    #[ignore = "downloads from Google Fonts; run with --ignored"]
    async fn google_fonts_download_once_then_come_from_the_cache() {
        let dir = std::env::temp_dir().join(format!("keyline-mcp-gfonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = ensure("Anton", &dir).await.unwrap();
        assert!(matches!(first, Outcome::Fetched(n) if n >= 1), "{first:?}");
        assert_eq!(ensure("Anton", &dir).await.unwrap(), Outcome::Available);
        // Static families ship one file per weight: all of them, not just 400.
        let lato = ensure("Lato", &dir).await.unwrap();
        assert!(matches!(lato, Outcome::Fetched(n) if n >= 5), "{lato:?}");
        let index = index(&dir);
        assert!(
            !index.is_empty()
                && index
                    .values()
                    .all(|e| e.family == "Anton" || e.family == "Lato")
        );
        for name in index.keys() {
            assert!(dir.join(name).exists(), "{name} missing from the cache");
        }
        let err = ensure("Not A Real Font Family", &dir)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't on Google Fonts"), "{err}");
    }
}
