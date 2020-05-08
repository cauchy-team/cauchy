use bytes::Bytes;

/*
Network messages
*/

#[derive(Clone, Debug, PartialEq)]
pub struct Minisketch(pub Bytes);

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
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transactions {
    pub txs: Vec<Transaction>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionInv {
    pub tx_ids: Vec<Bytes>,
}
