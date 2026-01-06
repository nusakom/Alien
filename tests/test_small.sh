#!/bin/bash
# Quick test with fewer operations

cd /home/ubuntu2204/Desktop/elle_dbfs_client

# Create a test version with only 8 operations and 2 concurrency
cat > src/test_main.rs << 'EOF'
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Serialize, Deserialize};

mod dbfs_client;
use dbfs_client::DbfsClient;

pub struct AsyncDbfsClient {
    client: Arc<Mutex<dbfs_client::DbfsClient>>,
    next_tx_id: Arc<AtomicU64>,
}

impl AsyncDbfsClient {
    pub async fn new(addr: &str) -> anyhow::Result<Self> {
        let addr = addr.to_string();
        let client = tokio::task::spawn_blocking(move || {
            dbfs_client::DbfsClient::connect(&addr)
        }).await??;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            next_tx_id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn begin_tx(&self) -> anyhow::Result<u64> {
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);
        let client = self.client.clone();
        let resp = tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.begin_tx(tx_id)
        }).await??;
        println!("TX-{}: begin -> LSN={}", tx_id, resp.lsn);
        Ok(resp.lsn)
    }

    pub async fn commit_tx(&self, tx_id: u64) -> anyhow::Result<u64> {
        let client = self.client.clone();
        let resp = tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.commit_tx(tx_id)
        }).await??;
        println!("TX-{}: commit -> LSN={}", tx_id, resp.lsn);
        Ok(resp.lsn)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Quick Test: 2 transactions, 2 concurrent tasks");
    let addr = "127.0.0.1:12345";

    // Task 1
    let task1 = tokio::spawn(async move {
        let client = AsyncDbfsClient::new(addr).await?;
        println!("Task 1: Starting");
        let lsn = client.begin_tx().await?;
        client.commit_tx(1).await?;
        println!("Task 1: Complete");
        Ok::<(), anyhow::Error>(())
    });

    // Task 2
    let task2 = tokio::spawn(async move {
        let client = AsyncDbfsClient::new(addr).await?;
        println!("Task 2: Starting");
        let lsn = client.begin_tx().await?;
        client.commit_tx(2).await?;
        println!("Task 2: Complete");
        Ok::<(), anyhow::Error>(())
    });

    task1.await??;
    task2.await??;

    println!("All tasks completed!");
    Ok(())
}
EOF

# Compile and run
echo "Compiling test..."
rustc --edition 2021 -o test_client src/test_main.rs -L target/release/deps --extern tokio=target/release/libtokio.rlib --extern serde=target/release/libserde.rlib --extern bincode=target/release/deps/libbincode*.rlib --extern anyhow=target/release/libanyhow.rlib 2>&1 || {
    echo "Compilation failed, using alternative method"
    # Just run the normal client with a note
    echo "Running normal client with short timeout..."
    timeout 10 ./target/release/elle_dbfs_client 2>&1 | head -30
}
