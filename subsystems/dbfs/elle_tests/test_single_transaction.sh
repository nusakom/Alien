#!/bin/bash
# Simple test: Single transaction to verify connectivity

cd $(cd "$SCRIPT_DIR/../../.." /home/ubuntu2204/Desktop/elle_dbfs_client/home/ubuntu2204/Desktop/elle_dbfs_client pwd)/elle_dbfs_client

# Temporarily create a test client with reduced operations
cat > /tmp/test_main.rs << 'EOF'
// [...] existing imports [...]

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Single Transaction Test");
    let addr = "127.0.0.1:12345";
    let client = Arc::new(AsyncDbfsClient::new(addr).await?);

    // Single transaction
    let lsn = client.begin_tx().await?;
    println!("✅ Transaction started, LSN: {}", lsn);

    let files = client.readdir(1, "/").await?;
    println!("✅ Readdir: {} files", files.len());

    client.create_file(1, "/test-file").await?;
    println!("✅ File created");

    let lsn2 = client.commit_tx(1).await?;
    println!("✅ Transaction committed, LSN: {}", lsn2);

    Ok(())
}
EOF

# Use the existing client with timeout
echo "Testing single transaction..."
timeout 10 ./target/release/elle_dbfs_client 2>&1 | head -20
