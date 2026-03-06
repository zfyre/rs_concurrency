# Rust Concurrency Learning

A hands-on project exploring concurrency patterns in Rust using Tokio.

## Project Structure

```
src/
├── dfs_concurr.rs       # Concurrent DFS algorithms
├── main.rs              # Entry point for concurrency demos
├── optimistic_locking.rs # Optimistic Locking OR Atomics without Ordering Understanding!!! 
├── read_write_lock.rs   # Custom Read-Write Lock using Tokio semaphores
└── total_ordering_for_deadlocks.rs # Deadlock prevention using total ordering

docs/
├── ReadWriteLockImpl.md
├── rust_concurrency_guide.md
└── stuff.md
```
## Documentation

- [Read-Write Lock Implementation Notes](docs/ReadWriteLockImpl.md)
- [Rust Concurrency: A Complete Guide for Students](docs/rust_concurrency_guide.md)
- [Boxing Futures in Recursive Async Functions](docs/stuff.md)

## Running

```bash
cargo run
```

## Key Learnings

1. **Read-Write Lock** - Implemented using Tokio semaphores with:
   - Lightswitch pattern for reader coordination
   - Turnstile pattern for writer starvation prevention
   - `forget()` + `add_permits()` for manual permit management

2. **Concurrent DFS** - Multiple approaches:
   - Sequential traversal
   - Thread-based concurrency
   - Channel-based communication

## Dependencies

- `tokio` - Async runtime with semaphore support
- `rand` - Random number generation for demos
