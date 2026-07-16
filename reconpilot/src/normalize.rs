use std::{collections::BTreeSet, path::Path};

use anyhow::Result;
use url::Url;

use crate::{models::UrlRecord, utils};

pub fn normalize_urls_from_file(path: &Path) -> Result<Vec<UrlRecord>> {
    // TODO: Preserve source metadata and emit JSONL instead of returning only in-memory records.
    let lines = utils::read_trimmed_lines(path)?;
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();

    for line in lines {
        if let Some(record) = normalize_url_record(&line) {
            if seen.insert(record.normalized_url.clone()) {
                records.push(record);
            }
        }
    }

    Ok(records)
}

pub fn normalize_url_record(raw: &str) -> Option<UrlRecord> {
    // TODO: Add query key sorting and path bucketing once the schema is finalized.
    let mut parsed = Url::parse(raw).ok()?;
    parsed.set_fragment(None);

    let scheme = parsed.scheme().to_ascii_lowercase();
    let _ = parsed.set_scheme(&scheme);

    if let Some(host) = parsed.host_str() {
        let _ = parsed.set_host(Some(&host.to_ascii_lowercase()));
    }

    if (scheme == "http" && parsed.port() == Some(80))
        || (scheme == "https" && parsed.port() == Some(443))
    {
        let _ = parsed.set_port(None);
    }

    let path = if parsed.path().is_empty() {
        "/".to_string()
    } else {
        parsed.path().to_string()
    };

    let parameters = parsed
        .query_pairs()
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();

    Some(UrlRecord {
        url: raw.to_string(),
        normalized_url: parsed.to_string(),
        source: "native-normalizer".to_string(),
        host: parsed.host_str().map(ToOwned::to_owned),
        path,
        parameters,
        tags: vec!["normalized".to_string()],
    })
}
