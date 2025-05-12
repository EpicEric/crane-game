use std::sync::{Arc, mpsc::Sender};

use anyhow::{Context, Result, anyhow};
use axum::routing::RouterIntoService;
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use russh::{
    Channel, ChannelId, ChannelMsg, Disconnect,
    client::{self, Config, Handle, Msg, Session, connect_stream},
    keys::{HashAlg, PrivateKeyWithHashAlg, ssh_key},
};
use tokio::io::{AsyncWriteExt, stderr, stdout};
use tracing::{debug, debug_span, info, trace};

use crate::{ClientInput, DEBUG};

/* Russh session and client */

/// User-implemented session type as a helper for interfacing with the SSH protocol.
pub(crate) struct TcpForwardSession(Handle<Client>);

/// User-implemented session type as a helper for interfacing with the SSH protocol.
impl TcpForwardSession {
    /// Attempts to connect to the SSH server. If authentication fails, it returns an error value immediately.
    ///
    /// Our reconnection strategy comes from an iterator which yields `Duration`s. Each one tells us how long to delay
    /// our next reconnection attempt. The function will stop attempting to reconnect once the iterator
    /// stops yielding values.
    pub(crate) async fn connect_password(
        host: &str,
        port: u16,
        login_name: &str,
        password: &str,
        config: Arc<Config>,
        server_fingerprint: &str,
        client_service: TowerToHyperService<RouterIntoService<Incoming>>,
    ) -> Result<Self> {
        let span = debug_span!("TcpForwardSession.connect");
        let _enter = span;
        debug!("TcpForwardSession connecting...");
        let socket = tokio::net::TcpStream::connect((host, port)).await?;
        if let Err(err) = socket.set_nodelay(true) {
            debug!("Failed to set nodelay: {err}");
        }
        match connect_stream(
            Arc::clone(&config),
            socket,
            Client {
                server_fingerprint: Some(server_fingerprint.into()),
                service: client_service,
            },
        )
        .await
        {
            Ok(mut session) => {
                if session
                    .authenticate_password(login_name, password)
                    .await
                    .with_context(|| "Error while authenticating with password.")?
                    .success()
                {
                    debug!("Password authentication succeeded!");
                    Ok(Self(session))
                } else {
                    Err(anyhow!("Password authentication failed."))
                }
            }
            Err(err) => Err(err).with_context(|| "Unable to connect to remote host."),
        }
    }

    pub(crate) async fn connect_key(
        host: &str,
        port: u16,
        login_name: &str,
        key: &PrivateKeyWithHashAlg,
        config: Arc<Config>,
        client_service: TowerToHyperService<RouterIntoService<Incoming>>,
    ) -> Result<Self> {
        let span = debug_span!("TcpForwardSession.connect");
        let _enter = span;
        debug!("TcpForwardSession connecting...");
        let socket = tokio::net::TcpStream::connect((host, port)).await?;
        if let Err(err) = socket.set_nodelay(true) {
            debug!("Failed to set nodelay: {err}");
        }
        match connect_stream(
            Arc::clone(&config),
            socket,
            Client {
                server_fingerprint: None,
                service: client_service,
            },
        )
        .await
        {
            Ok(mut session) => {
                if session
                    .authenticate_publickey(login_name, key.clone())
                    .await
                    .with_context(|| "Error while authenticating with key.")?
                    .success()
                {
                    debug!("Key authentication succeeded!");
                    Ok(Self(session))
                } else {
                    Err(anyhow!("Key authentication failed."))
                }
            }
            Err(err) => Err(err).with_context(|| "Unable to connect to remote host."),
        }
    }

    /// Sends a port forwarding request and opens a session to receive miscellaneous data.
    /// The function yields when the session is broken (for example, if the connection was lost).
    pub(crate) async fn start_forwarding(
        &mut self,
        game_input_tx: Sender<ClientInput>,
    ) -> Result<u32> {
        let span = debug_span!("TcpForwardSession.start");
        let _enter = span;
        let session = &mut self.0;
        let mut channel = session
            .channel_open_session()
            .await
            .with_context(|| "channel_open_session error.")?;
        debug!("Created open session channel.");
        session
            .tcpip_forward("", 80)
            .await
            .with_context(|| "tcpip_forward error.")?;
        debug!("Requested tcpip_forward session.");
        // let mut stdin = stdin();
        let mut stdout = stdout();
        let mut stderr = stderr();
        let code = loop {
            let Some(msg) = channel.wait().await else {
                return Err(anyhow!("Unexpected end of channel."));
            };
            trace!("Got a message through initial session!");
            match msg {
                ChannelMsg::Data { ref data } => {
                    let string = String::from_utf8_lossy(data);
                    if let Some(start) = string.find("https://") {
                        let end = string[start..].find(char::is_whitespace).unwrap();
                        let url = &string[start..start + end];
                        let _ = game_input_tx.send(ClientInput::ConnectionUrl {
                            url: String::from(url),
                        });
                    }
                    if DEBUG {
                        stdout.write_all(data).await?;
                        stdout.flush().await?;
                    }
                }
                ChannelMsg::ExtendedData { ref data, ext: 1 } => {
                    if DEBUG {
                        stderr.write_all(data).await?;
                        stderr.flush().await?;
                    }
                }
                ChannelMsg::Success => (),
                ChannelMsg::Close => break 0,
                ChannelMsg::ExitStatus { exit_status } => {
                    debug!("Exited with code {exit_status}");
                    channel
                        .eof()
                        .await
                        .with_context(|| "Unable to close connection.")?;
                    break exit_status;
                }
                msg => return Err(anyhow!("Unknown message type {:?}.", msg)),
            }
        };
        Ok(code)
    }

    pub async fn close(&mut self) -> Result<()> {
        self.0
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}

/// Our SSH client implementing the `Handler` callbacks for the functions we need to use.
struct Client {
    server_fingerprint: Option<String>,
    service: TowerToHyperService<RouterIntoService<Incoming>>,
}

impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        match &self.server_fingerprint {
            Some(server_fingerprint) => {
                Ok(&server_public_key.fingerprint(HashAlg::Sha256).to_string()
                    == server_fingerprint)
            }
            None => Ok(true),
        }
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let span = debug_span!("server_channel_open_forwarded_tcpip");
        let _enter = span.enter();
        debug!(
            sshid = %String::from_utf8_lossy(session.remote_sshid()),
            connected_address = connected_address,
            connected_port = connected_port,
            originator_address = originator_address,
            originator_port = originator_port,
            "New connection!"
        );
        let hyper_service = self.service.clone();
        tokio::spawn(async move {
            Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(channel.into_stream()), hyper_service)
                .await
                .expect("Invalid request");
        });
        Ok(())
    }

    async fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Received auth banner.");
        let mut stdout = stdout();
        stdout.write_all(banner.as_bytes()).await?;
        stdout.flush().await?;
        Ok(())
    }

    async fn exit_status(
        &mut self,
        channel: ChannelId,
        exit_status: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(channel = ?channel, "exit_status");
        if exit_status == 0 {
            info!("Remote exited with status {}.", exit_status);
        } else {
            info!("Remote exited with status {}.", exit_status);
        }
        Ok(())
    }

    async fn channel_open_confirmation(
        &mut self,
        channel: ChannelId,
        max_packet_size: u32,
        window_size: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(channel = ?channel, max_packet_size, window_size, "channel_open_confirmation");
        Ok(())
    }

    async fn channel_success(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(channel = ?channel, "channel_success");
        Ok(())
    }
}
