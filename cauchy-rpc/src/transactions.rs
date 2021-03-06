pub mod gen {
    tonic::include_proto!("transactions");
}

use bytes::Bytes;
use tonic::{Request, Response};
use tower_service::Service;
use tower_util::ServiceExt;

use common::network::Transaction as TransactionMsg;

use gen::transactions_server::Transactions;
use gen::*;

#[derive(Clone)]
pub struct TransactionsService<Pl> {
    player: Pl,
}

impl<Pl> TransactionsService<Pl> {
    pub fn new(player: Pl) -> Self {
        TransactionsService { player }
    }
}

#[tonic::async_trait]
impl<Pl> Transactions for TransactionsService<Pl>
where
    Pl: Clone + Send + Sync + 'static,
    // Broadcast transaction
    Pl: Service<TransactionMsg>,
    <Pl as Service<TransactionMsg>>::Future: Send,
{
    async fn broadcast_transaction(
        &self,
        transaction: Request<Transaction>,
    ) -> Result<Response<()>, tonic::Status> {
        let transaction = transaction.into_inner();
        let tx_msg = TransactionMsg {
            timestamp: transaction.timestamp,
            binary: Bytes::from(transaction.binary),
            aux_data: Bytes::from(transaction.aux_data),
        };

        self.player
            .clone()
            .oneshot(tx_msg)
            .await
            .map_err(|_err| tonic::Status::unknown("todo"))?;
        Ok(Response::new(()))
    }
}
