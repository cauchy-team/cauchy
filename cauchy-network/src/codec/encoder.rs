use std::io;

use bytes::buf::BufMut;
use bytes::BytesMut;
use tokio_util::codec::Encoder;
use tracing::trace;

use super::*;

#[derive(Debug)]
pub enum EncodingError {
    IO(io::Error),
}

impl From<io::Error> for EncodingError {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

fn put_transaction(tx: Transaction, dst: &mut BytesMut) {
    let binary_len = tx.binary.len();
    let aux_len = tx.aux_data.len();
    dst.reserve(8 + 4 + binary_len + 4 + aux_len);

    dst.put_u64(tx.timestamp);
    dst.put_u32(binary_len as u32);
    dst.put(tx.binary);
    dst.put_u32(aux_len as u32);
    dst.put(tx.aux_data);
}

impl Encoder<Message> for MessageCodec {
    type Error = EncodingError;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        trace!("encoding {:?}", item);
        match item {
            Message::Poll => {
                dst.reserve(1);
                dst.put_u8(0)
            }
            Message::Status(status) => {
                let oddsketch_len = status.oddsketch.len();
                dst.reserve(1 + 4 + oddsketch_len + DIGEST_LEN + 8);

                dst.put_u8(1);
                dst.put_u16(oddsketch_len as u16); // This is safe
                dst.put(status.oddsketch);
                dst.put(status.root);
                dst.put_u64(status.nonce);
            }
            Message::Reconcile(minisketch) => {
                let minisketch_raw = minisketch.0;
                let minisketch_len = minisketch_raw.len() as u32;
                dst.reserve(1);

                dst.put_u8(2);
                dst.put_u32(minisketch_len / 32); // This is safe
                dst.put(minisketch_raw);
            }
            Message::ReconcileResponse(txs) => {
                dst.reserve(1 + 4);

                dst.put_u8(3);
                let n_txs = txs.txs.len();
                dst.put_u32(n_txs as u32);
                for tx in txs.txs {
                    put_transaction(tx, dst);
                }
            }
            Message::Transaction(tx) => {
                let binary_len = tx.binary.len();
                let aux_len = tx.aux_data.len();
                dst.reserve(1 + 8 + 4 + binary_len + 4 + aux_len);

                dst.put_u8(4);
                dst.put_u64(tx.timestamp);
                dst.put_u32(binary_len as u32);
                dst.put(tx.binary);
                dst.put_u32(aux_len as u32);
                dst.put(tx.aux_data);
            }
            Message::TransactionInv(tx_inv) => {
                let n_tx_ids = tx_inv.tx_ids.len();
                let tx_id_size = n_tx_ids * DIGEST_LEN;
                dst.reserve(1 + 4 + tx_id_size);

                dst.put_u8(5);
                dst.put_u32(n_tx_ids as u32);
                for tx_id in tx_inv.tx_ids {
                    dst.put(tx_id);
                }
            }
            Message::Transactions(txs) => {
                dst.reserve(1 + 4);

                dst.put_u8(6);
                let n_txs = txs.txs.len();
                dst.put_u32(n_txs as u32);
                for tx in txs.txs {
                    put_transaction(tx, dst);
                }
            }
        }
        trace!("encoding successful; {:?}", dst);
        Ok(())
    }
}
