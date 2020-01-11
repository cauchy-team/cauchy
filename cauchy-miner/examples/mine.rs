use cauchy_miner::*;
use futures_util::stream::StreamExt;

#[tokio::main]
async fn main() {
    let miner = Mining::new(Default::default(), 0);

    let mut record_channel = miner.mine(Default::default());
    while let Some(record) = record_channel.next().await {
        println!("{:#?}", record);
    }
}
