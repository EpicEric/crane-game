use std::sync::atomic::Ordering;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use serde_json::json;

use crate::DEBUG;

use super::router::AppState;

/* Main page elements */

static STYLE: &str = include_str!("./style.css");

static SCRIPT: &str = include_str!("./script.js");

pub(crate) async fn index_handler(State(state): State<AppState>) -> Response {
    if state.is_connected.load(Ordering::Acquire) {
        return (StatusCode::BAD_REQUEST, "Already connected.").into_response();
    }
    html! {
        (DOCTYPE)
        head {
            meta charset="utf-8";
            title { "Crane Game" }
            meta name="viewport" content="width=device-width,initial-scale=1" {}
            script src="/assets/htmx.js" {}
            script src="/assets/htmx-ws.js" {}
            script src="/assets/alpine.js" defer {}
            link rel="stylesheet" href="/assets/water.css" {}
            style { (PreEscaped(STYLE)) }
            script { (PreEscaped(SCRIPT)) }
        }
        body {
            main hx-ext="ws" ws-connect="/ws" {
                #ping-infobox hidden[!DEBUG] {
                    label for="ping" { "Ping" }
                    (ping_span(0))
                }
                #controls {
                    (button("button-up", "move_up", html! {
                        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="44px" {
                            path fill-rule="evenodd" d="M11.47 7.72a.75.75 0 0 1 1.06 0l7.5 7.5a.75.75 0 1 1-1.06 1.06L12 9.31l-6.97 6.97a.75.75 0 0 1-1.06-1.06l7.5-7.5Z" clip-rule="evenodd";
                        }
                    }))
                    (button("button-left", "move_left", html! {
                        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="44px" {
                            path fill-rule="evenodd" d="M7.72 12.53a.75.75 0 0 1 0-1.06l7.5-7.5a.75.75 0 1 1 1.06 1.06L9.31 12l6.97 6.97a.75.75 0 1 1-1.06 1.06l-7.5-7.5Z" clip-rule="evenodd";
                        }
                    }))
                    (button("button-center", "deploy_claw", html!("DEPLOY!")))
                    (button("button-right", "move_right", html! {
                        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="44px" {
                            path fill-rule="evenodd" d="M16.28 11.47a.75.75 0 0 1 0 1.06l-7.5 7.5a.75.75 0 0 1-1.06-1.06L14.69 12 7.72 5.03a.75.75 0 0 1 1.06-1.06l7.5 7.5Z" clip-rule="evenodd";
                        }
                    }))
                    (button("button-down", "move_down", html! {
                        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="44px" {
                            path fill-rule="evenodd" d="M12.53 16.28a.75.75 0 0 1-1.06 0l-7.5-7.5a.75.75 0 0 1 1.06-1.06L12 14.69l6.97-6.97a.75.75 0 1 1 1.06 1.06l-7.5 7.5Z" clip-rule="evenodd";
                        }
                    }))
                }
                #preload aria-hidden="true" {
                    img src="/assets/snowman.webp";
                    img src="/assets/duende.webp";
                    img src="/assets/cat.webp";
                }
                #notification-infobox {
                    #notification-state .show {}
                    #notification {
                        "Connecting..."
                    }
                }
                script #extra-script {}
            }
        }
    }
    .into_response()
}

/* HTMX components */

fn button(id: &str, action: &str, content: Markup) -> Markup {
    html! {
        div id=(id) x-data="{ pressed: false }" {
            button x-bind:class="pressed && 'pressed'" x-on:pointerdown="pressed = true" x-on:pointerup="pressed = false" x-on:pointerleave="pressed = false" {
                (content)
            }
            div hx-vals=(json!({"type": "ButtonPress", "action": action})) hx-trigger=(format!("pointerdown from:#{id}")) ws-send {}
            div hx-vals=(json!({"type": "ButtonRelease", "action": action})) hx-trigger=(format!("pointerup from:#{id}, pointerleave from:#{id}")) ws-send {}
        }
    }
}

pub(crate) fn ping_span(latency: u64) -> Markup {
    html! {
        span #ping name="ping" hx-vals=r#"js:{type: "Ping", ping: getTimestamp()}"# hx-trigger=(
            format!("load delay:{}", if latency > 0 {"5000ms"} else {"2000ms"})
        ) ws-send {
            @if latency > 0 {
                (latency) "ms"
            }
        }
    }
}

pub(crate) fn ping_span_ack(latency: u64, ping: u64, pong: u64) -> Markup {
    html! {
        span #ping name="ping" hx-vals=(format!(r#"js:{{type: "PingAck", ping: {ping}, pong: {pong}, ping_ack: getTimestamp()}}"#)) hx-trigger="load" ws-send {
            @if latency > 0 {
                (latency) "ms"
            }
        }
    }
}
/* Notifications */

pub(crate) fn hide_notification() -> Markup {
    html! {
        #notification-state .hide {}
    }
}

pub(crate) fn show_notification() -> Markup {
    html! {
        #notification-state .show {}
    }
}

pub(crate) fn text_notification(text: &str) -> Markup {
    html! {
        #notification {
            (text)
        }
    }
}

pub fn prize_notification(image: &str) -> Markup {
    html! {
        #notification .prize {
            ul .starburst-wheel {
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
                li {}
            }
            img src={"/assets/"(image)".webp"};
        }
    }
}

pub(crate) fn resume_game_notification() -> Markup {
    html! {
        #notification {
            button #resume-game-button hx-vals=(json!({"type": "ResumeGame"})) hx-trigger="pointerdown from:#resume-game-button" ws-send {
                "Play"
            }
        }
    }
}
