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

- [ ] [IMPROVE] ソーシャル共有ウィジェットの除外（低優先度） <!-- 2026-05-15 -->
  - 日経新聞など CSS Modules 実装サイトで「記事を印刷する」「X（旧Twitter）」等の共有ボタンテキストが本文に混入する
  - 誤検出リスクを考慮して対応見送り。方針は2択: (A) NOISE_CLASS_PATTERNS に "share"/"sns" 追加（30分、CSS Modules には効かない）/ (B) `<li>` テキストが SNS 名と完全一致ならスキップ（2〜3時間、CSS Modules も対応可だが本文 SNS 言及を誤除外するリスクあり）
  - File: `crates/core/src/extractor/readability.rs`

- [x] [BUG] Classmethod dev blog の本文抽出が薄い <!-- 2026-05-15, fixed 2026-05-15 -->
  - `https://dev.classmethod.jp/articles/` の記事で readability が本文を取得できず目次・関連記事のみになる（SPA委譲なしで13行しか取得できない）
  - 原因: `is_noise` が `val.contains(pattern)` で部分文字列一致していたため、Tailwind の `shadow-2xs` クラスが `"ad"` パターンに誤ヒットして記事ラッパー全体をスキップしていた
  - 修正: `class_contains_pattern()` でスペース→ハイフン区切りのコンポーネント完全一致に変更
  - File: `crates/core/src/extractor/readability.rs`
