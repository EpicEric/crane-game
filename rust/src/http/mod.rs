use std::sync::OnceLock;

use axum::Router;

pub mod markup;
pub mod router;
pub mod server;

/// A lazily-created Router, to be used by the SSH client tunnels.
pub static ROUTER: OnceLock<Router> = OnceLock::new();
