pub mod entrypoint;
pub mod http;
pub mod qr_code;
pub mod ssh;

use std::collections::HashSet;
use std::sync::mpsc::Receiver;

use entrypoint::ssh_entrypoint;
use godot::classes::{Engine, Input, InputEventAction, Node};
use godot::global::randi;
use godot::prelude::*;
use http::{ROUTER, router::get_router};
use qr_code::QrCodeSingleton;
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinHandle;

pub const DEBUG: bool = false;
pub const AUDIENCE: &str = "crane-game@v1";

pub(crate) enum ClientInput {
    ConnectionUrl { url: String },
    Connected,
    Disconnected,
    Command(GameInput),
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(crate) enum GameInput {
    ButtonPress { action: String },
    ButtonRelease { action: String },
    ButtonStrength { action: String, strength: f32 },
    SubmitText { text: String },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
pub enum GameMessage {
    CollectPrize { prize: String },
    DisplayText { text: String },
}

struct CraneGameExtension;

#[gdextension]
unsafe impl ExtensionLibrary for CraneGameExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().register_singleton("QrCode", &QrCodeSingleton::new_alloc());
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            let mut engine = Engine::singleton();
            let singleton_name = "QrCode";

            if let Some(_singleton) = engine.get_singleton(singleton_name) {
                engine.unregister_singleton(singleton_name);
                // _singleton.free();
            }
        }
    }
}

#[derive(GodotClass)]
#[class(base=Node)]
struct NetworkConnection {
    base: Base<Node>,
    uuid_bytes: uuid::Bytes,
    runtime: Runtime,
    server_handle: Option<JoinHandle<()>>,
    actions: HashSet<String>,
    connections: i32,
    game_input_rx: Option<Receiver<ClientInput>>,
    game_message_tx: Option<Sender<GameMessage>>,
}

#[godot_api]
impl NetworkConnection {
    #[signal]
    fn client_connected();

    #[signal]
    fn client_disconnected();

    #[signal]
    fn connection_url(url: GString);

    #[signal]
    fn text(text: GString);

    #[func]
    fn start(&mut self) {
        if self.server_handle.is_some() {
            godot_error!("Cannot start server: Already started");
            return;
        }

        let (game_input_tx, game_input_rx) = ::std::sync::mpsc::channel();
        let (game_message_tx, game_message_rx) = ::tokio::sync::broadcast::channel(16);
        let bytes = self.uuid_bytes;
        self.server_handle = Some(self.runtime.spawn(async move {
            ROUTER
                .set(get_router(game_input_tx.clone(), game_message_rx).await)
                .unwrap();
            ssh_entrypoint(
                "eric.dev.br",
                443,
                &uuid::Builder::from_random_bytes(bytes)
                    .into_uuid()
                    .hyphenated()
                    .to_string(),
                AUDIENCE,
                Some(::russh::keys::PrivateKeyWithHashAlg::new(
                    ::std::sync::Arc::new(
                        ::russh::keys::PrivateKey::from_openssh(include_str!(
                            "../credentials/id_ed25519"
                        ))
                        .unwrap(),
                    ),
                    Some(::russh::keys::HashAlg::Sha256),
                )),
                game_input_tx,
            )
            .await
            .unwrap()
        }));
        self.game_input_rx = Some(game_input_rx);
        self.game_message_tx = Some(game_message_tx);
    }

    #[func]
    fn stop(&mut self) {
        let Some(handle) = self.server_handle.take() else {
            godot_error!("Cannot stop server: Already stopped");
            return;
        };
        handle.abort();
        self.game_input_rx = None;
        self.game_message_tx = None;
    }

    #[func]
    fn send_data(&mut self, json: String) {
        if let (Ok(message), Some(tx)) = (
            serde_json::from_str::<GameMessage>(&json),
            self.game_message_tx.as_ref(),
        ) {
            let _ = tx.send(message);
        } else {
            godot_error!("Attempted to send unknown data: {json}");
        }
    }
}

#[godot_api]
impl INode for NetworkConnection {
    fn init(base: Base<Node>) -> Self {
        let uuid_bytes: uuid::Bytes = (0..4)
            .flat_map(|_| {
                let randi = randi();
                [
                    (randi >> 24) as u8,
                    (randi >> 16) as u8,
                    (randi >> 8) as u8,
                    (randi) as u8,
                ]
            })
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        Self {
            base,
            uuid_bytes,
            runtime,
            server_handle: None,
            actions: Default::default(),
            connections: 0,
            game_input_rx: None,
            game_message_tx: None,
        }
    }

    fn process(&mut self, _delta: f64) {
        while let Some(Ok(message)) = self.game_input_rx.as_ref().map(|rx| rx.try_recv()) {
            match message {
                ClientInput::ConnectionUrl { url } => {
                    self.base_mut()
                        .emit_signal("connection_url", &[url.to_variant()]);
                }
                ClientInput::Connected => {
                    if self.connections == 0 {
                        self.base_mut().emit_signal("client_connected", &[]);
                    }
                    self.connections += 1;
                }
                ClientInput::Disconnected => {
                    self.connections -= 1;
                    if self.connections == 0 {
                        self.base_mut().emit_signal("client_disconnected", &[]);
                    }
                    for action in self.actions.drain() {
                        let mut event_action = InputEventAction::new_gd();
                        event_action.set_action(&action);
                        event_action.set_pressed(false);
                        Input::singleton().parse_input_event(&event_action);
                    }
                }
                ClientInput::Command(game_input) => match game_input {
                    GameInput::ButtonPress { action } => {
                        let mut event_action = InputEventAction::new_gd();
                        event_action.set_action(&action);
                        event_action.set_pressed(true);
                        Input::singleton().parse_input_event(&event_action);
                        self.actions.insert(action);
                    }
                    GameInput::ButtonRelease { action } => {
                        let mut event_action = InputEventAction::new_gd();
                        event_action.set_action(&action);
                        event_action.set_pressed(false);
                        Input::singleton().parse_input_event(&event_action);
                    }
                    GameInput::ButtonStrength { action, strength } => {
                        let mut event_action = InputEventAction::new_gd();
                        event_action.set_action(&action);
                        event_action.set_strength(strength);
                        Input::singleton().parse_input_event(&event_action);
                    }
                    GameInput::SubmitText { text } => {
                        self.base_mut().emit_signal("text", &[text.to_variant()]);
                    }
                },
            }
        }
    }
}
