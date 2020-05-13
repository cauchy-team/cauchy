/*
Future optimizations:
For code cleanliness, we greedily set our inner state options as soon as we parse them. This
is a problem because one can immediately set the state to DecodeState::Type when
the src buffer is large enough to complete entire parsing in one poll.

It's therefore possible that this is a few allocations short of perfect on some paths.
However, it is very possible that the compiler is compiling the problem away.
*/

use std::io;

use bytes::buf::Buf;
use bytes::BytesMut;
use tokio_util::codec::Decoder;
use tracing::trace;

use super::*;
use common::network::*;

/*
Decoding states
*/

#[derive(Default)]
pub struct StatusState {
    oddsketch_len: Option<u16>,
}

impl StatusState {
    fn decode_inner(oddsketch_len: u16, src: &mut BytesMut) -> Option<Status> {
        if src.remaining() < (oddsketch_len + DIGEST_LEN as u16 + 8) as usize {
            None
        } else {
            let oddsketch = src.split_to(oddsketch_len as usize).freeze();
            let root = src.split_to(DIGEST_LEN).freeze();
            let nonce = src.get_u64();
            let status = Status {
                oddsketch,
                root,
                nonce,
            };
            Some(status)
        }
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Status>, DecodeError> {
        if let Some(oddsketch_len) = self.oddsketch_len {
            Ok(Self::decode_inner(oddsketch_len, src))
        } else {
            if src.remaining() < 2 {
                Ok(None)
            } else {
                let oddsketch_len = src.get_u16();
                self.oddsketch_len = Some(oddsketch_len);

                Ok(Self::decode_inner(oddsketch_len, src))
            }
        }
    }
}

#[derive(Default)]
pub struct TransactionState {
    timestamp: Option<u64>,
    binary_len: Option<u32>,
}

impl TransactionState {
    fn decode_with_timestamp(
        &mut self,
        timestamp: u64,
        src: &mut BytesMut,
    ) -> Result<Option<Transaction>, DecodeError> {
        if src.remaining() < 4 {
            Ok(None)
        } else {
            let binary_len = src.get_u32();
            self.binary_len = Some(binary_len);

            if src.remaining() < binary_len as usize {
                Ok(None)
            } else {
                Ok(Self::decode_with_all(timestamp, binary_len, src))
            }
        }
    }

    fn decode_with_all(timestamp: u64, binary_len: u32, src: &mut BytesMut) -> Option<Transaction> {
        let binary_len_usize = binary_len as usize;
        if src.remaining() < binary_len_usize {
            None
        } else {
            let binary = src.split_to(binary_len_usize).freeze();
            let tx = Transaction { timestamp, binary };
            Some(tx)
        }
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Transaction>, DecodeError> {
        match (self.timestamp, self.binary_len) {
            (None, _) => {
                if src.remaining() < 8 {
                    Ok(None)
                } else {
                    let timestamp = src.get_u64();
                    self.timestamp = Some(timestamp);
                    self.decode_with_timestamp(timestamp, src)
                }
            }
            (Some(timestamp), None) => self.decode_with_timestamp(timestamp, src),
            (Some(timestamp), Some(binary_len)) => {
                Ok(Self::decode_with_all(timestamp, binary_len, src))
            }
        }
    }
}

#[derive(Default)]
pub struct TransactionsState {
    n_txs: Option<u32>,
    read: Option<Vec<Transaction>>,
    tx_state: TransactionState,
}

impl TransactionsState {
    fn decode_inner(
        &mut self,
        n_txs: u32,
        src: &mut BytesMut,
    ) -> Result<Option<Transactions>, DecodeError> {
        let n_read = self.read.as_ref().unwrap().len(); // This is safe
        let n_remaining_txs = n_txs as usize - n_read;
        for _ in 0..n_remaining_txs {
            match self.tx_state.decode(src)? {
                Some(transaction) => {
                    self.tx_state = TransactionState::default();
                    self.read.as_mut().unwrap().push(transaction)
                }
                None => return Ok(None),
            };
        }
        let txs = self.read.take().unwrap(); // This is safe
        let transactions = Transactions { txs };
        Ok(Some(transactions))
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Transactions>, DecodeError> {
        if let Some(n_txs) = self.n_txs {
            self.decode_inner(n_txs, src)
        } else {
            if src.len() < 4 {
                return Ok(None);
            }

            let n_txs = src.get_u32();
            self.n_txs = Some(n_txs);
            self.read = Some(vec![]);
            self.decode_inner(n_txs, src)
        }
    }
}

#[derive(Default)]
pub struct ReconcileState {
    minisketch_len: Option<u32>,
}

impl ReconcileState {
    fn decode_inner(minisketch_len: u32, src: &mut BytesMut) -> Option<Minisketch> {
        let total_len = minisketch_len as usize * 32;
        if src.remaining() < total_len {
            None
        } else {
            let minisketch = src.split_to(total_len);
            Some(Minisketch(minisketch.freeze()))
        }
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Minisketch>, DecodeError> {
        if let Some(minisketch_len) = self.minisketch_len {
            Ok(Self::decode_inner(minisketch_len, src))
        } else {
            if src.remaining() < 4 {
                Ok(None)
            } else {
                let minisketch_len = src.get_u32();
                self.minisketch_len = Some(minisketch_len);
                Ok(Self::decode_inner(minisketch_len, src))
            }
        }
    }
}

#[derive(Default)]
pub struct TransactionInvState {
    n_tx_ids: Option<u32>,
}

impl TransactionInvState {
    fn decode_inner(n_tx_ids: u32, src: &mut BytesMut) -> Option<TransactionInv> {
        if src.remaining() < n_tx_ids as usize * DIGEST_LEN {
            None
        } else {
            let tx_ids = (0..n_tx_ids)
                .map(|_| src.split_to(DIGEST_LEN).freeze())
                .collect();
            let tx_inv = TransactionInv { tx_ids };
            Some(tx_inv)
        }
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<TransactionInv>, DecodeError> {
        if let Some(n_tx_ids) = self.n_tx_ids {
            Ok(Self::decode_inner(n_tx_ids, src))
        } else {
            if src.remaining() < 4 {
                Ok(None)
            } else {
                let n_tx_ids = src.get_u32();
                self.n_tx_ids = Some(n_tx_ids);
                Ok(Self::decode_inner(n_tx_ids, src))
            }
        }
    }
}

pub enum DecodeState {
    Type,
    Poll,
    Status(StatusState),
    Reconcile(ReconcileState),
    ReconcileResponse(TransactionsState),
    TransactionInv(TransactionInvState),
    Transaction(TransactionState),
    Transactions(TransactionsState),
}

/*
Decoding error
*/

#[derive(Debug)]
pub enum DecodeError {
    UnexpectedType,
    IO(io::Error),
}

impl From<io::Error> for DecodeError {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

/*
Implement codec
*/

impl MessageCodec {
    fn decode_type(&mut self, src: &mut BytesMut) -> Result<Option<()>, DecodeError> {
        if src.is_empty() {
            return Ok(None);
        }

        self.state = match src.get_u8() {
            0 => DecodeState::Poll,
            1 => DecodeState::Status(StatusState::default()),
            2 => DecodeState::Reconcile(ReconcileState::default()),
            3 => DecodeState::ReconcileResponse(TransactionsState::default()),
            4 => DecodeState::Transaction(TransactionState::default()),
            5 => DecodeState::TransactionInv(TransactionInvState::default()),
            6 => DecodeState::Transactions(TransactionsState::default()),
            _ => return Err(DecodeError::UnexpectedType),
        };

        Ok(Some(()))
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, DecodeError> {
        if let DecodeState::Type = self.state {
            trace!("decoding type; {:?}", src);
            if self.decode_type(src)?.is_none() {
                return Ok(None);
            }
        }

        match &mut self.state {
            DecodeState::Poll => {
                trace!("decoding poll; {:?}", src);
                self.state = DecodeState::Type;
                Ok(Some(Message::Poll))
            }
            DecodeState::Status(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding status; {:?}", src);
                opt.map(|status| {
                    self.state = DecodeState::Type;
                    Message::Status(status)
                })
            }),
            DecodeState::Reconcile(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding reconcile; {:?}", src);
                opt.map(|minisketch| {
                    self.state = DecodeState::Type;
                    Message::Reconcile(minisketch)
                })
            }),
            DecodeState::ReconcileResponse(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding reconcile response; {:?}", src);
                opt.map(|txs| {
                    self.state = DecodeState::Type;
                    Message::ReconcileResponse(txs)
                })
            }),
            DecodeState::Transaction(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding reconcile response; {:?}", src);
                opt.map(|txs| {
                    self.state = DecodeState::Type;
                    Message::Transaction(txs)
                })
            }),
            DecodeState::TransactionInv(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding transaction inv; {:?}", src);
                opt.map(|inv| {
                    self.state = DecodeState::Type;
                    Message::TransactionInv(inv)
                })
            }),
            DecodeState::Transactions(inner_state) => inner_state.decode(src).map(|opt| {
                trace!("decoding transactions; {:?}", src);
                opt.map(|txs| {
                    self.state = DecodeState::Type;
                    Message::Transactions(txs)
                })
            }),
            _ => unreachable!(),
        }
    }
}
