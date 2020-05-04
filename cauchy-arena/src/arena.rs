use std::{net::SocketAddr, pin::Pin, sync::Arc};

use dashmap::DashMap;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::Status;
use rand::{rngs::OsRng, seq::IteratorRandom};
use tower::Service;

use super::*;
use crate::peer::*;

#[derive(Clone, Default)]
pub struct Arena {
    peers: Arc<DashMap<SocketAddr, Peer>>,
}

#[derive(Debug)]
pub enum NewPeerError {
    Preexisting,
}

impl std::fmt::Display for NewPeerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preexisting => writeln!(f, "attempted to add preexisting peer"),
        }
    }
}

impl std::error::Error for NewPeerError {}

impl Service<Peer> for Arena {
    type Response = ();
    type Error = NewPeerError;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, peer: Peer) -> Self::Future {
        let metadata = peer.get_metadata();

        let addr = metadata.addr.clone();
        if self.peers.contains_key(&addr) {
            return Box::pin(async move { Err(NewPeerError::Preexisting) });
        }

        self.peers.insert(addr, peer);
        Box::pin(async move { Ok(()) })
    }
}

pub struct GetAllMetadata;

impl Service<GetAllMetadata> for Arena {
    type Response = Vec<Arc<Metadata>>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <Peer as Service<GetMetadata>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: GetAllMetadata) -> Self::Future {
        let calls = self.peers.iter_mut().map(|mut peer| peer.call(GetMetadata));
        Box::pin(futures::future::try_join_all(calls))
    }
}

pub struct PollSample(usize);

impl Service<PollSample> for Arena {
    type Response = Vec<(SocketAddr, Status)>;
    type Error = ();
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <Peer as Service<GetMetadata>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, PollSample(num): PollSample) -> Self::Future {
        // TODO: Remove clone here
        let sample = self.peers.iter().choose_multiple(&mut OsRng, num);

        let collected = sample
            .iter()
            .map(move |reference| reference.pair())
            .map(move |(x, y)| (x.clone(), y.clone()))
            .map(move |(addr, mut peer): (SocketAddr, Peer)| {
                peer.call(GetStatus).map_ok(move |res| (addr, res))
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

#[derive(Debug)]
pub enum DirectedError<E> {
    Internal(E),
    Missing,
}

pub struct DirectedQuery<T>(pub SocketAddr, pub T);

impl<T> Service<DirectedQuery<T>> for Arena
where
    Peer: Service<T>,
    <Peer as Service<T>>::Future: Send,
    T: Send + 'static,
{
    type Response = <Peer as Service<T>>::Response;
    type Error = DirectedError<<Peer as Service<T>>::Error>;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <Peer as Service<T>>::poll_ready(&mut peer, cx) {
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
            let mut_client: Option<Peer> = peers.get(&addr).map(|some| some.value().clone());
            match mut_client {
                Some(mut some) => some.call(request).map_err(DirectedError::Internal).await,
                None => Err(DirectedError::Missing),
            }
        };

        Box::pin(fut)
    }
}

pub struct AllQuery<T>(pub T);

impl<T> Service<AllQuery<T>> for Arena
where
    Peer: Service<T>,
    <Peer as Service<T>>::Future: Send,
    <Peer as Service<T>>::Error: Send,
    <Peer as Service<T>>::Response: Send,
    T: 'static + Clone,
{
    type Response = std::collections::HashMap<SocketAddr, <Peer as Service<T>>::Response>;
    type Error = DirectedError<<Peer as Service<T>>::Error>;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
            match <Peer as Service<T>>::poll_ready(&mut peer, cx) {
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
            .map(move |(addr, mut peer): (SocketAddr, Peer)| {
                peer.call(request.clone()).map_ok(move |res| (addr, res))
            });

        let fut = futures::future::join_all(collected).map(move |collection: Vec<Result<_, _>>| {
            let filtered_collection: std::collections::HashMap<
                SocketAddr,
                <Peer as Service<T>>::Response,
            > = collection
                .into_iter()
                .filter_map(move |res: Result<_, _>| res.ok())
                .collect();

            Ok(filtered_collection)
        });
        Box::pin(fut)
    }
}
