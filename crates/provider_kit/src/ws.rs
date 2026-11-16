//! Blocking WebSocket client with heartbeats and bounded reads.
//!
//! The runtime drives the session loop; this client only moves frames. Ping
//! frames are answered automatically by the underlying protocol layer.

use aeris_provider_api::ProviderError;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket as TungsteniteSocket};

/// One inbound frame the session loop may act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WsMessage {
    Text(String),
    Close,
}

/// A connected WebSocket with a read deadline per receive.
pub struct WebSocket {
    socket: TungsteniteSocket<MaybeTlsStream<TcpStream>>,
}

impl WebSocket {
    /// Connects with a bounded handshake.
    ///
    /// # Errors
    ///
    /// Returns `Transport` when the connection cannot be established.
    pub fn connect(url: &str, timeout: Duration) -> Result<Self, ProviderError> {
        let (socket, _response) = tungstenite::connect(url)
            .map_err(|error| ProviderError::Transport(error.to_string()))?;
        let stream = match socket.get_ref() {
            MaybeTlsStream::Plain(stream) => Some(stream),
            MaybeTlsStream::Rustls(stream) => Some(stream.get_ref()),
            _ => None,
        };
        if let Some(stream) = stream {
            stream
                .set_read_timeout(Some(timeout.min(Duration::from_millis(50))))
                .map_err(|error| ProviderError::Transport(error.to_string()))?;
            stream
                .set_nodelay(true)
                .map_err(|error| ProviderError::Transport(error.to_string()))?;
        }
        Ok(Self { socket })
    }

    /// Sends one text frame.
    ///
    /// # Errors
    ///
    /// Returns `Transport` when the frame cannot be written.
    pub fn send_text(&mut self, text: &str) -> Result<(), ProviderError> {
        self.socket
            .send(Message::Text(text.to_string().into()))
            .map_err(|error| ProviderError::Transport(error.to_string()))
    }

    /// Sends a protocol ping.
    ///
    /// # Errors
    ///
    /// Returns `Transport` when the frame cannot be written.
    pub fn ping(&mut self) -> Result<(), ProviderError> {
        self.socket
            .send(Message::Ping(Vec::new().into()))
            .map_err(|error| ProviderError::Transport(error.to_string()))
    }

    /// Receives the next text or close frame, or `None` at the deadline.
    /// Protocol pings are answered and skipped.
    ///
    /// # Errors
    ///
    /// Returns `Transport` on a broken connection and `Closed` on a clean
    /// provider close.
    pub fn recv(&mut self, deadline: Instant) -> Result<Option<WsMessage>, ProviderError> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => return Ok(Some(WsMessage::Text(text.to_string()))),
                Ok(Message::Close(_)) => return Ok(Some(WsMessage::Close)),
                Ok(Message::Ping(payload)) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .map_err(|error| ProviderError::Transport(error.to_string()))?;
                }
                Ok(_) => {}
                Err(tungstenite::Error::ConnectionClosed) => {
                    return Err(ProviderError::Closed);
                }
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                }
                Err(error) => return Err(ProviderError::Transport(error.to_string())),
            }
        }
    }

    /// Closes the connection politely, ignoring a peer that already left.
    pub fn close(&mut self) {
        let _ = self.socket.close(None);
    }
}
