# Boxing Futures in Recursive Async Functions

## What is `.boxed()`?

`.boxed()` allocates the future on the **heap** and returns a `Box<dyn Future>` (type-erased, fixed-size pointer).

```rust
// Without boxing - concrete type, size known at compile time
async fn foo() { }  // returns impl Future<Output=()>

// With boxing - heap-allocated, fixed pointer size
fn foo() -> BoxFuture<'static, ()> {
    async { }.boxed()
}
```

## Why Recursive Futures Need Boxing

### The Problem: Infinite Type Size

When you write a recursive async function:

```rust
async fn traverse(node: i32) {
    traverse(child).await;  // ❌ recursive call
}
```

Rust needs to compute the size of the returned `Future` at compile time. But:

1. `traverse` returns a future of some size `S`
2. That future contains another call to `traverse`, which is also size `S`
3. Which contains another... and so on

This creates an **infinitely-sized type**:

```
Size = S + S + S + S + ...  → ∞
```

The compiler can't determine the size, so it fails.

### The Solution: Boxing

Boxing breaks the infinite recursion by using **indirection**:

```rust
fn traverse(node: i32) -> BoxFuture<'static, ()> {
    async move {
        traverse(child).await;  // ✅ returns BoxFuture (fixed 16 bytes)
    }.boxed()
}
```

Now:
- `BoxFuture` is always a fixed size (pointer = 16 bytes on 64-bit)
- The actual future lives on the heap
- No infinite type calculation needed

## What Actually Gets Heap-Allocated

Not the function itself—the **Future's state** lives on the heap.

When you call an async function, Rust creates a **state machine** (a struct) that holds:
- All local variables that live across `.await` points
- The current state/position in the function

```rust
fn traverse(node: i32) -> BoxFuture<'static, ()> {
    async move {
        let x = 10;           // captured in state machine
        some_async().await;   // <-- suspend point
        println!("{}", x);    // x must survive across await
    }.boxed()
}
```

### Without boxing (stack):
```
Stack:
┌─────────────────────────┐
│ Future State Machine    │
│ - node: i32             │
│ - x: i32                │
│ - state: enum           │
│ - nested Future (???)   │  ← can't determine size
└─────────────────────────┘
```

### With boxing (heap):
```
Stack:                      Heap:
┌─────────────────────┐    ┌─────────────────────────┐
│ Box<dyn Future>     │───→│ Future State Machine    │
│ (16 bytes)          │    │ - node: i32             │
└─────────────────────┘    │ - x: i32                │
                           │ - state: enum           │
                           │ - Box<dyn Future> ──────────→ (next level)
                           └─────────────────────────┘
```

## Summary

| What | Where |
|------|-------|
| Function code | Binary (read-only memory) - same either way |
| `Box` pointer | Stack (fixed 16 bytes) |
| Future state machine | **Heap** (variable size, allocated at runtime) |

The function's **code** never moves—it's always in the binary. Boxing just moves the **data/state** of the async computation to the heap.

## Trade-offs

Boxing adds:
- Heap allocation (small overhead)
- Dynamic dispatch via `dyn Future`

But it's the only way to make recursive async work in Rust.

---

# Controlling Variable Capture in `async move` / `move ||`

## The `move` keyword moves ALL captured variables by default

```rust
let a = String::from("hello");
let b = 42;
let c = vec![1, 2, 3];

tokio::spawn(async move {
    // a, b, c are ALL moved here
    println!("{} {} {:?}", a, b, c);
});

// a, b, c are no longer usable here!
```

## To control what moves vs. what doesn't:

### 1. Clone before the block (most common)
```rust
let graph = Arc::new(graph);
let graph_clone = Arc::clone(&graph);  // Clone outside

tokio::spawn(async move {
    // graph_clone is moved (but it's just an Arc pointer)
    use_graph(&graph_clone).await;
});

// graph is still usable here!
```

### 2. Create references before `move`
```rust
let data = vec![1, 2, 3];
let data_ref = &data;  // Borrow before move

// This WON'T work with spawn (needs 'static)
// But works with scoped tasks or non-'static contexts
```

### 3. Use blocks to limit scope
```rust
let owned_string = String::from("hello");
let number = 42;

// Only move what you need by creating local bindings
let to_move = owned_string.clone();  // Clone what you want to move

tokio::spawn(async move {
    println!("{}", to_move);  // Only to_move is moved
});

println!("{}", owned_string);  // Still works!
```

### 4. Explicit pattern for spawning tasks in a loop
```rust
for node in msg {
    // Explicitly prepare what to move
    let graph_clone = Arc::clone(&graph);  // ← Will be moved
    let tx_this = tx.clone();              // ← Will be moved
    // node is Copy, so it's copied automatically
    
    tokio::spawn(async move {
        // Inside here, we own: graph_clone, tx_this, node
        let children = get_children_async(node, &graph_clone).await;
        tx_this.send(children).await.unwrap();
    });
}
```

## Summary Table

| Technique | When to Use |
|-----------|-------------|
| `Arc::clone(&x)` before move | Shared ownership across tasks |
| `.clone()` before move | Need copy in task, keep original |
| `let x = x;` or shadow | Force ownership of specific binding |
| No `move` keyword | Borrow (only works if lifetime allows) |

## Key insight

There's no syntax like `move |x, y|` to list specific variables. You control it by:
1. **Cloning** what you want to share
2. **Shadowing** with `let x = x.clone()` 
3. **Preparing** variables outside the closure that you want moved

---

# Channel-Based Concurrency with `try_recv()` and `yield_now()`

## The Problem with `recv().await`

When using channels for work distribution, `recv().await` blocks until a message arrives:

```rust
while let Some(node) = rx.recv().await {  // ← Blocks forever if no messages
    // process...
}
```

**Problem**: If all work is done but `tx` still exists somewhere, `recv()` never returns `None` and the loop hangs.

## Solution: `try_recv()` + `yield_now()`

```rust
loop {
    match rx.try_recv() {
        Ok(new_node) => {
            // Process the node, spawn task...
        }
        Err(TryRecvError::Empty) => {
            if active.load(Ordering::SeqCst) == 0 {
                break;  // All work done, exit
            }
            tokio::task::yield_now().await;  // Let other tasks run
        }
        Err(TryRecvError::Disconnected) => break,  // Channel closed
    }
}
```

### How it works:

| Function | Behavior |
|----------|----------|
| `try_recv()` | Returns immediately (non-blocking) |
| `yield_now().await` | Gives other tasks a chance to run |
| `active` counter | Tracks how many tasks are still working |

### Flow:
1. Try to receive a message instantly
2. If empty + no active tasks → we're done, break
3. If empty + tasks still active → yield control, let tasks run and send more work
4. Repeat

This avoids timeouts and achieves clean termination.

---

# Concurrent DFS vs Channel DFS: Resource Efficiency

## The Key Difference

### Concurrent DFS (recursive spawn with await)
```rust
fn concurr_traverse(node, ...) -> BoxFuture<()> {
    async move {
        // ... process node ...
        for child in children {
            let handle = tokio::spawn(concurr_traverse(child, ...));
            handles.push(handle);
        }
        // PARENT WAITS for all children ↓
        for handle in handles {
            handle.await;  // ← Task stays alive, holding resources
        }
    }.boxed()
}
```

**Problem**: Parent task stays alive until ALL children complete:

```
Task Tree (Concurrent DFS):
                   
    Task(0) ─────────────────────────────────────────► [WAITING]
       ├─► Task(1) ───────────────────────► [WAITING]
       │      ├─► Task(3) ──────► [DONE]
       │      └─► Task(4) ──────► [DONE]
       │              ↑ Parent waits here
       └─► Task(2) ───────────────────────► [WAITING]
              └─► Task(5) ──────► [DONE]
                      ↑ Parent waits here

Active tasks at peak: ALL nodes (deep tree = many waiting tasks)
```

### Channel DFS (fire-and-forget)
```rust
async fn channel_traverse(node, ...) {
    loop {
        match rx.try_recv() {
            Ok(new_node) => {
                tokio::spawn(async move {
                    let children = get_children(new_node).await;
                    for child in children {
                        tx.send(child).await;  // Just send, don't wait
                    }
                    // Task EXITS immediately after sending
                });
            }
            // ...
        }
    }
}
```

**Advantage**: Task completes as soon as it fetches children and sends them:

```
Task Timeline (Channel DFS):

Time ──────────────────────────────────────────────►

Task(0): [fetch] [send children] [DONE] ✓
Task(1): ........[fetch] [send] [DONE] ✓
Task(2): ........[fetch] [send] [DONE] ✓
Task(3): ................[fetch] [send] [DONE] ✓
Task(4): ................[fetch] [send] [DONE] ✓
Task(5): ....................[fetch] [send] [DONE] ✓

Active tasks at any time: Only a few (limited by channel buffer + concurrency)
```

## Why Channel DFS is Better for Resource-Constrained Environments

| Aspect | Concurrent DFS | Channel DFS |
|--------|---------------|-------------|
| Task lifetime | Long (waits for children) | Short (fire-and-forget) |
| Memory per task | High (holds future state while waiting) | Low (completes quickly) |
| Max concurrent tasks | O(tree depth) × branching factor | Controlled by channel buffer |
| With semaphore(100) | 100 tasks, but many are **suspended/waiting** | 100 tasks, all **actively working** |
| Scheduler overhead | High (thousands of suspended tasks) | Low (few active tasks) |

### Real-world analogy: URL Fetching

```
Concurrent DFS:
  Parent fetches URL, spawns children for links, WAITS for all links to be fetched
  → Like a manager who assigns work but stands there watching until everyone is done
  
Channel DFS:
  Worker fetches URL, puts discovered links into a queue, EXITS
  → Like a worker who does their job and moves on, letting others pick up new work
```

### When to use what:

| Use Case | Best Approach |
|----------|---------------|
| Low thread limit / memory constrained | **Channel DFS** |
| Need to know when subtree completes | Concurrent DFS |
| High throughput, many items | **Channel DFS** |
| Complex dependencies between tasks | Concurrent DFS |
| Web crawling / URL fetching | **Channel DFS** |
| Tree reduction (aggregate results) | Concurrent DFS |

## Memory Comparison (10,000 nodes)

| Method | Time | Peak Heap Memory |
|--------|------|------------------|
| Sequential DFS | 24718 ms | 437 KB |
| Concurrent DFS | 366 ms | 1.90 MB |
| Channel DFS | 69 ms | 1.28 MB |

Channel DFS uses **~32% less memory** than Concurrent DFS while being faster.

## Performance Under Thread Constraints (10,000 nodes, 5 worker threads)

When limiting the Tokio runtime to only **5 worker threads**:

| Method | Time | Notes |
|--------|------|-------|
| Sequential DFS | 24898 ms | No parallelism (baseline) |
| Concurrent DFS | 6874 ms | **Severely degraded** - threads blocked waiting for children |
| Channel DFS | 80 ms | **~86x faster** than Concurrent DFS |

### Why such a dramatic difference?

**Important clarification**: `.await` does NOT block threads — it **yields**!

When a task hits `.await`, it:
1. Suspends the current task
2. Returns the thread to the runtime
3. The thread picks up other ready tasks

```
What ACTUALLY happens with async/await:

Time →

Thread 1: Task(0) yield → Task(5) run → Task(10) run → Task(0) resume...
Thread 2: Task(1) yield → Task(6) run → Task(11) run → Task(1) resume...
...

"yield" = task suspends, thread moves to next task (NOT blocked!)
```

### So why is Concurrent DFS still so much slower?

The problem isn't thread blocking — it's **task overhead**:

#### 1. Memory: All parent tasks stay ALIVE in memory

```
Concurrent DFS task tree at some point:

Task(0) [SUSPENDED - waiting for 1,2,3] ← ~500 bytes in memory
  ├─ Task(1) [SUSPENDED - waiting for 4,5]  ← ~500 bytes
  │    ├─ Task(4) [RUNNING]
  │    └─ Task(5) [RUNNING]
  ├─ Task(2) [SUSPENDED - waiting for 6,7]  ← ~500 bytes
  │    └─ ...
  └─ Task(3) [SUSPENDED]  ← ~500 bytes

Total tasks alive: O(all nodes) = 10,000 suspended futures!
```

#### 2. Scheduler overhead

With 10,000 suspended tasks, the Tokio runtime must:
- Track wake-up conditions for each task
- Poll tasks when children complete
- Manage a massive task queue
- Handle thousands of `JoinHandle` futures

#### 3. Channel DFS: Tasks complete and DISAPPEAR

```
Channel DFS at same point:

Main loop [RUNNING]
Task(4) [RUNNING] → will finish and DROP (memory freed)
Task(5) [RUNNING] → will finish and DROP (memory freed)
              ↓
Total tasks alive: ~10-20 at any time (controlled by concurrency)
```

### Visual comparison: Tasks alive over time

```
CONCURRENT DFS (10,000 nodes):
─────────────────────────────────────────
Tasks alive over time:

     │    ████████████████████████████████
10000│   █                                █
     │  █                                  █
 5000│ █                                    █
     │█                                      █
    0│────────────────────────────────────────
     Start                              End

→ Thousands of suspended tasks consuming memory
→ Scheduler polling/tracking all of them
→ Memory pressure, cache misses, GC-like overhead


CHANNEL DFS (10,000 nodes):
─────────────────────────────────────────
Tasks alive over time:

     │
10000│
     │
 5000│
     │  ─────────────────────────────────
   20│ ████████████████████████████████████
    0│────────────────────────────────────────
     Start                              End

→ Only ~20 tasks alive at any time
→ Minimal scheduler overhead
→ Excellent cache locality
→ Fast!
```

### Summary: The real difference

| Aspect | Concurrent DFS | Channel DFS |
|--------|----------------|-------------|
| Parent behavior | Waits for children (stays alive) | Sends children to queue and **exits** |
| Task count | O(nodes) = 10,000 | O(channel buffer) = ~20 |
| Memory footprint | 10,000 suspended futures | ~20 active futures |
| Scheduler overhead | High (polling thousands) | Minimal |
| Thread utilization | Good (threads yield, not block) | Good |
| **Total overhead** | **High** | **Low** |

**Both eventually complete**, but Concurrent DFS carries the weight of thousands of suspended tasks while Channel DFS keeps the active task count tiny.

**Channel DFS with 5 threads:**
```
Thread 1: fetch(0) → send → fetch(5) → send → fetch(10) → ...  [WORKING]
Thread 2: fetch(1) → send → fetch(6) → send → fetch(11) → ...  [WORKING]
Thread 3: fetch(2) → send → fetch(7) → send → fetch(12) → ...  [WORKING]
Thread 4: fetch(3) → send → fetch(8) → send → fetch(13) → ...  [WORKING]
Thread 5: fetch(4) → send → fetch(9) → send → fetch(14) → ...  [WORKING]
          ↑ All threads constantly working, minimal tasks alive
```

### Key takeaway

In resource-constrained environments (limited threads, limited memory), **Channel DFS dramatically outperforms Concurrent DFS** because:
1. Tasks complete immediately (fire-and-forget) instead of waiting
2. Only ~20 tasks alive vs 10,000 suspended tasks
3. Minimal scheduler and memory overhead
4. Maximum utilization of available resources
