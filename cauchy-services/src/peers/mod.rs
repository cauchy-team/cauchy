pub mod client;
pub mod player;
pub mod server;

use std::{collections::HashMap, net::SocketAddr, pin::Pin, sync::Arc, time::Instant};

use futures::channel::mpsc;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::*;
use rand::{rngs::OsRng, seq::IteratorRandom};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tower::Service;

pub struct Marker;
pub struct Minisketch;

pub struct GetStatus;

pub struct MissingStatus;

pub struct GetMetadata;

pub struct Metadata {
    start_time: Instant,
    addr: SocketAddr,
}

pub struct PeerFactory {
    player: player::Player,
}

pub enum HandleError {
    Socket(std::io::Error),
}

const BUFFER_SIZE: usize = 128;

pub struct PeerHandle {
    server: server::PeerServer,
    client: client::PeerClient,
    transport: server::Transport,
}

impl Service<TcpStream> for PeerFactory {
    type Response = PeerHandle;
    type Error = HandleError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TcpStream) -> Self::Future {
        let addr = match req.peer_addr() {
            Ok(ok) => ok,
            Err(err) => return Box::pin(async move { Err(HandleError::Socket(err)) }),
        };

        let codec = MessageCodec::default();
        let framed = Framed::new(req, codec);

        let (response_sink, response_stream) = mpsc::channel(BUFFER_SIZE);
        let (request_sink, request_stream) = mpsc::channel(BUFFER_SIZE);

        let transport = server::Transport::new(framed, request_stream);
        let server = server::PeerServer::new(response_sink, self.player.clone());

        let metadata = Arc::new(Metadata {
            start_time: Instant::now(),
            addr,
        });
        let client = client::PeerClient::new(metadata, request_sink, response_stream);
        let fut = async move {
            Ok(PeerHandle {
                server,
                client,
                transport,
            })
        };
        Box::pin(fut)
    }
}

pub struct Arena {
    peers: HashMap<SocketAddr, client::PeerClient>,
}

impl Service<client::PeerClient> for Arena {
    type Response = ();
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, peer_client: client::PeerClient) -> Self::Future {
        let metadata = peer_client.get_metadata();
        self.peers.insert(metadata.addr.clone(), peer_client);
        Box::pin(async { Ok(()) })
    }
}

pub struct GetAllMetadata;

impl Service<GetAllMetadata> for Arena {
    type Response = Vec<Arc<Metadata>>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.values_mut() {
            match <client::PeerClient as Service<GetMetadata>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: GetAllMetadata) -> Self::Future {
        let calls = self.peers.values_mut().map(|peer| peer.call(GetMetadata));
        Box::pin(futures::future::try_join_all(calls))
    }
}

pub struct PollSample(usize);

impl Service<PollSample> for Arena {
    type Response = Vec<(SocketAddr, Status)>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.values_mut() {
            match <client::PeerClient as Service<GetMetadata>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, PollSample(num): PollSample) -> Self::Future {
        // TODO: Remove clone here
        let sample = self
            .peers
            .clone()
            .into_iter()
            .map(move |(addr, peer)| (addr, peer))
            .choose_multiple(&mut OsRng, num);

        let collected = sample.into_iter().map(move |(addr, mut peer)| {
            peer.call(client::GetStatus).map_ok(move |res| (addr, res))
        });

        let fut = futures::future::join_all(collected).map(move |collection: Vec<Result<_, _>>| {
            let filtered_collection: Vec<(SocketAddr, Status)> = collection
                .into_iter()
                .filter_map(move |res: Result<_, _>| res.ok())
                .collect();

            Ok(filtered_collection)
        });
        Box::pin(fut)
    }
}

pub enum DirectedError<E> {
    Internal(E),
    Missing,
}

pub struct DirectedQuery<T>(SocketAddr, T);

impl<T: 'static> Service<DirectedQuery<T>> for Arena
where
    client::PeerClient: Service<T>,
{
    type Response = <client::PeerClient as Service<T>>::Response;
    type Error = DirectedError<<client::PeerClient as Service<T>>::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.values_mut() {
            match <client::PeerClient as Service<T>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(DirectedError::Internal(err))),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, DirectedQuery(addr, request): DirectedQuery<T>) -> Self::Future {
        let mut_client: Option<client::PeerClient> = self.peers.get(&addr).cloned();
        match mut_client {
            Some(mut some) => {
                Box::pin(async move { some.call(request).map_err(DirectedError::Internal).await })
            }
            None => Box::pin(async move { Err(DirectedError::Missing) }),
        }
    }
}
