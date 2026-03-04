# Read-Write Lock Implementation Notes

## Overview

This implementation solves the classic **Readers-Writers Problem** using Tokio semaphores with writer-priority fairness.

**Key invariants:**
- Multiple readers can access data simultaneously
- Only one writer can access data at a time
- Writers have priority to prevent starvation

## Core Components

### Semaphores Used

```rust
room_empty: Semaphore::new(1)   // Controls access to the "room" (data)
turnstile: Semaphore::new(1)    // Prevents writer starvation
reader_count: Mutex<usize>      // Tracks active readers
```

## The `forget()` + `add_permits()` Pattern

### Problem
In a readers-writers lock, the **first reader** must acquire exclusive access (blocking writers), but we don't want to store a `SemaphorePermit` for an indefinite time across multiple readers.

### Solution: The Lightswitch Pattern

**First reader enters:**
```rust
self.room_empty.acquire().await.unwrap().forget();
```

- `acquire()` returns a `SemaphorePermit`
- `forget()` **consumes the permit without releasing it back** to the semaphore
- The semaphore now has 0 available permits
- Writers calling `room_empty.acquire()` will block

**Last reader leaves:**
```rust
self.lock.room_empty.add_permits(1);
```

- `add_permits(1)` **restores** the permit to the semaphore
- Writers can now acquire the permit

### Why not just hold the permit?
Storing `SemaphorePermit` requires lifetime management. The permit would need to live as long as any reader is active, which is complex when readers come and go independently. `forget()` + `add_permits()` decouples the permit lifecycle from any single reader.

## Tokio Semaphore Queue Behavior

When you call `acquire()` on a Tokio `Semaphore`:

1. **If permits available** → Caller gets one immediately
2. **If no permits available** → Caller is added to an internal **FIFO queue** and suspends

When a permit becomes available (via `drop` or `add_permits()`):
- The **first waiter in the queue** is woken up
- Waiters are served **in order** (fair scheduling)

This FIFO behavior is crucial for the turnstile pattern.

## The Turnstile Pattern (Writer Starvation Prevention)

### Problem
Without protection, a continuous stream of readers could starve writers indefinitely:
```
Reader1 → Reader2 → Reader3 → Reader4 → ...  (Writer never gets access)
```

### Solution: The Turnstile

The turnstile is a semaphore that **both readers and writers must pass through**:

**Reader path:**
```rust
let _turnstile = self.turnstile.acquire().await.unwrap();
// ... increment reader_count, acquire room_empty if first ...
// turnstile is released here (dropped)
```

**Writer path:**
```rust
let turnstile_permit = self.turnstile.acquire().await.unwrap();
let room_empty_permit = self.room_empty.acquire().await.unwrap();
// Writer HOLDS turnstile until done
```

### How it prevents starvation

```
Time →

Reader1: acquires turnstile → releases → working...
Reader2: acquires turnstile → releases → working...
Writer:  acquires turnstile (HOLDS IT) → waiting on room_empty...
Reader3: waiting on turnstile (queued behind Writer)
Reader4: waiting on turnstile (queued behind Reader3)

[Reader1 & Reader2 finish, last one calls add_permits(1)]

Writer:  acquires room_empty → working...
Writer:  done, releases both permits

Reader3: acquires turnstile → continues...
Reader4: acquires turnstile → continues...
```

Because Tokio's semaphore uses a **FIFO queue**, Reader3 and Reader4 must wait behind the Writer. New readers can't "jump the line."

## Complete Flow Diagrams

### Read Lock Acquisition

```
read_lock() called
    │
    ▼
acquire turnstile ──────────────────┐
    │                               │
    ▼                               │ (released immediately
lock reader_count mutex             │  after this block)
    │                               │
    ▼                               │
increment count                     │
    │                               │
    ▼                               │
if count == 1:                      │
    acquire room_empty.forget() ◄───┘
    │
    ▼
return ReadGuard
```

### Read Lock Release (Drop)

```
ReadGuard dropped
    │
    ▼
lock reader_count mutex
    │
    ▼
decrement count
    │
    ▼
if count == 0:
    add_permits(1) to room_empty
```

### Write Lock Acquisition

```
write_lock() called
    │
    ▼
acquire turnstile (HOLD)
    │
    ▼
acquire room_empty (HOLD)
    │
    ▼
return WriteGuard { both permits }
```

### Write Lock Release (Drop)

```
WriteGuard dropped
    │
    ▼
Both SemaphorePermits dropped automatically
    │
    ▼
Permits returned to respective semaphores
```

## Key Insights

1. **`forget()` is safe here** because we balance it with `add_permits()`. The total permit count is preserved over the reader lifecycle.

2. **Writers hold the turnstile** while waiting for `room_empty`. This blocks new readers from entering.

3. **Readers release the turnstile immediately** after passing through, allowing other readers to follow (as long as no writer is waiting).

4. **FIFO queue guarantees fairness**. Without it, readers could continuously steal the turnstile from a waiting writer.
