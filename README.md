# stillo

AI-native terminal browser written in Rust.

## Features

- Fetch and render web pages in the terminal (TUI)
- SPA / JavaScript-rendered page support via delegation chain (CDP, Jina Reader, Firecrawl)
- Export page content as Markdown
- w3m-compatible key bindings

## Installation

```bash
cargo install stillo
```

## Usage

```bash
# Open TUI browser
stillo https://example.com

# Dump page as Markdown
stillo dump https://example.com

# Use Jina Reader for SPA pages
stillo dump --delegate jina https://react.dev
```

## Key Bindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `g` / `Home` | Scroll to top |
| `G` / `End` | Scroll to bottom |
| `Tab` | Next link |
| `Shift+Tab` | Previous link |
| `Enter` | Follow selected link |
| `B` | Go back |
| `U` | Open URL |
| `/` | Search |
| `n` | Next search match |
| `d` | Dump as Markdown |
| `q` | Quit |

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `stillo` | CLI binary |
| `stillo-core` | Pure domain types and extraction logic |
| `stillo-fetcher` | HTTP client and SPA delegation |
| `stillo-renderer` | ratatui TUI renderer |

## License

MIT OR Apache-2.0
