//! Optimistic Locking OR Atomics without Ordering Understanding!!! 

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::spawn;


const UNLOCKED: bool = false;
const LOCKED: bool = true;

struct Mutex<T> {
    locked: AtomicBool,
    v: UnsafeCell<T> // Interior Mutability
}

unsafe impl<T> Sync for Mutex<T> where T: Send {} 

impl<T> Mutex<T> {
    fn new(t: T) -> Self {
        Self {
            locked: AtomicBool::new(UNLOCKED),
            v: UnsafeCell::new(t)
        }
    }
    // Naive Spinlock Implementation - 1 
    fn with_lock_1<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        while self.locked.load(Ordering::Relaxed) != UNLOCKED {};
        // maybe another thread runs here -> TOCTOU (Time of Check and Time of Use Race Condition)
        std::thread::yield_now(); // Thread might get preempted here we are modelling what might happen in OS scheduling
        self.locked.store(LOCKED, Ordering::Relaxed);
        // Safety: we hold the lock threrefore we can create a mutable reference
        let ret = f(unsafe { &mut *self.v.get() });
        self.locked.store(UNLOCKED, Ordering::Relaxed);
        ret
    }
    // Naive Spinlock Implementation - 2
    fn with_lock_2<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        while self.locked.compare_exchange_weak( // Requires Exclusive access to the underlying memory location hence the ownership bounces between different threads -> Heavy Operation
            UNLOCKED,
            LOCKED,
            Ordering::Relaxed,
            Ordering::Relaxed
        ).is_err() {
            // MESI protocol
            while self.locked.load(Ordering::Relaxed) == LOCKED {} // load() DO NOT require EXCLUSINVE access just READ_ONLY access

            // x86: CAS (compare and Swap Instruction)
            // ARM: 
            //   LDREX -> (Load Exclusive: loads the value by taking exclusive owenership of memory),
            //   STREX -> (Store Exclusive: Only If i have the exclusive access to the memory (nobody else has taken it) only then it'll store) -> CHEAP OPERATION because no EX ownership is required of memory
            //      - compare_exchange: implemented using a loop of LDREX and STREX
            //      - compare_exchange_weak: LDREX STREX -> should be called when calling in loop because of spurious failures
        };
        // Safety: we hold the lock threrefore we can create a mutable reference
        let ret = f(unsafe { &mut *self.v.get() });
        self.locked.store(UNLOCKED, Ordering::Relaxed);
        ret
    }
}

pub async fn run(){

    let l = Arc::new(Mutex::new(0));
    let handles: Vec<_> = (0..100).map(|_| {
        let l_clone = Arc::clone(&l);
        spawn(move || {
            for _ in 0..1000 {
                l_clone.with_lock_2(|v| {
                    *v += 1;
                });      
            }
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }
    
    assert_eq!(l.with_lock_2(|v| *v), 100 * 1000);
}


#[test]
fn too_relaxed() {
    use std::sync::atomic::AtomicUsize;
    let x: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));
    let y: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));

    loop {
        let t1 = spawn(move ||{
            let r1 = y.load(Ordering::Relaxed);
            x.store(r1, Ordering::Relaxed);
            r1
        });
        
        let t2 = spawn(move ||{
            // These two lines can be are technically independent hence can be switched by the compiler or CPU
            // This is out of order executions!!!
            let r2 = x.load(Ordering::Relaxed);
            y.store(42, Ordering::Relaxed);
            r2
        });
        
        let r1 = t1.join().unwrap();
        let r2 = t2.join().unwrap();
        println!("r1: {}, r2: {}", r1, r2);
        if r1 == r2 && r1 ==  42 && r2 == 42{ break; }
    } // This will happen!! 

}