use std::convert::TryInto;

use bytes::Bytes;
use crypto::{blake3, Minisketch as MinisketchCrypto, MinisketchError};

/*
Network messages
*/

#[derive(Clone, Debug, PartialEq)]
pub struct Minisketch(pub Bytes);

impl Minisketch {
    pub fn hydrate(self, radius: usize) -> Result<MinisketchCrypto, MinisketchError> {
        let mut ms = MinisketchCrypto::try_new(64, 0, radius).unwrap(); // TODO: Make safe
        ms.deserialize(&self.0[..radius * 8]);
        Ok(ms)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub oddsketch: Bytes,
    pub root: Bytes,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub timestamp: u64,
    pub binary: Bytes,
    pub aux_data: Bytes,
}

impl Transaction {
    pub fn serialize(self) -> Vec<u8> {
        let mut raw = self.timestamp.to_be_bytes().to_vec();
        raw.extend(self.binary);
        raw.to_vec()
    }

    pub fn get_id(&self) -> [u8; blake3::OUT_LEN] {
        blake3::hash(&self.clone().serialize()).into()
    }

    pub fn get_short_id(&self) -> u64 {
        let arr: [u8; 8] = self.get_id()[..8].try_into().unwrap();
        u64::from_be_bytes(arr)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transactions {
    pub txs: Vec<Transaction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionInv {
    pub tx_ids: Vec<Bytes>,
}
