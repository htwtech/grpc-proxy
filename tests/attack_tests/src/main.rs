//! Attack test suite for solana-grpc-proxy.
//!
//! Tests various ways a malicious client might try to bypass per-IP
//! SubscribeRequest filtering:
//!
//! 1. Direct rejected account → must reject
//! 2. Valid then update to rejected (subscription update attack)
//! 3. Delayed subscribe (connect → wait 60s → send rejected)
//! 4. Empty initial frame then rejected (false first frame)
//! 5. Multiple rapid Subscribes on same stream
//! 6. Unknown IP → must reject
//!
//! Usage:
//!   cargo run -- --endpoint http://104.204.140.68:80 \
//!                --rejected pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA \
//!                --allowed J6wg7WSJcvZeBpbwgXAkVyCQS2R2TxdEbcsNCzdqxzYJ

use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tonic::Code;
use yellowstone_grpc_proto::geyser::{
    geyser_client::GeyserClient, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterTransactions,
};

#[derive(Parser, Debug)]
struct Args {
    /// Proxy endpoint, e.g. http://104.204.140.68:80
    #[arg(long)]
    endpoint: String,

    /// Account that MUST be rejected by proxy rules
    #[arg(long)]
    rejected: String,

    /// Account that must be allowed
    #[arg(long)]
    allowed: String,

    /// Number of the test to run; 0 = all
    #[arg(long, default_value_t = 0)]
    test: usize,
}

fn make_request(account_include: Vec<String>) -> SubscribeRequest {
    let mut transactions = HashMap::new();
    transactions.insert(
        "test".to_string(),
        SubscribeRequestFilterTransactions {
            account_include,
            account_exclude: vec![],
            account_required: vec![],
            ..Default::default()
        },
    );
    SubscribeRequest {
        transactions,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    }
}

async fn test_1_direct_rejected(args: &Args) -> Result<()> {
    println!("\n=== Test 1: Direct rejected account ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(make_request(vec![args.rejected.clone()])).await?;
    let req = tokio_stream::wrappers::ReceiverStream::new(rx);

    match client.subscribe(req).await {
        Ok(mut resp) => {
            let stream = resp.get_mut();
            match tokio::time::timeout(Duration::from_secs(3), stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    println!("  ❌ FAIL: received message: {:?}", msg.update_oneof.is_some());
                }
                Ok(Some(Err(status))) => {
                    println!("  ✅ PASS: stream rejected — code={:?} msg={}", status.code(), status.message());
                }
                Ok(None) => {
                    println!("  ✅ PASS: stream closed immediately");
                }
                Err(_) => {
                    println!("  ❌ FAIL: timeout waiting for reject");
                }
            }
        }
        Err(status) => {
            println!("  ✅ PASS: subscribe() failed — code={:?} msg={}", status.code(), status.message());
        }
    }
    Ok(())
}

async fn test_2_update_to_rejected(args: &Args) -> Result<()> {
    println!("\n=== Test 2: Valid first, then update to rejected ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);

    // First: valid request
    tx.send(make_request(vec![args.allowed.clone()])).await?;

    let req = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut resp = client.subscribe(req).await?;
    let stream = resp.get_mut();

    // Wait 2 seconds, then send update with rejected account
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("  -> sending update with rejected account");
    tx.send(make_request(vec![args.rejected.clone()])).await?;

    // Read messages for 5 seconds
    let start = std::time::Instant::now();
    let mut got_reject = false;
    let mut msg_count = 0;
    while start.elapsed() < Duration::from_secs(5) {
        match tokio::time::timeout(Duration::from_millis(500), stream.next()).await {
            Ok(Some(Ok(_))) => msg_count += 1,
            Ok(Some(Err(status))) => {
                println!("  ✅ PASS after update: code={:?} msg={}", status.code(), status.message());
                got_reject = true;
                break;
            }
            Ok(None) => {
                println!("  ✅ PASS: stream closed after update (msgs before close: {msg_count})");
                got_reject = true;
                break;
            }
            Err(_) => continue,
        }
    }
    if !got_reject {
        println!("  ❌ FAIL: update to rejected account was NOT blocked ({msg_count} msgs received)");
    }
    Ok(())
}

async fn test_3_delayed_subscribe(args: &Args) -> Result<()> {
    println!("\n=== Test 3: Connect, wait 30s, then send rejected ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let req = tokio_stream::wrappers::ReceiverStream::new(rx);

    let mut resp = client.subscribe(req).await?;
    let stream = resp.get_mut();

    println!("  -> waiting 30 seconds before sending");
    tokio::time::sleep(Duration::from_secs(30)).await;

    println!("  -> sending rejected SubscribeRequest");
    tx.send(make_request(vec![args.rejected.clone()])).await?;

    // Loop through messages until we get a reject or a transaction.
    // Pings are expected and ignored.
    let start = std::time::Instant::now();
    let mut got_reject = false;
    let mut got_tx = false;
    while start.elapsed() < Duration::from_secs(10) {
        match tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
            Ok(Some(Ok(msg))) => {
                use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
                match msg.update_oneof {
                    Some(UpdateOneof::Ping(_)) | Some(UpdateOneof::Pong(_)) => continue,
                    Some(UpdateOneof::Transaction(_)) => {
                        got_tx = true;
                        break;
                    }
                    _ => continue,
                }
            }
            Ok(Some(Err(status))) => {
                println!("  ✅ PASS: code={:?} msg={}", status.code(), status.message());
                got_reject = true;
                break;
            }
            Ok(None) => {
                println!("  ✅ PASS: stream closed");
                got_reject = true;
                break;
            }
            Err(_) => continue,
        }
    }
    if got_tx {
        println!("  ❌ FAIL: received transaction after delayed rejected");
    } else if !got_reject {
        println!("  ❌ FAIL: timeout — delayed rejected was not blocked");
    }
    Ok(())
}

async fn test_4_empty_then_rejected(args: &Args) -> Result<()> {
    println!("\n=== Test 4: Empty first request, then rejected ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);

    // Send empty SubscribeRequest (no filters)
    tx.send(SubscribeRequest::default()).await?;

    let req = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut resp = client.subscribe(req).await?;
    let stream = resp.get_mut();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Send rejected one
    println!("  -> sending rejected after empty");
    tx.send(make_request(vec![args.rejected.clone()])).await?;

    let mut got_reject = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        match tokio::time::timeout(Duration::from_millis(500), stream.next()).await {
            Ok(Some(Ok(msg))) => {
                if let Some(yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof::Transaction(_)) = msg.update_oneof {
                    println!("  ❌ FAIL: received transaction data with rejected filter");
                    break;
                }
            }
            Ok(Some(Err(status))) => {
                println!("  ✅ PASS: code={:?} msg={}", status.code(), status.message());
                got_reject = true;
                break;
            }
            Ok(None) => {
                println!("  ✅ PASS: stream closed");
                got_reject = true;
                break;
            }
            Err(_) => continue,
        }
    }
    if !got_reject {
        println!("  ❌ FAIL: no reject received");
    }
    Ok(())
}

async fn test_6_churn_attack(args: &Args) -> Result<()> {
    println!("\n=== Test 6: Rapid subscription updates (churn attack) ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    // Send first valid request
    tx.send(make_request(vec![args.allowed.clone()])).await?;

    let req = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut resp = client.subscribe(req).await?;
    let stream = resp.get_mut();

    // Spawn a concurrent sender task so we can listen to the stream
    // in parallel and catch the reject as soon as it arrives.
    let allowed = args.allowed.clone();
    let sender = tokio::spawn(async move {
        for _ in 0..30 {
            if tx.send(make_request(vec![allowed.clone()])).await.is_err() {
                break; // receiver closed
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    println!("  -> sending 30 rapid SubscribeRequest updates (concurrent)");

    // Listen for reject while sender is running
    let mut got_reject = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        match tokio::time::timeout(Duration::from_millis(200), stream.next()).await {
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(status))) => {
                if status.code() == Code::ResourceExhausted {
                    println!("  ✅ PASS: churn limit enforced — {}", status.message());
                } else {
                    println!("  ⚠  rejected with unexpected code {:?}: {}", status.code(), status.message());
                }
                got_reject = true;
                break;
            }
            Ok(None) => {
                println!("  ✅ PASS: stream closed");
                got_reject = true;
                break;
            }
            Err(_) => continue,
        }
    }
    let _ = sender.await;
    if !got_reject {
        println!("  ⚠  no reject after 30 rapid updates — churn protection may be disabled in rules");
    }
    Ok(())
}

async fn test_5_valid_allowed(args: &Args) -> Result<()> {
    println!("\n=== Test 5: Valid subscription (should work) ===");
    let mut client = GeyserClient::connect(args.endpoint.clone()).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(make_request(vec![args.allowed.clone()])).await?;

    let req = tokio_stream::wrappers::ReceiverStream::new(rx);
    let mut resp = client.subscribe(req).await?;
    let stream = resp.get_mut();

    let mut tx_count = 0;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        match tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
            Ok(Some(Ok(msg))) => {
                if let Some(yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof::Transaction(_)) = msg.update_oneof {
                    tx_count += 1;
                }
            }
            Ok(Some(Err(status))) => {
                println!("  ❌ FAIL: valid request rejected — {:?}", status);
                return Ok(());
            }
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    if tx_count > 0 {
        println!("  ✅ PASS: received {tx_count} transactions");
    } else {
        println!("  ⚠  got 0 transactions in 10s (may be normal if account is idle)");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("Target: {}", args.endpoint);
    println!("Rejected account: {}", args.rejected);
    println!("Allowed account: {}", args.allowed);

    let run_all = args.test == 0;

    if run_all || args.test == 1 {
        if let Err(e) = test_1_direct_rejected(&args).await {
            println!("  test 1 errored: {e:#}");
        }
    }
    if run_all || args.test == 2 {
        if let Err(e) = test_2_update_to_rejected(&args).await {
            println!("  test 2 errored: {e:#}");
        }
    }
    if run_all || args.test == 3 {
        if let Err(e) = test_3_delayed_subscribe(&args).await {
            println!("  test 3 errored: {e:#}");
        }
    }
    if run_all || args.test == 4 {
        if let Err(e) = test_4_empty_then_rejected(&args).await {
            println!("  test 4 errored: {e:#}");
        }
    }
    if run_all || args.test == 5 {
        if let Err(e) = test_5_valid_allowed(&args).await {
            println!("  test 5 errored: {e:#}");
        }
    }
    if run_all || args.test == 6 {
        if let Err(e) = test_6_churn_attack(&args).await {
            println!("  test 6 errored: {e:#}");
        }
    }

    Ok(())
}
