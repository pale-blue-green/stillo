# TODO

## Open

- [x] [IMPROVE] readability: 全項目がリンクで包まれたインデックスページの抽出精度向上 <!-- 2026-05-14, fixed 2026-05-14 -->
  - gihyo.jp 等、記事カード全体が `<a>` で囲まれた構造では body フォールバックが動作するがサイドバーノイズが混入する
  - 隣接する同クラス要素の繰り返しパターン検出（シブリング展開）で親コンテナを選ぶヒューリスティックが有効な可能性がある
  - File: `crates/core/src/extractor/readability.rs`

- [x] [BUG] fetch_url MCP ツール: Markdown 本文中のリンクが相対 URL のまま返される <!-- 2026-05-14, fixed 2026-05-14 -->
  - `HtmlToMarkdown` に base_url を持たせ `url::Url::join()` で絶対化
  - File: `crates/core/src/markdown.rs`
