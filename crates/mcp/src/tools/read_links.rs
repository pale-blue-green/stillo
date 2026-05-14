use anyhow::Result;
use serde_json::Value;
use stillo_core::{ContentExtractor, ExtractorConfig};
use stillo_fetcher::{HttpConfig, HttpFetcher};

pub async fn run(args: &Value) -> Result<String> {
    let url_str = args["url"].as_str().ok_or_else(|| anyhow::anyhow!("missing 'url'"))?;
    let url: url::Url = url_str.parse()?;

    let fetcher = HttpFetcher::new(HttpConfig::default());
    let extractor = ContentExtractor::new(ExtractorConfig::default());
    let raw = fetcher.fetch(&url).await?;
    let content = extractor.extract(&raw)?;

    let links: Vec<_> = content
        .links
        .iter()
        .map(|l| {
            serde_json::json!({
                "text": l.text,
                "href": l.href.as_str(),
                "rel": l.rel,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&links)?)
}
