# stillo 実装仕様書

**バージョン**: 0.1.0-draft  
**言語**: Rust (edition 2021)  
**策定日**: 2026-05-14

---

## 1. プロジェクト概要

### 1.1 コンセプト

stillo は「意味抽出ブラウザ」として、以下の2軸を統合するCLIツールである。

- **軸1**: w3m互換のターミナルブラウジング体験
- **軸2**: LLMパイプラインのフロントエンド・MCPサーバー

JavaScript非対応を欠点として補うのではなく、「JSノイズを排除して構造化コンテンツを抽出できる」という差別化として位置づける。

### 1.2 設計原則

1. **パイプライン哲学**: `curl | jq` の延長として機能すること
2. **プロセス分離**: JS実行は外部委譲。コアはピュアHTTP+HTML処理に集中
3. **フォールバックチェーン**: 静的HTML → ローカルCDP → 外部API の優先順位付き委譲
4. **副作用の境界明示**: ドメインロジックとI/O（HTTP・ファイル・LLM API）を型レベルで分離
5. **AI-Friendly設計**: 型と命名でドメイン意図を表現し、Claude Codeとの協働を前提とする

---

## 2. アーキテクチャ

### 2.1 クレート構成

```
stillo/
├── Cargo.toml                  # workspace root
├── crates/
│   ├── cli/                    # エントリポイント・引数解析
│   │   └── src/main.rs
│   ├── core/                   # ドメインロジック（副作用ゼロ）
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document.rs     # ドメイン型
│   │       ├── extractor.rs    # コンテンツ抽出ロジック
│   │       └── markdown.rs     # Markdownシリアライズ
│   ├── fetcher/                # HTTP取得レイヤー
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── http.rs         # reqwest ラッパー
│   │       └── spa.rs          # SPA委譲チェーン
│   ├── renderer/               # TUI表示
│   │   └── src/
│   │       ├── lib.rs
│   │       └── tui.rs          # ratatui ビュー
│   ├── llm/                    # LLMブリッジ
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs       # API抽象層
│   │       └── prompts.rs      # プロンプトテンプレート
│   └── mcp/                    # MCPサーバー
│       └── src/
│           ├── lib.rs
│           └── server.rs       # stdio transport
├── config/
│   └── default.toml            # デフォルト設定
└── CLAUDE.md                   # AI協働ガイド
```

### 2.2 依存方向

```
cli → fetcher → core
cli → renderer → core
cli → llm → core
cli → mcp → fetcher → core
```

`core` は他クレートに依存しない。副作用は `fetcher` / `llm` / `mcp` に閉じる。

---

## 3. ドメインモデル（`crates/core`）

### 3.1 ユビキタス言語

| 用語 | 定義 |
|------|------|
| `RawHtml` | HTTP取得した生のHTMLバイト列 |
| `ParsedDocument` | html5everでパースしたDOMツリー |
| `ExtractedContent` | Readabilityロジックで抽出したメインコンテンツ |
| `MarkdownDocument` | LLMに渡すMarkdown文字列 |
| `FetchResult` | 取得結果（成功・SPA検出・失敗）の直和型 |
| `DelegationTarget` | SPA描画委譲先の選択肢 |

### 3.2 コアドメイン型

```rust
// crates/core/src/document.rs

/// HTTP取得した生のHTML
#[derive(Debug, Clone)]
pub struct RawHtml {
    pub bytes: Vec<u8>,
    pub url: Url,
    pub content_type: String,
    pub status: u16,
}

/// パース済みドキュメント
#[derive(Debug)]
pub struct ParsedDocument {
    pub url: Url,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub root: NodeHandle,          // html5ever の NodeHandle
}

/// 抽出済みコンテンツ（副作用ゼロの純粋データ）
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub url: Url,
    pub title: String,
    pub byline: Option<String>,    // 著者・日付情報
    pub body_text: String,         // プレーンテキスト
    pub body_html: String,         // クリーンなHTML
    pub links: Vec<ExtractedLink>,
    pub metadata: PageMetadata,
}

#[derive(Debug, Clone)]
pub struct ExtractedLink {
    pub text: String,
    pub href: Url,
    pub rel: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PageMetadata {
    pub description: Option<String>,
    pub og_title: Option<String>,
    pub og_image: Option<String>,
    pub canonical: Option<Url>,
    pub published_at: Option<DateTime<Utc>>,
}

/// LLMに渡すMarkdown
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub content: String,
    pub source_url: Url,
    pub extracted_at: DateTime<Utc>,
}
```

### 3.3 SPA検出と委譲の状態モデル

```rust
// crates/core/src/document.rs

/// SPA判定結果（網羅的列挙）
#[derive(Debug, Clone, PartialEq)]
pub enum SpaDetection {
    /// 静的HTMLとして処理可能
    Static,
    /// SPAと判定: 本文テキストが閾値未満
    SuspectedSpa { text_length: usize },
    /// JSフレームワーク検出
    FrameworkDetected { framework: JsFramework },
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsFramework {
    React,
    Vue,
    Angular,
    Next,
    Nuxt,
    Unknown(String),
}

/// 委譲先（優先順位順）
#[derive(Debug, Clone, PartialEq)]
pub enum DelegationTarget {
    /// ローカルCDP（Ferrum経由）
    LocalCdp { port: u16 },
    /// Playwright デーモン（Unix socket）
    PlaywrightDaemon { socket_path: PathBuf },
    /// Jina Reader API
    JinaReader { api_key: Option<String> },
    /// Firecrawl（self-host or API）
    Firecrawl { base_url: Url, api_key: String },
    /// フォールバック不可
    Unavailable { reason: String },
}

/// フェッチ結果（直和型で網羅）
#[derive(Debug)]
pub enum FetchResult {
    /// 静的HTML取得成功
    Static(RawHtml),
    /// SPA検出 → 委譲先とともに返す
    SpaDelegated {
        detection: SpaDetection,
        target: DelegationTarget,
    },
    /// 委譲後の取得成功
    DelegatedHtml(RawHtml),
    /// 取得失敗
    Failed(FetchError),
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP error: {status} {url}")]
    Http { status: u16, url: Url },
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("Timeout after {seconds}s")]
    Timeout { seconds: u64 },
    #[error("Delegation failed: {0}")]
    DelegationFailed(String),
    #[error("All delegation targets unavailable")]
    NoDelegationAvailable,
}
```

---

## 4. フェッチレイヤー（`crates/fetcher`）

### 4.1 HTTPクライアント

```rust
// crates/fetcher/src/http.rs

use reqwest::{Client, ClientBuilder};

pub struct HttpFetcher {
    client: Client,
    config: HttpConfig,
}

pub struct HttpConfig {
    pub timeout_secs: u64,          // default: 30
    pub follow_redirects: bool,     // default: true
    pub max_redirects: usize,       // default: 10
    pub user_agent: String,         // default: "stillo/0.1"
    pub accept_language: String,    // default: "ja,en;q=0.9"
    pub cookie_store: bool,         // default: true
}

impl HttpFetcher {
    pub fn new(config: HttpConfig) -> Self {
        let client = ClientBuilder::new()
            .http2_prior_knowledge()              // HTTP/2 優先
            .use_rustls_tls()                     // rustls（TLS 1.3）
            .redirect(Policy::limited(config.max_redirects))
            .cookie_store(config.cookie_store)
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent(&config.user_agent)
            .build()
            .expect("failed to build HTTP client");
        Self { client, config }
    }

    pub async fn fetch(&self, url: &Url) -> Result<RawHtml, FetchError> {
        // 実装: GET → レスポンス → RawHtml
    }
}
```

**採用クレート**:
- `reqwest` with `rustls-tls` feature（TLS 1.3、HTTP/2）
- `cookie_store` feature（セッション管理）

### 4.2 SPA委譲チェーン

```rust
// crates/fetcher/src/spa.rs

pub struct SpaDelegationChain {
    targets: Vec<DelegationTarget>,   // 優先順位順のリスト
}

impl SpaDelegationChain {
    /// 設定ファイルと環境から利用可能なターゲットを構築
    pub fn from_config(config: &SpaConfig) -> Self { ... }

    /// フォールバックチェーンを実行
    pub async fn fetch_with_js(&self, url: &Url) -> Result<RawHtml, FetchError> {
        for target in &self.targets {
            match self.try_target(target, url).await {
                Ok(html) => return Ok(html),
                Err(e) => {
                    tracing::warn!("delegation target {:?} failed: {}", target, e);
                    continue;
                }
            }
        }
        Err(FetchError::NoDelegationAvailable)
    }

    async fn try_target(
        &self,
        target: &DelegationTarget,
        url: &Url,
    ) -> Result<RawHtml, FetchError> {
        match target {
            DelegationTarget::LocalCdp { port } => self.fetch_via_cdp(*port, url).await,
            DelegationTarget::PlaywrightDaemon { socket_path } => {
                self.fetch_via_playwright(socket_path, url).await
            }
            DelegationTarget::JinaReader { api_key } => {
                self.fetch_via_jina(api_key.as_deref(), url).await
            }
            DelegationTarget::Firecrawl { base_url, api_key } => {
                self.fetch_via_firecrawl(base_url, api_key, url).await
            }
            DelegationTarget::Unavailable { reason } => {
                Err(FetchError::DelegationFailed(reason.clone()))
            }
        }
    }
}
```

**デフォルト優先順位**（設定ファイルで変更可）:
1. `LocalCdp` — Chrome/Chromiumがローカルに存在する場合
2. `PlaywrightDaemon` — `stillo daemon` で常駐起動している場合
3. `JinaReader` — `JINA_API_KEY` 環境変数があれば有料、なければ無料tier
4. `Firecrawl` — `FIRECRAWL_URL` + `FIRECRAWL_API_KEY` 環境変数がある場合

---

## 5. コンテンツ抽出エンジン（`crates/core/src/extractor.rs`）

```rust
pub struct ContentExtractor {
    config: ExtractorConfig,
}

pub struct ExtractorConfig {
    /// 本文とみなす最小テキスト長（SPA判定にも使用）
    pub min_content_length: usize,       // default: 500
    /// 除去するセレクタ（nav, header, footer etc.）
    pub noise_selectors: Vec<String>,
    /// リンクを保持するか
    pub preserve_links: bool,            // default: true
}

impl ContentExtractor {
    /// RawHtml → ExtractedContent（純粋関数）
    pub fn extract(&self, raw: &RawHtml) -> Result<ExtractedContent, ExtractionError> {
        let document = self.parse_html(raw)?;
        let spa_detection = self.detect_spa(&document);
        let content = self.readability_extract(&document)?;
        Ok(content)
    }

    /// SPA判定（副作用ゼロ）
    fn detect_spa(&self, doc: &ParsedDocument) -> SpaDetection {
        let text_len = extract_text_length(doc);
        if text_len < self.config.min_content_length {
            return SpaDetection::SuspectedSpa { text_length: text_len };
        }
        if let Some(framework) = detect_js_framework(doc) {
            return SpaDetection::FrameworkDetected { framework };
        }
        SpaDetection::Static
    }
}
```

### 5.1 Readabilityロジック

Mozilla Readability.jsのRust移植として以下のアルゴリズムを実装する。

1. **ノイズ除去**: `<nav>`, `<header>`, `<footer>`, `<aside>`, `class="sidebar"` 等を除去
2. **スコアリング**: `<p>` タグのテキスト密度、リンク密度比でノードをスコアリング
3. **本文抽出**: 最高スコアのコンテナノード以下をメインコンテンツとして採用
4. **クリーンアップ**: 残存する低スコアノードを除去

**既存クレートの調査対象**:
- `readability` crate（存在する場合は採用を検討）
- なければ `html5ever` + 独自実装

---

## 6. Markdownシリアライザ（`crates/core/src/markdown.rs`）

```rust
pub struct MarkdownSerializer {
    config: MarkdownConfig,
}

pub struct MarkdownConfig {
    pub max_line_width: usize,      // default: 80（pager表示向け）
    pub include_links: bool,        // default: true
    pub include_images: bool,       // default: false（テキスト優先）
    pub heading_style: HeadingStyle,
}

#[derive(Debug, Clone)]
pub enum HeadingStyle {
    Atx,     // # H1, ## H2 ...（default）
    Setext,  // H1\n===
}

impl MarkdownSerializer {
    /// ExtractedContent → MarkdownDocument（純粋関数）
    pub fn serialize(&self, content: &ExtractedContent) -> MarkdownDocument {
        let mut out = String::new();
        // title
        writeln!(out, "# {}", content.title);
        if let Some(byline) = &content.byline {
            writeln!(out, "*{}*\n", byline);
        }
        writeln!(out, "> Source: {}\n", content.url);
        // body
        out.push_str(&self.html_to_markdown(&content.body_html));
        MarkdownDocument {
            content: out,
            source_url: content.url.clone(),
            extracted_at: Utc::now(),
        }
    }
}
```

---

## 7. CLIインターフェース（`crates/cli`）

### 7.1 コマンド体系

```
stillo [OPTIONS] [URL]

SUBCOMMANDS:
  browse      TUIブラウザモード（デフォルト）
  dump        Markdown/テキストをstdoutに出力
  qa          ページについてLLMに質問
  summarize   ページを要約
  extract     指定した情報を抽出
  daemon      Playwright委譲デーモンを起動
  mcp         MCPサーバーとして起動（stdio）

OPTIONS:
  --format <FORMAT>     出力形式 [markdown|plain|json] (default: markdown)
  --delegate <TARGET>   SPA委譲先を明示 [cdp|playwright|jina|firecrawl]
  --no-delegate         JS委譲を無効化（静的HTMLのみ）
  --timeout <SECS>      タイムアウト秒数 (default: 30)
  --config <PATH>       設定ファイルパス
  -v, --verbose         詳細ログ出力
```

### 7.2 使用例

```bash
# TUIブラウズ
stillo https://example.com

# Markdown dump → llmへパイプ
stillo dump https://example.com | llm "要約して"

# QA
stillo qa "この記事の著者は誰？" https://example.com

# サマリー
stillo summarize https://example.com

# 構造化抽出（JSON出力）
stillo extract --format json "タイトル,著者,公開日" https://example.com

# MCPサーバーとして起動（Claude Code用）
stillo mcp
```

### 7.3 引数解析クレート

`clap` v4（`derive` feature）を使用。

```rust
#[derive(Parser)]
#[command(name = "stillo", version, about = "AI-native terminal browser")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// デフォルトはbrowseモード
    pub url: Option<Url>,

    #[arg(long, default_value = "markdown")]
    pub format: OutputFormat,

    #[arg(long)]
    pub delegate: Option<DelegateTarget>,

    #[arg(long)]
    pub no_delegate: bool,

    #[arg(long, default_value = "30")]
    pub timeout: u64,
}

#[derive(Subcommand)]
pub enum Command {
    Browse { url: Url },
    Dump { url: Url, #[arg(long)] format: Option<OutputFormat> },
    Qa { question: String, url: Url },
    Summarize { url: Url },
    Extract { fields: String, url: Url, #[arg(long)] format: Option<OutputFormat> },
    Daemon,
    Mcp,
}
```

---

## 8. TUIビューア（`crates/renderer`）

### 8.1 採用クレート

`ratatui` + `crossterm`

### 8.2 UIコンポーネント

```
┌─────────────────────────────────────────────────────┐
│ stillo │ https://example.com             [q]uit [/]search │
├─────────────────────────────────────────────────────┤
│                                                     │
│  # Article Title                                    │
│                                                     │
│  Lorem ipsum dolor sit amet...                     │
│                                                     │
│  [1] Link to something                              │
│  [2] Another link                                   │
│                                                     │
├─────────────────────────────────────────────────────┤
│ [Enter]follow  [Tab]next-link  [?]ask-AI  [d]dump   │
└─────────────────────────────────────────────────────┘
```

### 8.3 キーバインド（w3m互換）

| キー | 動作 |
|------|------|
| `j` / `↓` | 下スクロール |
| `k` / `↑` | 上スクロール |
| `Enter` | リンクをフォロー |
| `B` / `Alt+←` | 戻る |
| `Tab` | 次のリンクへ |
| `U` | URLを直接入力 |
| `/` | ページ内検索 |
| `n` | 次の検索結果 |
| `d` | Markdown dump |
| `?` | LLMへ質問（インラインプロンプト） |
| `q` | 終了 |

---

## 9. LLMブリッジ（`crates/llm`）

### 9.1 API抽象層

```rust
// crates/llm/src/client.rs

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(
        &self,
        messages: Vec<Message>,
        config: &CompletionConfig,
    ) -> Result<CompletionStream, LlmError>;
}

pub struct CompletionConfig {
    pub max_tokens: u32,    // default: 1024
    pub temperature: f32,   // default: 0.3（事実抽出向け）
    pub stream: bool,       // default: true
}

/// 実装: Claude（Anthropic API）
pub struct AnthropicClient {
    api_key: String,
    model: String,          // default: "claude-sonnet-4-5"
    http: reqwest::Client,
}

/// 実装: OpenAI互換
pub struct OpenAiCompatClient {
    base_url: Url,          // OpenAI / Ollama / LM Studio
    api_key: Option<String>,
    model: String,
    http: reqwest::Client,
}
```

### 9.2 プロンプトテンプレート

```rust
// crates/llm/src/prompts.rs

pub fn summarize_prompt(doc: &MarkdownDocument) -> Vec<Message> {
    vec![
        Message::system("You are a precise summarizer. Respond in the same language as the document. Be concise."),
        Message::user(format!(
            "以下のWebページを3-5文で要約してください。\n\nURL: {}\n\n{}",
            doc.source_url,
            truncate(&doc.content, 6000)
        )),
    ]
}

pub fn qa_prompt(question: &str, doc: &MarkdownDocument) -> Vec<Message> {
    vec![
        Message::system("Answer questions about the provided web page content. Be direct and cite the relevant parts."),
        Message::user(format!(
            "以下のWebページについて質問に答えてください。\n\n質問: {}\n\nURL: {}\n\n{}",
            question,
            doc.source_url,
            truncate(&doc.content, 6000)
        )),
    ]
}

pub fn extract_prompt(fields: &str, doc: &MarkdownDocument) -> Vec<Message> {
    vec![
        Message::system("Extract structured information from the web page. Return JSON only, no explanation."),
        Message::user(format!(
            "以下のフィールドをJSON形式で抽出してください: {}\n\nURL: {}\n\n{}",
            fields,
            doc.source_url,
            truncate(&doc.content, 6000)
        )),
    ]
}
```

---

## 10. MCPサーバー（`crates/mcp`）

### 10.1 公開ツール定義

```json
{
  "tools": [
    {
      "name": "fetch_url",
      "description": "Fetch a URL and return its content as Markdown. Handles SPAs via delegation chain.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "url": { "type": "string", "description": "URL to fetch" },
          "format": {
            "type": "string",
            "enum": ["markdown", "plain", "json"],
            "default": "markdown"
          },
          "delegate": {
            "type": "string",
            "enum": ["auto", "cdp", "jina", "none"],
            "default": "auto"
          }
        },
        "required": ["url"]
      }
    },
    {
      "name": "read_links",
      "description": "Extract all links from a URL with their anchor text.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "url": { "type": "string" }
        },
        "required": ["url"]
      }
    },
    {
      "name": "extract_structured",
      "description": "Extract specific fields from a page as JSON.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "url": { "type": "string" },
          "fields": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Field names to extract"
          }
        },
        "required": ["url", "fields"]
      }
    }
  ]
}
```

### 10.2 起動方法（Claude Code設定）

```json
// ~/.claude/claude_desktop_config.json
{
  "mcpServers": {
    "stillo": {
      "command": "stillo",
      "args": ["mcp"]
    }
  }
}
```

### 10.3 トランスポート

MCP仕様のstdio transport（JSON-RPC 2.0）を実装する。

```rust
// crates/mcp/src/server.rs

pub struct McpServer {
    fetcher: Arc<HttpFetcher>,
    extractor: Arc<ContentExtractor>,
    serializer: Arc<MarkdownSerializer>,
    spa_chain: Arc<SpaDelegationChain>,
}

impl McpServer {
    pub async fn run_stdio(&self) -> Result<(), McpError> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        // JSON-RPC 2.0 リクエストを読み取り、ツールを実行し、レスポンスを返す
        loop {
            let request = self.read_request(&mut stdin).await?;
            let response = self.handle_request(request).await;
            self.write_response(&mut stdout, response).await?;
        }
    }
}
```

---

## 11. 設定ファイル

```toml
# ~/.config/stillo/config.toml

[http]
timeout_secs = 30
user_agent = "stillo/0.1"
cookie_store = true

[extractor]
min_content_length = 500
preserve_links = true

[markdown]
max_line_width = 80
include_links = true
include_images = false

[spa]
# 委譲先の優先順位（上から試行）
delegation_chain = ["cdp", "playwright", "jina"]

[spa.cdp]
port = 9222                     # Chrome --remote-debugging-port

[spa.playwright]
socket_path = "/tmp/stillo-playwright.sock"

[spa.jina]
# JINA_API_KEY 環境変数から自動取得

[spa.firecrawl]
# FIRECRAWL_URL / FIRECRAWL_API_KEY 環境変数から自動取得

[llm]
provider = "anthropic"          # anthropic | openai | ollama
model = "claude-sonnet-4-5"
max_tokens = 1024
# ANTHROPIC_API_KEY 環境変数から自動取得

[llm.ollama]
base_url = "http://localhost:11434"
model = "llama3"
```

---

## 12. 採用クレート一覧

| カテゴリ | クレート | バージョン | 用途 |
|----------|----------|-----------|------|
| HTTP | `reqwest` | 0.12 | HTTP/2クライアント（rustls feature） |
| TLS | `rustls` | 0.23 | TLS 1.3（reqwest経由） |
| HTMLパース | `html5ever` | 0.27 | HTML5準拠パーサ |
| DOM操作 | `markup5ever_rcdom` | 0.3 | DOMツリー操作 |
| URL | `url` | 2.5 | URL型・バリデーション |
| 非同期 | `tokio` | 1 | async runtime（full features） |
| CLI | `clap` | 4 | 引数解析（derive feature） |
| TUI | `ratatui` | 0.28 | ターミナルUI |
| ターミナル | `crossterm` | 0.28 | クロスプラットフォームターミナル制御 |
| 設定 | `config` | 0.14 | TOML設定ファイル |
| シリアライズ | `serde` + `serde_json` | 1 | JSON / 構造体変換 |
| エラー | `thiserror` | 2 | エラー型定義 |
| エラー伝播 | `anyhow` | 1 | アプリケーション層エラー |
| ログ | `tracing` + `tracing-subscriber` | 0.1 | 構造化ログ |
| 日時 | `chrono` | 0.4 | DateTime型 |
| 非同期trait | `async-trait` | 0.1 | async fn in trait |
| CDP | `chromiumoxide` または `ferrum` | latest | CDP接続（SPA委譲） |
| テスト | `tokio::test` + `mockito` | — | 非同期テスト・HTTPモック |

---

## 13. ディレクトリ構造（完全版）

```
stillo/
├── Cargo.toml
├── Cargo.lock
├── CLAUDE.md                           # AI協働ガイド・lessons
├── README.md
├── config/
│   └── default.toml
├── crates/
│   ├── cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── args.rs
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document.rs
│   │       ├── extractor.rs
│   │       ├── extractor/
│   │       │   ├── readability.rs
│   │       │   └── spa_detection.rs
│   │       └── markdown.rs
│   ├── fetcher/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── http.rs
│   │       └── spa/
│   │           ├── mod.rs
│   │           ├── cdp.rs
│   │           ├── playwright.rs
│   │           ├── jina.rs
│   │           └── firecrawl.rs
│   ├── renderer/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tui.rs
│   │       └── widgets/
│   │           ├── content_view.rs
│   │           ├── link_bar.rs
│   │           └── status_bar.rs
│   ├── llm/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs
│   │       ├── providers/
│   │       │   ├── anthropic.rs
│   │       │   ├── openai_compat.rs
│   │       │   └── ollama.rs
│   │       └── prompts.rs
│   └── mcp/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── server.rs
│           └── tools/
│               ├── fetch_url.rs
│               ├── read_links.rs
│               └── extract_structured.rs
└── tests/
    ├── integration/
    │   ├── fetch_static.rs
    │   ├── fetch_spa.rs
    │   └── mcp_server.rs
    └── fixtures/
        ├── static_page.html
        └── spa_page.html
```

---

## 14. 実装フェーズ

### Phase 1: コアパイプライン（MVP）
- `core`: ドメイン型・抽出エンジン・Markdownシリアライザ
- `fetcher`: HTTP取得（静的HTMLのみ）
- `cli`: `dump` サブコマンドのみ
- 目標: `stillo dump https://example.com` が動作すること

### Phase 2: SPA委譲
- `fetcher/spa`: CDPおよびJina Reader実装
- SPA検出ロジック
- 目標: React/Next.jsサイトでもコンテンツ抽出できること

### Phase 3: TUIビューア
- `renderer`: ratatuiベースのTUI
- キーバインド・リンクナビゲーション
- 目標: `stillo https://example.com` でターミナルブラウジングできること

### Phase 4: LLMブリッジ
- `llm`: Anthropic / OpenAI / Ollama クライアント
- `cli`: `qa` / `summarize` / `extract` サブコマンド
- 目標: `stillo qa "著者は？" https://example.com` が動作すること

### Phase 5: MCPサーバー
- `mcp`: stdio transport実装
- Claude Code設定ドキュメント
- 目標: Claude CodeからMCPツールとして呼び出せること

---

## 15. CLAUDE.md（AI協働ガイド）

```markdown
# stillo CLAUDE.md

## プロジェクト概要
AIネイティブなターミナルブラウザ。Rust実装。

## クレート責務
- `core`: 純粋関数のみ。副作用禁止
- `fetcher`: HTTP・SPA委譲のI/O
- `renderer`: TUI描画
- `llm`: LLM API呼び出し
- `mcp`: MCPサーバー
- `cli`: コマンドライン引数解析・全体オーケストレーション

## 非同期
async/awaitを使用。.then()チェーン禁止。

## エラー処理
- ライブラリクレート: `thiserror` で型付きエラー
- アプリケーション層（cli）: `anyhow` で伝播

## 命名規則
- SPA関連型: `Spa` プレフィックス（例: `SpaDetection`, `SpaDelegationChain`）
- 取得結果: `FetchResult` enum（直和型）
- 変換関数: `extract_*`, `serialize_*`, `detect_*`

## lessons
- RawHtmlはバイト列で保持し、文字コード変換は抽出レイヤーで行う
- SPAかどうかを判定してから委譲先を選ぶ（委譲先の選択をフェッチ前に決めない）
- MCPのstdout書き込みはバッファリングして都度flushすること
```
