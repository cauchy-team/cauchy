mod decoder;
mod encoder;

use bytes::Bytes;

pub use decoder::*;
pub use encoder::*;

const DIGEST_LEN: usize = 32;

pub const MAGIC_BYTES: [u8; 4] = [1, 2, 3, 4];

/*
Network messages
*/

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub oddsketch: Bytes,
    pub root: Bytes,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    timestamp: u64,
    binary: Bytes,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transactions {
    txs: Vec<Transaction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionInv {
    tx_ids: Vec<Bytes>,
}

#[derive(Clone, Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use tokio_util::codec::{Decoder as _, Encoder as _};
    use bytes::BytesMut;
    use rand::prelude::*;

    use super::*;

    fn generate_random_status() -> Status {
        let mut rng = rand::thread_rng();
        let root: Vec<u8> = (0..DIGEST_LEN).map(|_| rng.gen()).collect();
        let oddsketch_len = 128; // TODO: Randomize
        let oddsketch: Vec<u8> = (0..oddsketch_len).map(|_| rng.gen()).collect();
        Status {
            oddsketch: Bytes::from(oddsketch),
            root: Bytes::from(root),
            nonce: 324,
        }
    }

    fn generate_random_tx() -> Transaction {
        let mut rng = rand::thread_rng();
        let binary_len = 1024;
        let binary: Vec<u8> = (0..binary_len).map(|_| rng.gen()).collect();
        let timestamp = rng.gen();
        Transaction {
            timestamp,
            binary: Bytes::from(binary),
        }
    }

    fn generate_random_digest() -> Vec<u8> {
        let mut rng = rand::thread_rng();
        (0..DIGEST_LEN).map(|_| rng.gen()).collect()
    }

    #[test]
    fn poll_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        codec
            .encode(Message::Poll, &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::Poll)
    }

    #[test]
    fn status_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        let status = generate_random_status();

        codec
            .encode(Message::Status(status.clone()), &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::Status(status));

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }

    #[test]
    fn reconcile_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        codec
            .encode(Message::Reconcile, &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::Reconcile);

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }

    #[test]
    fn reconcile_response_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        let n_txs = 128; // TODO: Randomize
        let txs: Vec<_> = (0..n_txs).map(|_| generate_random_tx()).collect();
        let transactions = Transactions { txs };

        codec
            .encode(Message::ReconcileResponse(transactions.clone()), &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::ReconcileResponse(transactions));

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }

    #[test]
    fn transaction_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        let tx = generate_random_tx();

        codec
            .encode(Message::Transaction(tx.clone()), &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::Transaction(tx));

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }

    #[test]
    fn transactions_inv_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        let n_tx_ids = 128; // TODO: Randomize
        let tx_ids: Vec<_> = (0..n_tx_ids)
            .map(|_| Bytes::from(generate_random_digest()))
            .collect();
        let inv = TransactionInv { tx_ids };

        codec
            .encode(Message::TransactionInv(inv.clone()), &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::TransactionInv(inv));

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }

    #[test]
    fn transactions_complete() {
        let mut buf = BytesMut::default();
        let mut codec = MessageCodec::default();

        let n_txs = 128; // TODO: Randomize
        let txs: Vec<_> = (0..n_txs).map(|_| generate_random_tx()).collect();
        let transactions = Transactions { txs };

        codec
            .encode(Message::Transactions(transactions.clone()), &mut buf)
            .expect("encoding error");

        let result = codec
            .decode(&mut buf)
            .expect("decoding error")
            .expect("decoding incomplete");

        assert_eq!(result, Message::Transactions(transactions));

        let result = codec.decode(&mut buf).expect("decoding error");
        assert_eq!(result, None);
    }
}
