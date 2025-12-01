
// --- Imports ---

use std::time::Duration;
use std::env;
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::process;
use chrono::{DateTime, Utc};

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

/// Represents the JSON payload sent to Discord
#[derive(Serialize)]
struct DiscordBody {
    content: String,
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


    let mut last_alert_time: Option<DateTime<Utc>> = None;
    let alert_cooldown = chrono::Duration::seconds(30); // Change to minutes in production, use Duration::minutes(15)

    loop {
        let remote_result = fetch_block_number(&client, &config.remote_rpc).await;
        let local_result = fetch_block_number(&client, &config.local_rpc).await;
        match (remote_result, local_result) {

            // HEALTHY: Both RPCs responded
            (Ok(remote), Ok(local)) => {
                if local <= remote {
                    let lag = remote - local;
                    if lag < config.lag_threshold {
                        // All good: Print to terminal only
                        println!("[OK] Synced [Lag: {}]", lag);
                        last_alert_time = None;
                    } else {
                        // Problem: Lagging too far behind
                        let msg = format!("🚨[WARN] NODE LAGGING! Lag: {} blocks | Local: {} | Remote: {}", lag, local, remote);
                        println!("{}", msg);

                        process_alert(&client, &config.discord_webhook, &msg, &mut last_alert_time, alert_cooldown).await;   
                    }
                } else {
                        // Local ahead: a reorg or if remote is slow 
                        println!("[INFO] Local is ahead | Local: {} | Remote: {}", local, remote);
                    }
            }

            // REMOTE DIED: Skip health check (SoT is lost)
            (Err(e), _) => {
                eprintln!("[ERROR] FAILED to fetch Remote RPC: {}", e);
            }

            // LOCAL DIED: Node is down
            (Ok(_), Err(e)) => {
                let msg = format!("🚨[CRITICAL] LOCAL NODE DOWN! Error: {}", e);
                eprintln!("{}", msg);

                process_alert(&client, &config.discord_webhook, &msg, &mut last_alert_time, alert_cooldown).await;
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

async fn send_alert(client: &reqwest::Client, url: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    // If the URL is empty or the placeholder, don't try to send
    if url.is_empty() || url.contains("REDACTED") {
        return Ok(());
    }

    let payload = DiscordBody {
        content: message.to_string(),
    };

    client.post(url)
        .json(&payload)
        .send()
        .await?;

    Ok(())
}

/// Checks cooldown logic and sends an alert if necessary. Updates last_alert_time.
async fn process_alert(
    client: &reqwest::Client,
    webhook_url: &str,
    message: &str,
    last_alert_time: &mut Option<DateTime<Utc>>, 
    cooldown: chrono::Duration,
) {
    // Check if we should alert
    let should_alert = match last_alert_time {
        None => true,
        Some(last) => Utc::now() - *last > cooldown, 
    };

    if should_alert {
        if let Err(e) = send_alert(client, webhook_url, message).await {
            eprintln!("Error: Failed to send Discord alert: {}", e);
        } else {
            *last_alert_time = Some(Utc::now());
        }
    }
}