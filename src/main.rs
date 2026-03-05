//! Entry point for concurrency demos

mod dfs_concurr;
mod read_write_lock;
mod total_ordering_for_deadlocks;

#[tokio::main]
async fn main() {
    // dfs_concurr::run().await;
    // read_write_lock::run().await;
    total_ordering_for_deadlocks::run().await;
}