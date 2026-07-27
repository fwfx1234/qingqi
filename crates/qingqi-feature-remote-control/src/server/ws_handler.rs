use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;

use crate::server::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::Extension(state): axum::extract::Extension<AppState>,
) -> impl IntoResponse {
    tracing::info!("[远程控制] WebSocket 连接升级请求");
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("[远程控制] WebSocket 已连接");
    
    let mut rx = state.events.subscribe();

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if socket.send(Message::Text(json)).await.is_err() {
                    tracing::warn!("[远程控制] WebSocket 发送失败，断开连接");
                    break;
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Close(_) => {
                        tracing::info!("[远程控制] WebSocket 客户端断开");
                        break;
                    }
                    Message::Ping(data) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            tracing::warn!("[远程控制] WebSocket Pong 发送失败");
                            break;
                        }
                    }
                    Message::Text(text) => {
                        tracing::debug!("[远程控制] WebSocket 收到消息: {}", text);
                    }
                    _ => {}
                }
            }
            else => break,
        }
    }
    
    tracing::info!("[远程控制] WebSocket 连接已关闭");
}
