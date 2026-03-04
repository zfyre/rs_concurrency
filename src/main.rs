//! Entry point for concurrency demos

mod dfs_concurr;
mod read_write_lock;

#[tokio::main]
async fn main() {
    // dfs_concurr::run().await;
    read_write_lock::run().await;
}