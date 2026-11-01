//! The axum router: the JSON API under `/api` and the CDN passthrough under
//! `/cdn`. The SvelteKit dev proxy and Caddy route these paths here and
//! everything else to the frontend, so the browser only ever sees one origin.

pub mod api;
pub mod imports;
pub mod skinr;

use std::path::PathBuf;

use axum::Router;
use axum::extract::FromRef;
use axum::routing::get;
use sqlx::PgPool;

use crate::cdn::{self, CdnClient};

/// The default bind address of the API; the dev proxy and Caddy point at it.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3100";

/// Where `frontend/scripts/render-thumbnails.ts` writes ship thumbnails.
const DEFAULT_THUMBNAILS_DIR: &str = "storage/thumbnails";

/// The thumbnail directory from `THUMBNAILS_DIR`, with the local default.
pub fn thumbnails_dir() -> PathBuf {
    std::env::var("THUMBNAILS_DIR")
        .unwrap_or_else(|_| DEFAULT_THUMBNAILS_DIR.to_owned())
        .into()
}

/// The listen address from `BIND_ADDR`, with the local default.
pub fn bind_addr() -> String {
    std::env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned())
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cdn: CdnClient,
    /// Rendered ship thumbnails (`<id>-<theme>.png`), served under /api/thumbnails.
    pub thumbnails: PathBuf,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for CdnClient {
    fn from_ref(state: &AppState) -> Self {
        state.cdn.clone()
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api::router())
        .route("/cdn/binaries/{*path}", get(cdn::binaries))
        .route("/cdn/resources/{*path}", get(cdn::resources))
        .fallback(api::not_found)
        .with_state(state)
}
