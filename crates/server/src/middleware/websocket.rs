use std::{
    borrow::Cow,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    extract::{
        FromRequestParts,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::request::Parts,
    response::IntoResponse,
};
use futures_util::{Sink, SinkExt, Stream, StreamExt};

pub struct LocalWsUpgrade {
    ws: WebSocketUpgrade,
}

impl<S> FromRequestParts<S> for LocalWsUpgrade
where
    S: Send + Sync,
{
    type Rejection = axum::extract::ws::rejection::WebSocketUpgradeRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ws = WebSocketUpgrade::from_request_parts(parts, state).await?;
        Ok(Self { ws })
    }
}

impl LocalWsUpgrade {
    pub fn protocols<I>(mut self, protocols: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Cow<'static, str>>,
    {
        self.ws = self.ws.protocols(protocols);
        self
    }

    pub fn on_upgrade<F, Fut>(self, callback: F) -> impl IntoResponse
    where
        F: FnOnce(LocalWebSocket) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.ws
            .on_upgrade(move |socket| callback(LocalWebSocket { inner: socket }))
    }
}

pub struct LocalWebSocket {
    inner: WebSocket,
}

impl LocalWebSocket {
    pub async fn send(&mut self, message: Message) -> anyhow::Result<()> {
        SinkExt::send(&mut self.inner, message)
            .await
            .map_err(Into::into)
    }
    pub async fn recv(&mut self) -> anyhow::Result<Option<Message>> {
        self.inner.next().await.transpose().map_err(Into::into)
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        SinkExt::close(&mut self.inner).await.map_err(Into::into)
    }
}
impl Stream for LocalWebSocket {
    type Item = Result<Message, anyhow::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner)
            .poll_next(cx)
            .map(|opt| opt.map(|r| r.map_err(Into::into)))
    }
}
impl Sink<Message> for LocalWebSocket {
    type Error = anyhow::Error;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner)
            .poll_ready(cx)
            .map_err(Into::into)
    }
    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.get_mut().inner)
            .start_send(item)
            .map_err(Into::into)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner)
            .poll_flush(cx)
            .map_err(Into::into)
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.get_mut().inner)
            .poll_close(cx)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_socket_roundtrips_text_and_binary_without_relay_envelopes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/ws",
            axum::routing::get(|upgrade: LocalWsUpgrade| async move {
                upgrade.on_upgrade(|mut socket| async move {
                    while let Ok(Some(message)) = socket.recv().await {
                        if matches!(message, Message::Close(_)) {
                            break;
                        }
                        if socket.send(message).await.is_err() {
                            break;
                        }
                    }
                })
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
                .await
                .unwrap();
            use tokio_tungstenite::tungstenite::Message as ClientMessage;
            for message in [
                ClientMessage::Text("terminal output".into()),
                ClientMessage::Binary(vec![0, 1, 255].into()),
            ] {
                socket.send(message.clone()).await.unwrap();
                assert_eq!(socket.next().await.unwrap().unwrap(), message);
            }
            socket.close(None).await.unwrap();
        })
        .await;
        server.abort();
        let _ = server.await;
        result.expect("local WebSocket roundtrip timed out");
    }
}
