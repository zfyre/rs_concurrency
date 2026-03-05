//! Deadlock prevention using total ordering

use std::{mem::swap, sync::Arc, time::Duration};
use tokio::sync::Mutex;


const MAX_WORKERS: usize = 6;
const MAX_RESOURCES: usize = 3;

struct Resource {
    data: Mutex<String>
}
impl Resource {
    fn new(name: String) -> Self {
        let data = Mutex::new(name);
        Self {data}
    }
}

async fn mimic_load(resource: &Arc<Vec<Resource>>, worker_id: usize){
    
    loop {

        let mut rnd1 = rand::random::<usize>() % MAX_RESOURCES;
        let mut rnd2 = rand::random::<usize>() % MAX_RESOURCES;
        
        if rnd1 == rnd2 { continue;}

        /* 
        Infusing total Ordering to prevent Deadlocks logically without maintaining the resource allocation stuff
        */ 
        if rnd2 < rnd1 {
            swap(&mut rnd1, &mut rnd2);
        }

        println!("worker {} is trying to acquire lock on resource {}", worker_id, rnd1);
        let guard1 = resource[rnd1].data.lock().await;
        println!("worker {} acquired lock on resource {}", worker_id, rnd1);
        println!("worker {} is trying to acquire lock on resource {}", worker_id, rnd2);
        let guard2 = resource[rnd2].data.lock().await;
        println!("worker {} acquired lock on resource {}", worker_id, rnd2);
        
        tokio::time::sleep(Duration::from_secs(2)).await; // Mimic some processing on data
        
        drop(guard1);
        println!("worker {} released lock on resource {}", worker_id, rnd1);
        drop(guard2);
        println!("worker {} released lock on resource {}", worker_id, rnd2);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}


pub async fn run() {

    let mut record = Vec::<Resource>::new();
    for i in 0..MAX_RESOURCES {
        record.push(Resource::new(format!("resource {}", i)));
    }


    let record = Arc::new(record);
    let mut handles = vec![];
    for worker_id in 0..MAX_WORKERS {
        let record_clone = record.clone();
        let handle = tokio::spawn(async move { mimic_load(&record_clone, worker_id).await });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}



