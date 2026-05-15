# stillo

AIネイティブなターミナルブラウザ。Rustで実装。

**stillo** は *distill*（蒸留する）から作った造語です。Webページを蒸留して情報だけを抽出するというコンセプトを名前に込めています。

## 概要

stilloはターミナル上で動作するブラウザです。静的なHTMLページはそのまま取得し、React/Vue等のSPAはChrome CDP・Jina Reader・Firecrawl等へ自動委譲してコンテンツを取得します。取得したページはMarkdown/テキスト/JSONとして出力するか、TUIで対話的にブラウズできます。LLMと連携してQ&A・要約・構造化抽出を行う機能や、MCPサーバーとして他のAIエージェントに機能を提供する機能も備えています。

## インストール

```bash
cargo build --release
# バイナリは target/release/stillo に生成されます
```

## 使い方

```bash
# TUIブラウザとして開く
stillo https://example.com

# Markdownとして標準出力に出力
stillo dump https://example.com

# 出力形式を指定
stillo dump --format json https://example.com
stillo dump --format plain https://example.com

# SPA委譲先を明示
stillo dump --delegate jina https://example.com

# JS委譲を無効化（静的HTMLのみ処理）
stillo dump --no-delegate https://example.com

# タイムアウト秒数を指定（デフォルト30秒）
stillo dump --timeout 60 https://example.com

# 詳細ログを出力（stderrに出力）
stillo dump -v https://example.com
```

## サブコマンド

| コマンド | 説明 |
|----------|------|
| `dump`      | ページをMarkdown/テキスト/JSONとして stdout に出力 |
| `browse`    | TUIブラウザとして起動 |
| `qa`        | ページについてLLMに質問（LLM APIキーが必要） |
| `summarize` | ページをLLMで要約（LLM APIキーが必要） |
| `extract`   | 指定フィールドをLLMで抽出（LLM APIキーが必要） |
| `mcp`       | MCPサーバーとして起動（stdio transport、JSON-RPC 2.0） |

## SPA委譲

SPAと判定されたページは以下の優先順位で委譲を試みます（いずれかが成功すれば以降はスキップ）。

1. **Chrome CDP** — `localhost:9222` でChromeが起動していれば使用
2. **Playwright Daemon** — `/tmp/stillo-playwright.sock` が存在すれば使用
3. **Jina Reader** — 常にフォールバック候補（`JINA_API_KEY` 設定で認証）
4. **Firecrawl** — `FIRECRAWL_URL` と `FIRECRAWL_API_KEY` が両方設定されていれば使用

全ターゲットが失敗した場合は静的HTMLにフォールバックします。

### Chrome CDP を使う場合

```bash
# --remote-debugging-port=9222 でChromeを起動しておく
google-chrome --remote-debugging-port=9222 --headless
stillo dump https://your-spa-app.example.com
```

### Jina Reader を使う場合

```bash
export JINA_API_KEY=your_api_key
stillo dump https://your-spa-app.example.com
```

### Firecrawl を使う場合

```bash
export FIRECRAWL_URL=https://api.firecrawl.dev/
export FIRECRAWL_API_KEY=your_api_key
stillo dump https://your-spa-app.example.com
```

## LLM連携

`qa`・`summarize`・`extract` サブコマンドはLLMにアクセスします。以下の環境変数で使用するプロバイダーを選択します（上から優先）。

| 優先度 | プロバイダー | 環境変数 |
|--------|-------------|----------|
| 1 | Anthropic | `ANTHROPIC_API_KEY`（モデル: `ANTHROPIC_MODEL`、デフォルト `claude-sonnet-4-5`） |
| 2 | OpenAI | `OPENAI_API_KEY`（ベースURL: `OPENAI_BASE_URL`、モデル: `OPENAI_MODEL`、デフォルト `gpt-4o-mini`） |
| 3 | llama.cpp | `LLAMA_CPP_BASE_URL`（モデル: `LLAMA_CPP_MODEL`、デフォルト `default`、APIキー不要） |
| 4 | Ollama（ローカル） | `OLLAMA_BASE_URL`（デフォルト `http://localhost:11434/`）、`OLLAMA_MODEL`（デフォルト `llama3`） |

OpenAI互換APIを持つ任意のプロバイダー（LM Studio等）は `OPENAI_BASE_URL` と `OPENAI_API_KEY` で利用できます。

```bash
# Anthropic でQ&A
export ANTHROPIC_API_KEY=your_api_key
stillo qa "この記事の主張は何ですか？" https://example.com/article

# 要約
stillo summarize https://example.com/article

# フィールド抽出（カンマ区切りで複数指定）
stillo extract "title,author,published_at" https://example.com/article
stillo extract "title,author" --format json https://example.com/article

# Ollama でローカル実行
stillo qa "記事の要点は？" https://example.com/article
```

## MCPサーバー

`stillo mcp` でstdio transportのMCPサーバーとして起動します。Claude DesktopやClaude Codeなど、MCPクライアントからstilloの機能を呼び出せます。

### 利用可能なツール

| ツール | 説明 |
|--------|------|
| `fetch_url` | URLを取得しMarkdown/テキスト/JSONで返す。SPA自動委譲対応 |
| `read_links` | URLからリンク一覧（テキスト・href）を抽出して返す |
| `extract_structured` | LLMを使って指定フィールドをJSONで抽出する（LLM APIキーが必要） |

### Claude Codeでの設定例

```json
{
  "mcpServers": {
    "stillo": {
      "command": "/path/to/stillo",
      "args": ["mcp"]
    }
  }
}
```

## クレート構成

| クレート | 役割 |
|---------|------|
| `stillo-core`     | 純粋関数のみ（HTML解析・コンテンツ抽出・Markdown変換） |
| `stillo-fetcher`  | HTTP取得・SPA委譲 |
| `stillo-renderer` | TUI描画 |
| `stillo-llm`      | LLM API呼び出し（Anthropic / OpenAI互換 / Ollama） |
| `stillo-mcp`      | MCPサーバー（JSON-RPC 2.0 stdio transport） |
| `stillo` (cli)    | コマンドライン引数解析・全体オーケストレーション |

## 将来構想

### stilloネイティブモード

サーバとクライアントの主従を逆転させるコンセプト。従来のWebはサーバがHTMLでデザインごとコンテンツを送りつけるが、stilloネイティブモードではサーバはデータとセマンティクスのみを返し、表示・デザインはクライアントが全面的にコントロールする。

既存のWebへのフォールバックを維持しつつ、対応サーバにはよりリッチなセマンティクスを返させるオプトイン型の拡張として設計する想定。`stillo mcp` と組み合わせ、MCPサーバが変換層を担うことも視野に入れる。

## 開発

```bash
# ビルド
cargo build

# テスト
cargo test

# CDPサポートを有効にしてビルド
cargo build --features stillo-fetcher/cdp
```

ログレベルは `RUST_LOG` 環境変数で制御できます（stderrに出力）。

```bash
RUST_LOG=debug stillo dump https://example.com
```
