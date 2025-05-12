use core::str;
use std::{
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

use anyhow::{Context, Result};
use axum::routing::RouterIntoService;
use backon::{ExponentialBuilder, Retryable};
use base64::{prelude::BASE64_STANDARD, Engine};
use hmac::{Hmac, Mac};
use hyper::body::Incoming;
use hyper_util::service::TowerToHyperService;
use reqwest::{Client, Response};
use russh::{client, keys::PrivateKeyWithHashAlg};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{debug, error, info};

use crate::{http::ROUTER, ssh::TcpForwardSession, ClientInput};

#[derive(Serialize)]
struct AuthenticationRequest<'a> {
    audience: &'a str,
    user: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct AuthenticationResponse {
    jwt: String,
    server_fingerprint: String,
}

/* Local server entrypoint */

/// Begins remote port forwarding (reverse tunneling) with Russh to serve an Axum application.
pub(crate) async fn ssh_entrypoint(
    host: &str,
    port: u16,
    login_name: &str,
    audience: &str,
    debug_key: Option<PrivateKeyWithHashAlg>,
    game_input_tx: Sender<ClientInput>,
) -> Result<()> {
    let config = Arc::new(client::Config {
        ..Default::default()
    });
    let mac = Hmac::<Sha256>::new_from_slice(audience.as_bytes())
        .unwrap()
        .chain_update(login_name.as_bytes())
        .finalize();
    let password = BASE64_STANDARD.encode(mac.into_bytes());
    let router: RouterIntoService<Incoming> = ROUTER
        .get()
        .with_context(|| "Router hasn't been initialized.")?
        .clone()
        .into_service();
    let hyper_service = TowerToHyperService::new(router);
    loop {
        let mut session = match debug_key {
            Some(ref key) => {
                let connect = async || {
                    TcpForwardSession::connect_key(
                        host,
                        port,
                        login_name,
                        key,
                        Arc::clone(&config),
                        hyper_service.clone(),
                    )
                    .await
                };
                connect
                    .retry(
                        ExponentialBuilder::default()
                            .with_jitter()
                            .with_max_delay(Duration::from_secs(20)),
                    )
                    .await
                    .with_context(|| "SSH connection failed.")?
            }
            None => {
                let request = async || {
                    let client = Client::new();
                    let result: Result<Response> = Ok(client
                        .post("TO-DO")
                        .header("content-type", "application/json")
                        .body(
                            serde_json::to_string(&AuthenticationRequest {
                                audience,
                                user: login_name,
                                password: &password,
                            })
                            .unwrap(),
                        )
                        .send()
                        .await
                        .with_context(|| "Unable to send request to signaling server")?
                        .error_for_status()
                        .with_context(|| "Received error from signaling server")?);
                    result
                };
                let result = request
                    .retry(ExponentialBuilder::default().with_jitter())
                    .await
                    .with_context(|| "Token creation failed.")?;
                let response: AuthenticationResponse =
                    serde_json::from_str(str::from_utf8(&result.bytes().await.unwrap()).unwrap())
                        .unwrap();
                let connect = async || {
                    TcpForwardSession::connect_password(
                        host,
                        port,
                        login_name,
                        &response.jwt,
                        Arc::clone(&config),
                        &response.server_fingerprint,
                        hyper_service.clone(),
                    )
                    .await
                };
                connect
                    .retry(
                        ExponentialBuilder::default()
                            .with_jitter()
                            .with_max_delay(Duration::from_secs(20)),
                    )
                    .await
                    .with_context(|| "SSH connection failed.")?
            }
        };
        match session.start_forwarding(game_input_tx.clone()).await {
            Err(e) => error!(error = ?e, "TCP forward session failed."),
            _ => info!("Connection closed."),
        }
        debug!("Attempting graceful disconnect.");
        if let Err(e) = session.close().await {
            debug!(error = ?e, "Graceful disconnect failed.")
        }
        debug!("Restarting connection.");
    }
}
