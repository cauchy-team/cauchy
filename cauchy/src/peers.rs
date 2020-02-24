use futures::{channel::mpsc, prelude::*};

use parking_lot::RwLock;
use std::convert::TryInto;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use crate::ego::{EgoHandle, SharedEgoHandle};
use arena::{Player, Position, State as ArenaState};
use network::{FramedConnection, Message};

const MESSAGE_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct SharedPeerHandle(Arc<PeerHandle>);

pub struct PeerHandle {
    start_time: Instant,
    message_sink: mpsc::Sender<Result<Message, ()>>,
    position: Arc<RwLock<Position>>,
}

impl Player for SharedPeerHandle {
    fn get_position(&self) -> Arc<RwLock<Position>> {
        self.0.position.clone()
    }
}

pub enum PeerError {
    Terminated,
    UnexpectedStatus,
    Decoding(network::codec::DecodeError),
    Encoding(network::codec::EncodingError),
}

impl PeerHandle {
    pub fn start(
        framed_connection: FramedConnection,
        self_handle: SharedEgoHandle,
    ) -> (SocketAddr, SharedPeerHandle) {
        let start_time = framed_connection.start_time;
        let addr = framed_connection.addr;
        let (message_sink, message_recv) = mpsc::channel(MESSAGE_CHANNEL_CAPACITY);
        let peer_handle = SharedPeerHandle(Arc::new(PeerHandle {
            start_time,
            message_sink,
            position: Arc::new(RwLock::new(Position::default())),
        }));
        let peer_handle_outer = peer_handle.clone();

        let (framed_sink, framed_stream) = framed_connection.framed.split();
        let responses_stream = framed_stream.filter_map(move |result| {
            let peer_handle = peer_handle.clone();
            let self_handle = self_handle.clone();
            async move {
                let msg = match result {
                    Ok(msg) => msg,
                    Err(err) => return Some(Err(PeerError::Decoding(err))),
                };
                match msg {
                    Message::Poll => {
                        let self_mut = self_handle.write();

                        let status = self_mut.status();
                        let perception = self_mut.perception();
                        peer_handle.get_position().write().perception = Some(perception);

                        Some(Ok(Message::Status(status)))
                    }
                    Message::Status(status) => {
                        let position = peer_handle.get_position();

                        // Check if in polling state
                        if position.read().state == ArenaState::Idle {
                            // TODO: Error
                            return Some(Err(PeerError::UnexpectedStatus));
                        }

                        let mut position_mut = position.write();

                        // Add the poll response to peer
                        let entry = consensus::Entry {
                            oddsketch: status.oddsketch[..]
                                .try_into()
                                .expect("unexpected oddsketch length"),
                            mass: 1, // TODO
                        };
                        let root = status.root[..].try_into().expect("unexpected root length");
                        position_mut.poll_response = Some(arena::PollResponse { entry, root });
                        position_mut.state = ArenaState::Idle;
                        None
                    }
                    Message::Reconcile => {
                        // let mut position = peer_handle.get_mut_position();
                        None
                    }
                    _ => unreachable!(),
                }
            }
        });

        let select_stream = stream::select(
            responses_stream,
            message_recv.map_err(|_| PeerError::Terminated),
        )
        .inspect_err(|err| {
            println!("found error");
        });
        let framed_sink = framed_sink.sink_map_err(PeerError::Encoding);
        let event_loop = select_stream.forward(framed_sink);
        tokio::spawn(event_loop);
        (addr, peer_handle_outer)
    }
}
