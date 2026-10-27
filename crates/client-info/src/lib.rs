use std::{net::SocketAddr, sync::OnceLock};

/// Runtime information about the local server.
#[derive(Clone)]
pub struct ClientInfo {
    server_addr: OnceLock<SocketAddr>,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientInfo {
    pub fn new() -> Self {
        Self {
            server_addr: OnceLock::new(),
        }
    }

    pub fn set_server_addr(&self, addr: SocketAddr) -> Result<(), String> {
        self.server_addr
            .set(addr)
            .map_err(|_| "server address already set".to_string())
    }

    pub fn get_server_addr(&self) -> Option<SocketAddr> {
        self.server_addr.get().copied()
    }
}
