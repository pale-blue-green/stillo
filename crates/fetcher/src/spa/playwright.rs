use std::path::Path;
use url::Url;
use stillo_core::document::{FetchError, RawHtml};

/// Playwright デーモン経由のフェッチ（Phase 3以降で実装予定）
pub async fn fetch_via_playwright(
    _socket_path: &Path,
    _url: &Url,
) -> Result<RawHtml, FetchError> {
    Err(FetchError::DelegationFailed(
        "Playwright daemon is not implemented yet (planned for Phase 3)".into(),
    ))
}
