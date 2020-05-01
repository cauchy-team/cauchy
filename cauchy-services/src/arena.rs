use std::{collections::HashMap, net::SocketAddr, pin::Pin, sync::Arc};

use dashmap::DashMap;
use futures::{
    prelude::*,
    task::{Context, Poll},
};
use network::codec::Status;
use rand::{rngs::OsRng, seq::IteratorRandom};
use tower::Service;

use crate::peers::*;

#[derive(Clone, Default)]
pub struct Arena {
    peers: Arc<DashMap<SocketAddr, client::PeerClient>>,
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
        for mut peer in self.peers.iter_mut() {
            match <client::PeerClient as Service<GetMetadata>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: GetAllMetadata) -> Self::Future {
        let calls = self.peers.iter_mut().map(|mut peer| peer.call(GetMetadata));
        Box::pin(futures::future::try_join_all(calls))
    }
}

pub struct PollSample(usize);

impl Service<PollSample> for Arena {
    type Response = Vec<(SocketAddr, Status)>;
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        for mut peer in self.peers.iter_mut() {
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
        let sample = self.peers.iter().choose_multiple(&mut OsRng, num);

        let collected = sample
            .iter()
            .map(move |reference| reference.pair())
            .map(|(x, y)| (x.clone(), y.clone()))
            .map(move |(addr, mut peer): (SocketAddr, client::PeerClient)| {
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
        for mut peer in self.peers.iter_mut() {
            match <client::PeerClient as Service<T>>::poll_ready(&mut peer, cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(DirectedError::Internal(err))),
                _ => continue,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, DirectedQuery(addr, request): DirectedQuery<T>) -> Self::Future {
        let mut_client: Option<client::PeerClient> =
            self.peers.get(&addr).map(|some| some.value().clone());
        match mut_client {
            Some(mut some) => {
                Box::pin(async move { some.call(request).map_err(DirectedError::Internal).await })
            }
            None => Box::pin(async move { Err(DirectedError::Missing) }),
        }
    }
}
