
// --- Imports ---

use std::time::Duration;
use std::env;
use dotenvy::dotenv;
use serde::Deserialize;
use std::process;

// --- Data Structures & Configuration ---

/// Application configuration loaded from the environment
struct Config {
    local_rpc: String,
    remote_rpc: String,
    lag_threshold: u64,
    discord_webhook: String,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok(); // Load .env file if present, ignore if file is missing

         // Helper to parse optional u64, defaulting to 3
        let lag_threshold = env::var("LAG_THRESHOLD")
            .unwrap_or_else(|_| "3".to_string()) // Default to string "3"
            .parse::<u64>()
            .expect("LAG_THRESHOLD must be a valid number");

        Config {
            local_rpc: get_env("LOCAL_RPC_URL"),
            remote_rpc: get_env("REMOTE_RPC_URL"),
            lag_threshold,
            discord_webhook: get_env("DISCORD_WEBHOOK_URL"),
        }
    }     
}

#[derive(Deserialize, Debug)]
struct RpcResponse {
    result: String,
}


// --- Main Execution ---

#[tokio::main]
async fn main() {
    println!("eth-alive daemon starting up...");

    let config = Config::from_env();

    let client = reqwest::Client::new();

    println!("Configuration Loaded. Starting Watchdog Loop...");
    println!("  Local Node:  {}", config.local_rpc);
    println!("  Remote Node: {}", config.remote_rpc);
    println!("  Threshold:   {} blocks", config.lag_threshold);
    println!("  Webhook:     [REDACTED]");

    loop {
        // Fetch remote 
        let remote_result = fetch_block_number(&client, &config.remote_rpc).await;

        // Fetch local
        let local_result = fetch_block_number(&client, &config.local_rpc).await;

        // Compare and report
        match (remote_result, local_result) {
            (Ok(remote), Ok(local)) => {
                if local <= remote {
                    let lag = remote - local;
                    if lag < config.lag_threshold {
                        // Healthy 
                        println!("Synced! [Lag: {}] | Local: {} | Remote: {}", lag, local, remote);
                    } else {
                        // Lagging 
                        println!("Node lagging! [Lag: {}] | Local: {} | Remote: {}", lag, local, remote);
                    }
                } else {
                    // Local ahead, during a reorg or if remote is slow 
                    println!("Local is ahead (or remote is behind) | Local: {} | Remote: {}", local, remote);
                }
            }
            (Err(e), _) => {
                // If the remote fails, we can't judge health
                eprintln!("FAILED to fetch Remote RPC: {}", e);
            }
            (Ok(_), Err(e)) => {
                // If local failed, we can't judge health
                eprintln!("LOCAL NODE DOWN: {}", e);
                // TODO: Alerting 
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

// --- Helpers ---

fn get_env (key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        eprintln!("Error: Required environment variable '{}' not set.", key);
        process::exit(1);
    })
}

/// Performs 'eth_blockNumber' JSON-RPC call to the specified URL
async fn fetch_block_number(client: &reqwest::Client, url: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });

    let resp = client.post(url)
        .json(&payload)
        .send()
        .await?;

    // Parse the JSON answer into our struct
    let rpc_resp: RpcResponse = resp.json().await?;

    // Parse hex string (e.g., "0x10a") into u64
    let hex_str = rpc_resp.result.trim_start_matches("0x");
    let block_number = u64::from_str_radix(hex_str, 16)?;

    Ok(block_number)
}