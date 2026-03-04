# Rust Concurrency: A Complete Guide for Students

## Table of Contents
1. [Introduction to Concurrency in Rust](#introduction)
2. [Data Races vs Race Conditions](#data-races-vs-race-conditions)
3. [The Send and Sync Traits](#send-and-sync-traits)
4. [Atomics and Memory Ordering](#atomics-and-memory-ordering)
5. [Practical Examples](#practical-examples)

---

## Introduction to Concurrency in Rust

### What Makes Rust Different?

Unlike many programming languages that force you into a specific way of handling concurrency (like Go's goroutines or Erlang's actor model), **Rust doesn't mandate a single concurrency model**. Instead, it gives you the building blocks and lets libraries figure out the best approach for different use cases.

### What Rust Provides

The standard library gives you:
- **OS threads**: The fundamental unit of concurrent execution
- **Blocking system calls**: Uniform abstractions that work across different operating systems

### The Philosophy

At Rust 1.0, the language designers faced a choice: pick one concurrency model (like message passing, green threads, or async APIs) or let the ecosystem evolve. They chose the latter because each approach has trade-offs.

Instead of picking winners, Rust provides:
1. **Lifetime system**: Enforces memory safety even with multiple threads
2. **`Send` trait**: Marks types that can be moved between threads
3. **`Sync` trait**: Marks types that can be shared across threads

This means you can build your own concurrency model as a library, and it will safely compose with other people's code.

---

## Data Races vs Race Conditions

### Understanding Data Races

A **data race** occurs when ALL three conditions are met:
1. Two or more threads access the same memory location
2. At least one access is a write
3. At least one access is unsynchronized

**Critical fact**: Data races cause **Undefined Behavior** and are **impossible to create in safe Rust**. The compiler won't let you.

### How Rust Prevents Data Races

Rust's ownership system prevents data races through a simple rule: **you cannot have multiple mutable references to the same data at the same time**. No aliasing + mutation = no data races.

### Race Conditions Are Different

A **race condition** (also called a resource race) happens when your program's behavior depends on the unpredictable timing of threads. Here's the key insight:

**Rust does NOT prevent race conditions, and that's okay!**

Why? Because race conditions alone cannot violate memory safety. They might cause:
- Deadlocks
- Incorrect results
- Unexpected behavior

But they won't cause:
- Memory corruption
- Undefined behavior
- Crashes (unless combined with unsafe code)

### Example: Safe Race Condition

```rust
use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

let data = vec![1, 2, 3, 4];
let idx = Arc::new(AtomicUsize::new(0));
let other_idx = idx.clone();

thread::spawn(move || {
    // This thread adds 10 to idx
    other_idx.fetch_add(10, Ordering::SeqCst);
});

// Race condition: idx might be 0 or 10
// If it's 10, this will panic (out of bounds)
// BUT: No memory corruption, just a controlled panic
println!("{}", data[idx.load(Ordering::SeqCst)]);
```

This is safe! The worst that happens is a panic, which is a controlled crash.

### Example: Unsafe Race Condition

```rust
if idx.load(Ordering::SeqCst) < data.len() {
    unsafe {
        // DANGER! The value of idx could change between
        // the check above and the access below
        println!("{}", data.get_unchecked(idx.load(Ordering::SeqCst)));
    }
}
```

This is dangerous because:
1. We check that `idx` is in bounds
2. Another thread changes `idx` to 10
3. We access `data[10]` without bounds checking
4. **Memory corruption!**

**Lesson**: Race conditions + unsafe code = potential memory unsafety.

---

## Send and Sync Traits

These are the cornerstone of Rust's concurrency safety. Let's break them down.

### What Are Send and Sync?

```rust
// Send: Safe to send to another thread
pub unsafe trait Send { }

// Sync: Safe to share between threads
// (T is Sync if &T is Send)
pub unsafe trait Sync { }
```

### Send: "Can I Move This?"

A type is `Send` if you can safely move it from one thread to another.

**Think of it like this**: If you put something in a box and mail it to a friend in another city, is that safe?
- A book? Yes (Send).
- A live wire? No (not Send).

Most types are Send:
- `String`, `Vec<T>`, `i32`, `Box<T>`
- Anything you can safely hand off to another thread

### Sync: "Can We Share This?"

A type is `Sync` if you can safely share references to it across multiple threads.

**The formal rule**: `T` is `Sync` if and only if `&T` is `Send`.

**Think of it like this**: Can multiple people safely look at something at the same time?
- A read-only document? Yes (Sync).
- A bank account being updated? Only with proper locking (not Sync without synchronization).

### Types That Are NOT Send or Sync

```rust
// Raw pointers: No safety guarantees
*const T
*mut T

// UnsafeCell: The basis of interior mutability
// Not Sync because it allows mutation through shared references
std::cell::UnsafeCell<T>

// Cell and RefCell: Built on UnsafeCell
// Not Sync because they're not thread-safe
std::cell::Cell<T>
std::cell::RefCell<T>

// Rc: Reference counting without atomic operations
// Not Send or Sync because the refcount isn't synchronized
std::rc::Rc<T>
```

### Why Are Raw Pointers Not Send/Sync?

Raw pointers (`*const T`, `*mut T`) aren't automatically marked thread-safe because:
1. They have complex, untracked ownership
2. They can point to thread-local storage
3. They can be dangling
4. They're a "lint" to prevent types containing them from being auto-derived as thread-safe

### Automatic Derivation

Here's the magic: **Send and Sync are automatically derived**.

If all fields of your struct are Send, your struct is Send. If all fields are Sync, your struct is Sync.

```rust
struct MyStruct {
    name: String,      // Send + Sync
    count: i32,        // Send + Sync
    data: Vec<u8>,     // Send + Sync
}
// MyStruct is automatically Send + Sync!
```

### Manual Implementation

Sometimes you need to manually implement Send/Sync for types containing raw pointers:

```rust
struct MyBox(*mut u8);

// SAFETY: MyBox owns its pointer exclusively
unsafe impl Send for MyBox {}

// SAFETY: MyBox has no interior mutability
unsafe impl Sync for MyBox {}
```

**Warning**: Getting this wrong causes Undefined Behavior!

### Removing Send/Sync (Negative Impls)

You can explicitly opt-out:

```rust
#![feature(negative_impls)]

struct SpecialThreadToken(u8);

impl !Send for SpecialThreadToken {}
impl !Sync for SpecialThreadToken {}
```

### Real Example: Custom Smart Pointer

```rust
use std::ptr::NonNull;

struct Carton<T>(NonNull<T>);

// SAFETY: Carton owns its data exclusively (like Box)
// If T is Send, moving the Carton moves the data
unsafe impl<T> Send for Carton<T> where T: Send {}

// SAFETY: Carton has no interior mutability
// All mutations require &mut self
unsafe impl<T> Sync for Carton<T> where T: Sync {}

impl<T> Drop for Carton<T> {
    fn drop(&mut self) {
        unsafe {
            // Clean up the heap allocation
            libc::free(self.0.as_ptr().cast());
        }
    }
}
```

### Special Case: MutexGuard

`MutexGuard` is interesting:
- **Not Send**: You must unlock the mutex on the same thread that locked it
- **Is Sync**: Sharing a `&MutexGuard` is fine because dropping a reference does nothing

---

## Atomics and Memory Ordering

Now we're getting into the deep stuff. Atomics are how you build lock-free concurrent data structures.

### The Problem: Two Kinds of Reordering

When you write:
```rust
x = 1;
y = 3;
x = 2;
```

You might think this executes in order. But:

1. **Compiler reordering**: The compiler might optimize to:
   ```rust
   x = 2;  // Why set x to 1 first?
   y = 3;
   ```

2. **Hardware reordering**: Even if the compiler doesn't reorder, the CPU might! Due to:
   - Out-of-order execution
   - Store buffers
   - Cache hierarchies

### Why This Matters

Consider this scenario:

```rust
// Thread 1          // Thread 2
y = 3;               if x == 1 {
x = 1;                   y *= 2;
                     }
```

Possible outcomes:
- `y = 3`: Thread 2 runs before Thread 1
- `y = 6`: Thread 2 runs after Thread 1
- `y = 2`: Thread 2 sees `x = 1` but not `y = 3` (reordering!)

The last one seems impossible, but it can happen on weakly-ordered hardware like ARM.

### Data Accesses vs Atomic Accesses

**Data Accesses** (normal variables):
- Unsynchronized
- Aggressively optimized
- Can be freely reordered
- Enable data races
- **Cannot be used for synchronization**

**Atomic Accesses**:
- Synchronized
- Have ordering constraints
- Tell the compiler and CPU "this is shared"
- Prevent data races
- **The foundation of lock-free programming**

### The Four Memory Orderings

Rust exposes four atomic orderings (from strongest to weakest):

#### 1. Sequentially Consistent (`SeqCst`)

**The strongest guarantee**: Everything happens in a global order.

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static FLAG: AtomicBool = AtomicBool::new(false);

// All threads agree on the order of these operations
FLAG.store(true, Ordering::SeqCst);
let value = FLAG.load(Ordering::SeqCst);
```

**Guarantees**:
- No reordering across this operation
- All threads see operations in the same order
- Strongest synchronization

**Cost**:
- Requires memory fences even on x86/64
- Most expensive

**When to use**:
- When you're not sure about correctness
- Default choice until proven otherwise

#### 2. Acquire-Release

**Moderate guarantee**: Establishes causality between specific threads.

**Acquire**: "All operations after this must stay after this"
```rust
let value = FLAG.load(Ordering::Acquire);
// Nothing from below this line can move above
```

**Release**: "All operations before this must stay before this"
```rust
// Nothing from above this line can move below
FLAG.store(true, Ordering::Release);
```

**The Key**: When Thread A releases and Thread B acquires the *same* atomic:
- Everything A did before the release is visible to B after the acquire
- Establishes a "happens-before" relationship

**Spinlock Example**:
```rust
use std::sync::atomic::{AtomicBool, Ordering};

struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    fn lock(&self) {
        // Acquire: Establishes synchronization
        while self.locked.compare_and_swap(false, true, Ordering::Acquire) {
            // Spin until we get the lock
        }
    }

    fn unlock(&self) {
        // Release: Makes our changes visible
        self.locked.store(false, Ordering::Release);
    }
}
```

**Cost**:
- Free on x86/64 (hardware is already strongly ordered)
- Cheaper than SeqCst on ARM and other weakly-ordered platforms

**When to use**:
- Locks and synchronization primitives
- Producer-consumer patterns
- When you need causality between specific threads

#### 3. Relaxed

**Weakest guarantee**: Only guarantees atomicity of the operation itself.

```rust
counter.fetch_add(1, Ordering::Relaxed);
```

**Guarantees**:
- The operation is atomic (no data race)
- No reordering constraints
- No happens-before relationship

**When to use**:
- Counters where you don't care about synchronization
- Statistics gathering
- When you're using other synchronization mechanisms

**Example**:
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// Safe: Just counting events, no synchronization needed
fn increment_counter() {
    COUNTER.fetch_add(1, Ordering::Relaxed);
}
```

**Cost**:
- Minimal overhead
- Most efficient atomic operation

#### 4. Consume (Not Available in Rust)

There's technically a fourth ordering in C++ called `Consume`, but it's not exposed in Rust because:
- It's extremely subtle and easy to get wrong
- Compiler support is incomplete
- Use `Acquire` instead

### Memory Ordering Cheat Sheet

| Ordering | Prevents Reordering | Establishes Causality | Use Case | Cost |
|----------|--------------------|-----------------------|----------|------|
| SeqCst | All | Yes (global) | Default/unsure | High |
| Acquire | Before → Before | Yes (paired) | Lock acquire | Medium |
| Release | After → After | Yes (paired) | Lock release | Medium |
| Relaxed | None | No | Counters | Low |

### Important Principles

1. **Start with SeqCst**: Only optimize to weaker orderings after proving correctness
2. **Test on ARM**: Code that works on x86/64 might be broken on weakly-ordered hardware
3. **Pair Acquire-Release**: They work together to establish synchronization
4. **Understand happens-before**: This is the fundamental relationship that prevents races

### Happens-Before Relationship

This is the key concept in understanding atomics:

```rust
// Thread A
data.store(42, Ordering::Relaxed);
flag.store(true, Ordering::Release);  // Everything above "happens-before" this

// Thread B
while !flag.load(Ordering::Acquire) { }  // This "happens-before" everything below
let value = data.load(Ordering::Relaxed);
assert_eq!(value, 42);  // Guaranteed to be 42!
```

The Acquire-Release pair creates a happens-before relationship:
- Thread A's write to `data` happens-before the Release
- The Acquire happens-before Thread B's read of `data`
- Therefore, Thread B sees Thread A's write

---

## Practical Examples

### Example 1: Thread-Safe Counter

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                // Relaxed: We only care about atomicity, not ordering
                counter.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Final count: {}", counter.load(Ordering::Relaxed));
    // Output: Final count: 10000
}
```

### Example 2: Simple Spinlock

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    fn new() -> Self {
        SpinLock {
            locked: AtomicBool::new(false),
        }
    }

    fn lock(&self) {
        // Acquire: Synchronize with previous release
        while self.locked.swap(true, Ordering::Acquire) {
            // Spin (busy-wait)
        }
    }

    fn unlock(&self) {
        // Release: Make our changes visible
        self.locked.store(false, Ordering::Release);
    }
}

fn main() {
    let lock = Arc::new(SpinLock::new());
    let mut handles = vec![];

    for i in 0..5 {
        let lock = Arc::clone(&lock);
        let handle = thread::spawn(move || {
            lock.lock();
            println!("Thread {} has the lock", i);
            // Critical section
            thread::sleep(std::time::Duration::from_millis(100));
            lock.unlock();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
```

### Example 3: Producer-Consumer with Flag

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

fn main() {
    let data = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));

    let data_clone = Arc::clone(&data);
    let ready_clone = Arc::clone(&ready);

    // Producer
    let producer = thread::spawn(move || {
        // Do some work
        thread::sleep(std::time::Duration::from_millis(100));

        // Write data
        data_clone.store(true, Ordering::Relaxed);

        // Signal that data is ready
        // Release: Ensures data write happens before this
        ready_clone.store(true, Ordering::Release);
    });

    // Consumer
    let consumer = thread::spawn(move || {
        // Wait for data
        // Acquire: Ensures we see the data write
        while !ready.load(Ordering::Acquire) {
            thread::sleep(std::time::Duration::from_millis(10));
        }

        // Read data (guaranteed to see the write)
        let value = data.load(Ordering::Relaxed);
        println!("Consumer saw: {}", value);
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}
```

---

## Key Takeaways

1. **Rust doesn't prevent race conditions**, only data races. Race conditions are safe; data races are undefined behavior.

2. **Send and Sync are the foundation** of safe concurrency in Rust. Understanding them is crucial.

3. **Atomics are subtle**. Start with `SeqCst`, optimize to weaker orderings only after proving correctness.

4. **Acquire-Release pairs** establish happens-before relationships between threads.

5. **Test on weakly-ordered hardware** if you're writing lock-free code. x86/64 masks many concurrency bugs.

6. **Use standard library types** (`Mutex`, `RwLock`, `Arc`) when possible. Only drop down to atomics when necessary.

7. **Unsafe code + race conditions = danger**. Be extra careful when mixing the two.

---

## Further Reading

- [The Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Rust Atomics and Locks](https://marabos.nl/atomics/) by Mara Bos
- [C++ Memory Ordering](https://en.cppreference.com/w/cpp/atomic/memory_order)
- [Rust Standard Library Atomic Types](https://doc.rust-lang.org/std/sync/atomic/)

---

**Remember**: Concurrent programming is hard. Start simple, use the standard library, and only optimize when you have proof it's necessary. Rust gives you the tools to do it safely, but you still need to understand the concepts!
