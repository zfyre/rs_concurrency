use std::sync::{Arc, Mutex};
use tokio::sync::{Semaphore, SemaphorePermit};
use rand::Rng;

struct ReadWriteLock<T> {
    // Writer require two permissions -> 1) No writer accessing data, 2) No reader accessing data
    // Reader require one permission -> 1) No writer accessing data
    // Hence this can be implemented with two Semaphores
    data: Mutex<T>,
    reader_count: Mutex<usize>,
    room_empty: Semaphore,                 // 1 permit -> (only reader can enter)
    turnstile: Semaphore,                  // 1 permit -> (either reader or writer can enter)
}

struct WriteGuard<'a, T> {
    lock: &'a ReadWriteLock<T>,
    _turnstile_permit: SemaphorePermit<'a>,
    _room_empty_permit: SemaphorePermit<'a>
}

struct ReadGuard<'a, T> {
    lock: &'a ReadWriteLock<T>,
}

impl<T> ReadWriteLock<T> {
    fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            reader_count: Mutex::new(0),
            room_empty: Semaphore::new(1),
            turnstile: Semaphore::new(1),
        }
    }

    async fn read_lock(&self) -> ReadGuard<T> {
        let _turnstile = self.turnstile.acquire().await.unwrap();

        let is_first_reader = {
            let mut lock = self.reader_count.lock().unwrap();
            *lock += 1;
            *lock == 1
        };

        if is_first_reader {
            self.room_empty.acquire().await.unwrap().forget();
        }

        ReadGuard { lock: self }
    }

    async fn write_lock(&self) -> WriteGuard<T> {
        let turnstile_permit = self.turnstile.acquire().await.unwrap();
        let room_empty_permit = self.room_empty.acquire().await.unwrap();
        
        WriteGuard {
            lock: self,
            _turnstile_permit: turnstile_permit,
            _room_empty_permit: room_empty_permit
        }
    }
}

impl<T> ReadGuard<'_, T> {
    fn read(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock.data.lock().unwrap()
    }
}

impl<T> WriteGuard<'_, T> {
    fn write(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock.data.lock().unwrap()
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        let mut count = self.lock.reader_count.lock().unwrap();
        *count -= 1;
        if *count == 0 {
            self.lock.room_empty.add_permits(1);
        }
    }
}

// ============== Demo Utilities ==============

// 0 = Read, 1 = Write (+1), 2 = Write2 (+2)
fn generate_random_queue(len: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..len).map(|_| rng.gen_range(0..=2)).collect()
}

// Precompute expected values if queue ran sequentially
fn compute_expected_values(queue: &[u8]) -> Vec<i32> {
    let mut current_val = 0;
    queue.iter().map(|&action| {
        match action {
            0 => current_val, // Read sees current value
            1 => { current_val += 1; current_val }
            _ => { current_val += 2; current_val }
        }
    }).collect()
}

fn op_name(action: u8) -> &'static str {
    match action {
        0 => "Read",
        1 => "Write",
        _ => "Write2",
    }
}

fn print_header() {
    println!("\n{:=<70}", "");
    println!("{:^70}", "READ-WRITE LOCK DEMO");
    println!("{:=<70}\n", "");
    println!("{:-<70}", "");
    println!("{:^10} | {:^15} | {:^15} | {:^12}", "Queue Idx", "Op Type", "Expected Val", "Actual Val");
    println!("{:-<70}", "");
}

fn print_row(queue_idx: usize, op_type: &str, expected_val: i32, actual_val: i32) {
    println!("{:^10} | {:^15} | {:^15} | {:^12}", queue_idx, op_type, expected_val, actual_val);
}

fn print_footer(final_value: i32) {
    println!("{:-<70}", "");
    println!("\n{:^70}", format!("Final value: {}", final_value));
    println!("{:=<70}\n", "");
}

// ============== Main Run Function ==============

pub async fn run() {
    let rwlock = Arc::new(ReadWriteLock::new(0i32));
    
    let queue_len = 20;
    let queue = generate_random_queue(queue_len);
    let expected_vals = compute_expected_values(&queue);

    print_header();

    let mut handles = vec![];

    for (idx, &action) in queue.iter().enumerate() {
        let rwlock_clone = Arc::clone(&rwlock);
        let expected_val = expected_vals[idx];
        let op_type = op_name(action);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            let actual_val = match action {
                0 => {
                    // Read operation
                    let read_guard = rwlock_clone.read_lock().await;
                    *read_guard.read()
                }
                1 => {
                    // Write operation (+1)
                    let write_guard = rwlock_clone.write_lock().await;
                    let mut data = write_guard.write();
                    *data += 1;
                    *data
                }
                _ => {
                    // Write2 operation (+2)
                    let write_guard = rwlock_clone.write_lock().await;
                    let mut data = write_guard.write();
                    *data += 2;
                    *data
                }
            };
            print_row(idx, op_type, expected_val, actual_val);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let final_value = *rwlock.read_lock().await.read();
    print_footer(final_value);
}
