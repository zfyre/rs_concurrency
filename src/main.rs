//! Entry point for concurrency demos

mod dfs_concurr;
mod read_write_lock;
mod total_ordering_for_deadlocks;
mod optimistic_locking;

#[tokio::main]
async fn main() {
    // dfs_concurr::run().await;
    // read_write_lock::run().await;
    // total_ordering_for_deadlocks::run().await;
    optimistic_locking::run().await;
}