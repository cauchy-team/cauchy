use std::{net::SocketAddr, sync::Arc};

use dashmap::DashMap;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use rand::{rngs::OsRng, seq::IteratorRandom};

use tower::Service;

use common::*;
use player::peer::*;

#[derive(Clone)]
pub struct Arena {
    peers: Arc<DashMap<SocketAddr, PeerClient>>,
}

impl Default for Arena {
    fn default() -> Self {
        Self {
            peers: Arc::new(DashMap::new()),
        }
    }
}

impl Service<(SocketAddr, PeerClient)> for Arena {
    type Response = ();
    type Error = ();
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (addr, client): (SocketAddr, PeerClient)) -> Self::Future {
        self.peers.insert(addr, client);
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Debug)]
pub enum NewPeerError {
    Preexisting,
    Socket(std::io::Error),
}

impl std::fmt::Display for NewPeerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preexisting => writeln!(f, "attempted to add preexisting peer"),
            Self::Socket(err) => err.fmt(f),
        }
    }
}

impl<T> Service<SampleQuery<T>> for Arena
where
    PeerClient: Service<T>,
    <PeerClient as Service<T>>::Future: Send + 'static,
    <PeerClient as Service<T>>::Error: Send,
    <PeerClient as Service<T>>::Response: Send,
    T: 'static + Clone,
{
    type Response = Vec<(SocketAddr, <PeerClient as Service<T>>::Response)>;
    type Error = MissingStatus;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <PeerClient as Service<GetStatus>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, SampleQuery(req, num): SampleQuery<T>) -> Self::Future {
        // TODO: Remove clone here
        let sample = self.peers.iter().choose_multiple(&mut OsRng, num);

        let collected = sample
            .iter()
            .map(move |reference| reference.pair())
            .map(move |(x, y)| (x.clone(), y.clone()))
            .map(move |(addr, mut peer): (SocketAddr, PeerClient)| {
                peer.call(req.clone()).map_ok(move |res| (addr, res))
            });

        let fut = futures::future::join_all(collected).map(move |collection: Vec<Result<_, _>>| {
            let filtered_collection: Vec<(SocketAddr, <PeerClient as Service<T>>::Response)> =
                collection
                    .into_iter()
                    .filter_map(move |res: Result<_, _>| res.ok())
                    .collect();

            Ok(filtered_collection)
        });
        Box::pin(fut)
    }
}

#[derive(Debug)]
pub enum DirectedError<E> {
    Internal(E),
    Missing,
}

impl<T> Service<DirectedQuery<T>> for Arena
where
    PeerClient: Service<T>,
    <PeerClient as Service<T>>::Future: Send,
    T: 'static + Send + Sized,
{
    type Response = <PeerClient as Service<T>>::Response;
    type Error = DirectedError<<PeerClient as Service<T>>::Error>;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <PeerClient as Service<T>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(DirectedError::Internal(err))),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, DirectedQuery(addr, request): DirectedQuery<T>) -> Self::Future {
        let peers = self.peers.clone();
        let fut = async move {
            let mut_client = peers.get(&addr).map(|some| some.value().clone());
            match mut_client {
                Some(mut some) => some.call(request).map_err(DirectedError::Internal).await,
                None => Err(DirectedError::Missing),
            }
        };

        Box::pin(fut)
    }
}

impl<T> Service<AllQuery<T>> for Arena
where
    PeerClient: Service<T>,
    <PeerClient as Service<T>>::Future: Send + 'static,
    <PeerClient as Service<T>>::Error: Send,
    <PeerClient as Service<T>>::Response: Send,
    T: 'static + Clone,
{
    type Response = std::collections::HashMap<SocketAddr, <PeerClient as Service<T>>::Response>;
    type Error = DirectedError<<PeerClient as Service<T>>::Error>;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <PeerClient as Service<T>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(DirectedError::Internal(err))),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, AllQuery(request): AllQuery<T>) -> Self::Future {
        // TODO: Remove clone here
        let peers = self.peers.clone();
        let collected = peers
            .iter_mut()
            .map(move |reference| {
                let (x, y) = reference.pair();
                (x.clone(), y.clone())
            })
            .map(move |(addr, mut peer)| peer.call(request.clone()).map_ok(move |res| (addr, res)));

        let fut = futures::future::join_all(collected).map(move |collection: Vec<Result<_, _>>| {
            let filtered_collection: std::collections::HashMap<
                SocketAddr,
                <PeerClient as Service<T>>::Response,
            > = collection
                .into_iter()
                .filter_map(move |res: Result<_, _>| res.ok())
                .collect();

            Ok(filtered_collection)
        });
        Box::pin(fut)
    }
}
