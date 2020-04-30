use std::{sync::Arc, collections::HashMap, net::SocketAddr};

use futures::prelude::*;
use parking_lot::RwLock;
use network::{NetworkHandles, FramedConnection};
use arena::Arena;

use crate::peers::SharedPeerHandle;

pub struct Manager {
    peers: Arc<RwLock<HashMap<SocketAddr, SharedPeerHandle>>>,
    network_handles: NetworkHandles,
}

impl Manager {
    fn new(network_handles: NetworkHandles) -> Self {
        Manager {
            peers: Default::default(),
            network_handles,
        }
    }

    fn remove(&self, addr: &SocketAddr) -> Option<SharedPeerHandle> {
        self.peers.write().remove(addr)
    }

    async fn add_framed_connection_stream<S: Stream<Item = FramedConnection> + Unpin>(
        framed_conn_stream: S,
    ) {
        while let Some(framed_connection) = framed_conn_stream.next().await {}
    }

    async fn add_disconnect_stream<S: Stream<Item = SocketAddr>>() {}
}

impl Arena<SocketAddr, SharedPeerHandle> for Manager {
    fn get_player(&self, addr: &SocketAddr) -> Option<SharedPeerHandle> {
        self.peers.read().get(addr).cloned()
    }

    fn players_map(&self) -> Arc<RwLock<HashMap<SocketAddr, SharedPeerHandle>>> {
        self.peers.clone()
    }
}
