//! # WebSocket Client
//!
//! WebSocket connection tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `ws-connect` | `(url -- handle)` | Connect to WebSocket server |
//! | `ws-send` | `(handle message --)` | Send message |
//! | `ws-recv` | `(handle -- message)` | Receive message |
//! | `ws-close` | `(handle --)` | Close connection |
//!
//! ## Example
//!
//! ```kore
//! "wss://api.example.com/ws" ws-connect  -- ( handle )
//! dup { "type": "subscribe", "channel": "updates" } ws-send
//! dup ws-recv  -- ( handle message )
//! swap ws-close
//! ```

use futures_util::{SinkExt, StreamExt};
use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// WebSocket connection handle.
type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Split WebSocket into sender and receiver.
type WsSender = futures_util::stream::SplitSink<WsStream, Message>;
type WsReceiver = futures_util::stream::SplitStream<WsStream>;

/// A WebSocket connection.
struct WsConnection {
    sender: WsSender,
    receiver: WsReceiver,
}

/// Global WebSocket connection store.
static WS_CONNECTIONS: std::sync::OnceLock<RwLock<HashMap<String, WsConnection>>> =
    std::sync::OnceLock::new();

fn connections() -> &'static RwLock<HashMap<String, WsConnection>> {
    WS_CONNECTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register all WebSocket tools into a context.
pub async fn register_websocket_tools(ctx: &mut Context) {
    // ws-connect: (url -- handle)
    ctx.dict.write().await.register(
        Tool::native("ws-connect", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let url = stack.pop()?.into_text()?;

                let (ws_stream, _) = connect_async(&url)
                    .await
                    .map_err(|e| kore::Error::Runtime(format!("WebSocket connect failed: {}", e)))?;

                let (sender, receiver) = ws_stream.split();
                let conn = WsConnection { sender, receiver };

                let handle_id = Uuid::new_v4().to_string();
                connections().write().await.insert(handle_id.clone(), conn);

                stack.push(Value::Text(handle_id))?;
                Ok((stack, ctx))
            })
        }),
    );

    // ws-send: (handle message -- )
    ctx.dict.write().await.register(
        Tool::native("ws-send", "(text text -- )", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let message = stack.pop()?.into_text()?;
                let handle = stack.pop()?.into_text()?;

                let mut conns = connections().write().await;
                let conn = conns
                    .get_mut(&handle)
                    .ok_or_else(|| kore::Error::Runtime(format!("WebSocket not found: {}", handle)))?;

                conn.sender
                    .send(Message::Text(message))
                    .await
                    .map_err(|e| kore::Error::Runtime(format!("WebSocket send failed: {}", e)))?;

                Ok((stack, ctx))
            })
        }),
    );

    // ws-recv: (handle -- message)
    ctx.dict.write().await.register(
        Tool::native("ws-recv", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let handle = stack.pop()?.into_text()?;

                let mut conns = connections().write().await;
                let conn = conns
                    .get_mut(&handle)
                    .ok_or_else(|| kore::Error::Runtime(format!("WebSocket not found: {}", handle)))?;

                let result = match conn.receiver.next().await {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Binary(data))) => {
                        String::from_utf8(data)
                            .map_err(|e| kore::Error::Runtime(format!("Invalid UTF-8: {}", e)))?
                    }
                    Some(Ok(Message::Close(_))) => {
                        return Err(kore::Error::Runtime("WebSocket closed by server".into()));
                    }
                    Some(Err(e)) => {
                        return Err(kore::Error::Runtime(format!("WebSocket receive failed: {}", e)));
                    }
                    None => {
                        return Err(kore::Error::Runtime("WebSocket stream ended".into()));
                    }
                    _ => {
                        return Err(kore::Error::Runtime("Unexpected WebSocket message type".into()));
                    }
                };

                stack.push(Value::Text(result))?;
                Ok((stack, ctx))
            })
        }),
    );

    // ws-close: (handle -- )
    ctx.dict.write().await.register(
        Tool::native("ws-close", "(text -- )", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let handle = stack.pop()?.into_text()?;

                let mut conns = connections().write().await;
                if let Some(mut conn) = conns.remove(&handle) {
                    let _ = conn.sender.close().await;
                }

                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ws_tools_registered() {
        let mut ctx = Context::new();
        register_websocket_tools(&mut ctx).await;

        let names = {
            let dict = ctx.dict.read().await;
            dict.list(&ctx.tenant)
        };

        assert!(names.contains(&"ws-connect".to_string()));
        assert!(names.contains(&"ws-send".to_string()));
        assert!(names.contains(&"ws-recv".to_string()));
        assert!(names.contains(&"ws-close".to_string()));
    }
}
