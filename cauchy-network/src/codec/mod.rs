mod decoder;

use bytes::Bytes;

use decoder::DecodeState;

const DIGEST_LEN: usize = 32;

/*
Network messages
*/

pub struct Status {
    oddsketch: Bytes,
    root: Bytes,
    nonce: u64,
}

pub struct Transaction {
    timestamp: u64,
    binary: Bytes,
}

pub struct TransactionInv {
    tx_ids: Vec<Bytes>,
}

pub struct Transactions {
    txs: Vec<Transaction>,
}

pub enum Message {
    Poll,
    Status(Status),
    Reconcile,
    ReconcileResponse(Transactions),
    Transaction(Transaction),
    TransactionInv(TransactionInv),
    Transactions(Transactions),
}

/*
Message codec
*/

pub struct MessageCodec {
    state: DecodeState,
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self {
            state: DecodeState::Type,
        }
    }
}
