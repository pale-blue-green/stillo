use anyhow::Result;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde_json::Value;
use std::collections::HashMap;
use stillo_fetcher::{HttpConfig, HttpFetcher};
use url::Url;

pub async fn run(args: &Value) -> Result<String> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'query'"))?;
    let format = args["format"].as_str().unwrap_or("markdown");

    let mut search_url = Url::parse("https://html.duckduckgo.com/html/").unwrap();
    search_url.query_pairs_mut().append_pair("q", query.trim());

    let fetcher = HttpFetcher::new(HttpConfig::default());
    let raw = fetcher.fetch(&search_url).await?;
    let html = String::from_utf8_lossy(&raw.bytes);

    let results = parse_ddg_html(&html);

    match format {
        "links" => {
            let json: Vec<_> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "title": r.title,
                        "url": r.url.as_str(),
                        "snippet": r.snippet,
                        "display_url": r.display_url,
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&json)?)
        }
        _ => {
            let mut md = format!("# Search: {}\n\n", query);
            for (i, r) in results.iter().enumerate() {
                md.push_str(&format!("## {}. {}\n", i + 1, r.title));
                md.push_str(&format!("*{}*\n\n", r.display_url));
                if !r.snippet.is_empty() {
                    md.push_str(&format!("{}\n\n", r.snippet));
                }
                md.push_str(&format!("<{}>\n\n---\n\n", r.url));
            }
            Ok(md)
        }
    }
}

struct SearchResult {
    title: String,
    url: Url,
    snippet: String,
    display_url: String,
}

/// DDG HTML から検索結果を抽出する専用パーサー。
/// result__a / result__url / result__snippet の各 <a> を uddg= キーでグループ化し、
/// スニペット付きの構造化データを生成する。
fn parse_ddg_html(html: &str) -> Vec<SearchResult> {
    let dom = match parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
    {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    // (class, href, text) の形式で全対象リンクを収集
    let mut raw: Vec<(String, String, String)> = Vec::new();
    collect_result_links(&dom.document, &mut raw);

    // DDGリダイレクトURLの uddg= パラメータをキーにグループ化
    // order は挿入順を保持するためのキーリスト
    let mut order: Vec<String> = Vec::new();
    #[derive(Default)]
    struct Group {
        title: Option<String>,
        display_url: Option<String>,
        snippet: Option<String>,
        real_url: Option<Url>,
    }
    let mut groups: HashMap<String, Group> = HashMap::new();

    for (class, href, text) in raw {
        let normalized = if href.starts_with("//") {
            format!("https:{}", href)
        } else {
            href.clone()
        };

        let parsed = match Url::parse(&normalized) {
            Ok(u) => u,
            Err(_) => continue,
        };

        if parsed.host_str() != Some("duckduckgo.com") {
            continue;
        }

        let uddg = match parsed
            .query_pairs()
            .find(|(k, _)| k == "uddg")
            .map(|(_, v)| v.into_owned())
        {
            Some(u) => u,
            None => continue,
        };

        let real_url = uddg.parse::<Url>().ok();

        if !groups.contains_key(&uddg) {
            order.push(uddg.clone());
            groups.insert(uddg.clone(), Group::default());
        }

        let g = groups.get_mut(&uddg).unwrap();
        if real_url.is_some() {
            g.real_url = real_url;
        }

        match class.as_str() {
            "result__a" => g.title = Some(text.trim().to_owned()),
            "result__url" => g.display_url = Some(text.trim().to_owned()),
            "result__snippet" => g.snippet = Some(text.trim().to_owned()),
            _ => {}
        }
    }

    order
        .into_iter()
        .filter_map(|key| {
            let g = groups.remove(&key)?;
            Some(SearchResult {
                title: g.title?,
                url: g.real_url?,
                snippet: g.snippet.unwrap_or_default(),
                display_url: g.display_url.unwrap_or_default(),
            })
        })
        .collect()
}

/// DOM を再帰走査して result__a / result__url / result__snippet の <a> を収集する。
fn collect_result_links(handle: &Handle, out: &mut Vec<(String, String, String)>) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        if name.local.as_ref() == "a" {
            let attrs_ref = attrs.borrow();
            let class = attrs_ref
                .iter()
                .find(|a| a.name.local.as_ref() == "class")
                .map(|a| a.value.as_ref().to_owned())
                .unwrap_or_default();

            if matches!(
                class.as_str(),
                "result__a" | "result__url" | "result__snippet"
            ) {
                let href = attrs_ref
                    .iter()
                    .find(|a| a.name.local.as_ref() == "href")
                    .map(|a| a.value.as_ref().to_owned())
                    .unwrap_or_default();

                let mut text = String::new();
                collect_text(handle, &mut text);
                out.push((class, href, text));
                return; // <a> の子孫は再帰しない
            }
        }
    }

    for child in handle.children.borrow().iter() {
        collect_result_links(child, out);
    }
}

/// テキストノードを再帰的に収集してプレーンテキストを生成する。
fn collect_text(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            out.push_str(contents.borrow().as_ref());
        }
        _ => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
    }
}
