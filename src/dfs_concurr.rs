//! Concurrent DFS algorithms

use rand::Rng;
use tokio::sync::{Mutex, Semaphore};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time;
use futures::future::BoxFuture;
use futures::FutureExt;

type Graph = Vec<Vec<i32>>;

fn generate_graph(num_nodes: usize) -> Graph {
    let mut graph: Graph = vec![vec![]; num_nodes];
    let mut rng = rand::thread_rng();

    // Create a spanning tree to ensure connectivity
    for i in 1..num_nodes {
        let parent = rng.gen_range(0..i);
        graph[parent].push(i as i32);
    }

    // Add random edges to create cycles
    let num_extra_edges = rng.gen_range(3..=8);
    for _ in 0..num_extra_edges {
        let from = rng.gen_range(0..num_nodes);
        let to = rng.gen_range(0..num_nodes);
        if from != to && !graph[from].contains(&(to as i32)) {
            graph[from].push(to as i32);
        }
    }

    // Add deliberate back edges to create obvious cycles
    // Cycle: 0 -> 1 -> 2 -> 0
    if num_nodes >= 3 {
        if !graph[0].contains(&1) { graph[0].push(1); }
        if !graph[1].contains(&2) { graph[1].push(2); }
        if !graph[2].contains(&0) { graph[2].push(0); }
    }

    // Another cycle: 5 -> 6 -> 7 -> 5
    if num_nodes >= 8 {
        if !graph[5].contains(&6) { graph[5].push(6); }
        if !graph[6].contains(&7) { graph[6].push(7); }
        if !graph[7].contains(&5) { graph[7].push(5); }
    }

    graph
}

fn get_children(node: i32, graph: &Graph) -> std::slice::Iter<'_, i32> {
    // adding artificial time 
    std::thread::sleep(time::Duration::from_millis(2));

    graph[node as usize].iter()
}

// Async version - doesn't block runtime threads
async fn get_children_async(node: i32, graph: &Graph) -> Vec<i32> {
    // adding artificial time - non-blocking!
    tokio::time::sleep(time::Duration::from_millis(2)).await;

    graph[node as usize].iter().copied().collect()
}

pub async fn run() {

    let num_nodes = 10000;
    let graph: Graph = generate_graph(num_nodes);

    { // Sequential DFS code block
        let mut vis: Vec<bool> = vec![false; num_nodes];

        println!("Running 'DFS' for {} nodes", num_nodes);
        
        let start = time::Instant::now();
        seq_traverse(0, &mut vis, &graph);
        let end = time::Instant::now();

        println!("Time Taken: {:?} ms", time::Duration::from(end - start).as_millis());
    }
    { // Concurrent DFS code block
        let vis: Vec<bool> = vec![false; num_nodes];
        let mu_vis = Arc::new(Mutex::new(vis)); // Arc is shared pointer
        let graph_arc = Arc::new(graph.clone());
        let semaphore = Arc::new(Semaphore::new(5)); // Limit to 100 concurrent tasks

        println!("Running 'Concurrent DFS' for {} nodes", num_nodes);
        
        let start = time::Instant::now();
        concurr_traverse(0, graph_arc, mu_vis, semaphore).await;
        let end = time::Instant::now();

        println!("Time Taken: {:?} ms", time::Duration::from(end - start).as_millis());
    }  
    { // Channel DFS code block
        let vis: Vec<bool> = vec![false; num_nodes];
        let mu_vis = Arc::new(Mutex::new(vis));
        let graph_arc = Arc::new(graph.clone());

        println!("Running 'Channel DFS' for {} nodes", num_nodes);
        
        let start = time::Instant::now();
        channel_traverse(0, mu_vis, graph_arc).await;
        let end = time::Instant::now();

        println!("Time Taken: {:?} ms", time::Duration::from(end - start).as_millis());
    } 
}

// DFS
fn seq_traverse(node: i32, vis: &mut Vec<bool>, graph: &Graph) {

    if vis[node as usize] == true { return; }
    vis[node as usize] = true;
    // println!("Node {}", node);
    for &child in get_children(node, graph) {
        seq_traverse(child, vis, graph);
    }
}

// Concurrent DFS
fn concurr_traverse(
    node: i32,
    graph: Arc<Graph>,
    mu_vis: Arc<Mutex<Vec<bool>>>,
    semaphore: Arc<Semaphore>,
) -> BoxFuture<'static, ()> {
    async move {
        // Acquire semaphore permit - blocks if limit reached
        let permit = semaphore.acquire().await.unwrap();
        
        // Acquire lock, check and mark visited, then release
        {
            let mut guard = mu_vis.lock().await;
            if guard[node as usize] { return; }
            guard[node as usize] = true;
        } // lock released here

        // println!("Node {}", node);
        
        let children: Vec<i32> = get_children_async(node, &graph).await;
        let mut handles = vec![];
        
        for child in children {
            let graph_clone = Arc::clone(&graph);
            let mu_vis_clone = Arc::clone(&mu_vis);
            let sem_clone = Arc::clone(&semaphore);
            
            let handle = tokio::spawn(concurr_traverse(child, graph_clone, mu_vis_clone, sem_clone));
            handles.push(handle);
        }
        
        // Release permit BEFORE waiting for children to prevent deadlock
        drop(permit);
        
        // Wait for all child tasks to complete
        for handle in handles {
            let _ = handle.await;
        }
    }.boxed()
}


// Concurrency DFS using Channels
async fn channel_traverse(
    node: i32,
    mu_vis: Arc<Mutex<Vec<bool>>>,
    graph: Arc<Graph>,
){

    // At max 10 messages from different transmitter can be, receiver queue is at most 10 at a given time
    // We will send a list of nodes in this channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<i32>(5); 
    
    { // Marking first node as visited before sending
    let mut guard = mu_vis.lock().await;
    guard[node as usize] = true;
    }
    tx.send(node).await.unwrap(); // Send the first node value to the receiver via main transmitter
    

let active = Arc::new(AtomicUsize::new(1)); // Start at 1 (for initial node)

    loop {
        match rx.try_recv() {
            Ok(new_node) => {
                let graph_clone = Arc::clone(&graph);
                let mu_vis_clone = Arc::clone(&mu_vis);
                let tx_this = tx.clone();
                let active_clone = Arc::clone(&active);
                // println!("Node: {}", new_node);
                tokio::spawn(async move {
                    let children = get_children_async(new_node, &graph_clone).await;
                    for child in children {
                        let mut guard = mu_vis_clone.lock().await;
                        if guard[child as usize] { continue; }
                        guard[child as usize] = true;
                        drop(guard);
                        
                        active_clone.fetch_add(1, Ordering::SeqCst);  // +1 for new work
                        tx_this.send(child).await.unwrap();
                    }
                    active_clone.fetch_sub(1, Ordering::SeqCst);  // -1 when task done
                });
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                if active.load(Ordering::SeqCst) == 0 {
                    break;  // All work done
                }
                tokio::task::yield_now().await;  // Let other tasks run
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}