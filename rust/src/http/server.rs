use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures::StreamExt;
use serde::Deserialize;
use tokio::{sync::mpsc::channel, task::JoinHandle, time::sleep};

use crate::{
    ClientInput, GameInput, GameMessage,
    http::{
        markup::{
            hide_notification, ping_span, ping_span_ack, prize_notification,
            resume_game_notification, show_notification, text_notification,
        },
        router::AppState,
    },
};

/* WebSocket handling */

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WebSocketEvent {
    Command(GameInput),
    Network(WebSocketNetworkEvent),
    Interface(WebSocketInterfaceEvent),
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WebSocketNetworkEvent {
    Ping { ping: u64 },
    PingAck { ping: u64, pong: u64, ping_ack: u64 },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WebSocketInterfaceEvent {
    ResumeGame,
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    if state.is_connected.swap(true, Ordering::AcqRel) {
        return;
    }
    let state_rx = state.rx.lock().await;
    let mut game_rx = state_rx.resubscribe();
    drop(state_rx);
    let (clean_notifications_tx, mut clean_notifications_rx) = channel::<()>(1);
    let mut clean_notifications_timer: Option<JoinHandle<()>> = None;
    let _ = socket
        .send(Message::Text(
            resume_game_notification().into_string().into(),
        ))
        .await;
    loop {
        tokio::select! {
            _ = clean_notifications_rx.recv() => {
                if socket
                    .send(Message::Text(hide_notification().into_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            game_message = game_rx.recv() => {
                match game_message {
                    Ok(message) => {
                        match message {
                            GameMessage::DisplayText { text } => {
                                if let Some(handle) = clean_notifications_timer.take() {
                                    handle.abort();
                                }
                                if socket.send(Message::Text(text_notification(&text).into_string().into())).await.is_err() {
                                    break;
                                }
                                if socket.send(Message::Text(show_notification().into_string().into())).await.is_err() {
                                    break;
                                }
                                let tx = clean_notifications_tx.clone();
                                clean_notifications_timer = Some(tokio::spawn(async move {
                                    sleep(Duration::from_secs(4)).await;
                                    let _ = tx.send(()).await;
                                }));
                            },
                            GameMessage::CollectPrize{ prize } => {
                                if let Some(handle) = clean_notifications_timer.take() {
                                    handle.abort();
                                }
                                if socket.send(Message::Text(prize_notification(&prize).into_string().into())).await.is_err() {
                                    break;
                                }
                                if socket.send(Message::Text(show_notification().into_string().into())).await.is_err() {
                                    break;
                                }
                                let tx = clean_notifications_tx.clone();
                                clean_notifications_timer = Some(tokio::spawn(async move {
                                    sleep(Duration::from_secs(4)).await;
                                    let _ = tx.send(()).await;
                                }));
                            },
                        }
                    },
                    Err(_) => break,
                }
            }
            socket_message = socket.next() => {
                match socket_message {
                    Some(message) => {
                        if let Ok(Message::Text(text)) = message {
                            match serde_json::from_str::<WebSocketEvent>(&text) {
                                Ok(WebSocketEvent::Command(game_input)) => {
                                    if state.tx.send(ClientInput::Command(game_input)).is_err() {
                                        return;
                                    }
                                }
                                Ok(WebSocketEvent::Interface(interface_event)) => match interface_event {
                                    WebSocketInterfaceEvent::ResumeGame => {
                                        if state.tx.send(ClientInput::Connected).is_err() {
                                            return;
                                        }
                                        if socket
                                            .send(Message::Text(hide_notification().into_string().into()))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    },
                                }
                                Ok(WebSocketEvent::Network(network_event)) => match network_event {
                                    WebSocketNetworkEvent::Ping { ping } => {
                                        let pong = Instant::now().duration_since(state.startup).as_millis() as u64;
                                        if socket
                                            .send(Message::Text(
                                                ping_span_ack(state.latency.load(Ordering::Acquire), ping, pong)
                                                    .into_string().into(),
                                            ))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        };
                                    }
                                    WebSocketNetworkEvent::PingAck {
                                        ping,
                                        pong,
                                        ping_ack,
                                    } => {
                                        let pong_ack = Instant::now().duration_since(state.startup).as_millis() as u64;
                                        let latency = (ping_ack - ping) - ((pong_ack - pong) / 2);
                                        state.latency.store(latency, Ordering::Release);
                                        if socket
                                            .send(Message::Text(ping_span(latency).into_string().into()))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        };
                                    }
                                }
                                Err(_) => println!("unknown {}", text),
                            }
                        }
                    },
                    None => break,
                }
            }
        }
    }
    let _ = state.tx.send(ClientInput::Disconnected);
    state.is_connected.store(false, Ordering::Release);
}
