use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
        mpsc::Sender,
    },
    time::Instant,
};

use axum::{
    Router,
    http::{StatusCode, Uri, header},
    response::IntoResponse,
    routing::get,
};
use mime_guess::MimeGuess;
use rust_embed::Embed;
use tokio::sync::{Mutex, broadcast::Receiver};

use crate::{ClientInput, GameMessage};

use super::{markup::index_handler, server::ws_handler};

/* Router definitions */

#[derive(Clone)]
pub(crate) struct AppState {
    pub startup: Instant,
    pub tx: Arc<Sender<ClientInput>>,
    pub rx: Arc<Mutex<Receiver<GameMessage>>>,
    pub is_connected: Arc<AtomicBool>,
    pub latency: Arc<AtomicU64>,
}

#[derive(Embed)]
#[folder = "src/http/assets/"]
struct Asset;

/// A lazily-created Router, to be used by the SSH client tunnels.
pub(crate) async fn get_router(tx: Sender<ClientInput>, rx: Receiver<GameMessage>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/", get(index_handler))
        .with_state(AppState {
            is_connected: Arc::new(AtomicBool::new(false)),
            startup: Instant::now(),
            latency: Arc::new(AtomicU64::new(0)),
            tx: Arc::new(tx),
            rx: Arc::new(Mutex::new(rx)),
        })
        .nest("/assets", assets_router())
}

/* Static assets */

fn assets_router() -> Router {
    Router::new().route(
        "/{*file}",
        get(|uri: Uri| async move {
            let path = uri.path().trim_start_matches('/');
            match Asset::get(path) {
                Some(content) => {
                    let mime = MimeGuess::from_path(path).first_or_octet_stream();
                    ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
                }
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }),
    )
}
