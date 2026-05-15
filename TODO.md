# TODO

## Open

- [x] [IMPROVE] readability: 全項目がリンクで包まれたインデックスページの抽出精度向上 <!-- 2026-05-14, fixed 2026-05-14 -->
  - gihyo.jp 等、記事カード全体が `<a>` で囲まれた構造では body フォールバックが動作するがサイドバーノイズが混入する
  - 隣接する同クラス要素の繰り返しパターン検出（シブリング展開）で親コンテナを選ぶヒューリスティックが有効な可能性がある
  - File: `crates/core/src/extractor/readability.rs`

- [x] [BUG] fetch_url MCP ツール: Markdown 本文中のリンクが相対 URL のまま返される <!-- 2026-05-14, fixed 2026-05-14 -->
  - `HtmlToMarkdown` に base_url を持たせ `url::Url::join()` で絶対化
  - File: `crates/core/src/markdown.rs`

- [x] [BUG] RSS 1.0 (RDF形式) 未対応 <!-- 2026-05-15, fixed 2026-05-15 -->
  - はてブの RSS は `<rdf:RDF>` をルートとする RSS 1.0 形式。`parse_rss_to_ast` が対応するのは RSS 2.0 (`<rss>`) と Atom 1.0 (`<feed>`) のみ
  - links が空になり、フィードとして認識されず HTML 扱いになる
  - File: `crates/core/src/rss_to_ast.rs`

- [x] [BUG] Classmethod dev blog の本文抽出が薄い <!-- 2026-05-15, fixed 2026-05-15 -->
  - `https://dev.classmethod.jp/articles/` の記事で readability が本文を取得できず目次・関連記事のみになる（SPA委譲なしで13行しか取得できない）
  - 原因: `is_noise` が `val.contains(pattern)` で部分文字列一致していたため、Tailwind の `shadow-2xs` クラスが `"ad"` パターンに誤ヒットして記事ラッパー全体をスキップしていた
  - 修正: `class_contains_pattern()` でスペース→ハイフン区切りのコンポーネント完全一致に変更
  - File: `crates/core/src/extractor/readability.rs`
