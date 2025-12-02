
// --- Imports ---

use std::time::Duration;
use std::env;
use dotenvy::dotenv;
use serde::Serialize;
use serde_json::Value; 
use std::process;
use chrono::{DateTime, Utc};
use colored::Colorize;
use url::Url;

// --- Data Structures & Configuration ---

/// Application configuration loaded from the environment
struct Config {
    local_rpc: String,
    remote_rpc: String,
    lag_threshold: u64,
    alert_cooldown_minutes: u64,
    poll_interval_seconds: u64,
    discord_webhook: String,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok(); // Load .env file if present, ignore if file is missing

        // optional LAG_THRESHOLD (u64), defaulting to 3
        let lag_threshold = env::var("LAG_THRESHOLD")
            .unwrap_or_else(|_| "3".to_string()) // Default to string "3"
            .parse::<u64>()
            .expect("LAG_THRESHOLD must be a valid number");
        
        // optional ALERT_COOLDOWN_MINUTES u64, defaulting to 15
        let alert_cooldown_minutes = env::var("ALERT_COOLDOWN_MINUTES")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<u64>()
            .expect("ALERT_COOLDOWN_MINUTES must be a valid number");
        // optional POLL_INTERVAL_SECONDS u64, defaulting to 60 secs 
        let poll_interval_seconds = env::var("POLL_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse::<u64>()
            .expect("POLL_INTERVAL_SECONDS must be a valid number");

        Config {
            local_rpc: get_env("LOCAL_RPC_URL"),
            remote_rpc: get_env("REMOTE_RPC_URL"),
            lag_threshold,
            alert_cooldown_minutes,
            poll_interval_seconds,
            discord_webhook: get_env("DISCORD_WEBHOOK_URL"),
        }
    }     
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client");

    println!("Configuration Loaded. Starting Watchdog Loop...");
    println!("{}", "-------------------------------------------------".dimmed());
    println!("  Local Node:        {}", redact_url(&config.local_rpc));
    println!("  Remote Node:       {}", redact_url(&config.remote_rpc));
    println!("  Threshold:         {} blocks", config.lag_threshold);
    println!("  Notif Cooldown:    {} minutes", config.alert_cooldown_minutes); 
    println!("  Polling:           Every {} seconds", config.poll_interval_seconds);

    let mut last_alert_time: Option<DateTime<Utc>> = None;
    let alert_cooldown = chrono::Duration::minutes(config.alert_cooldown_minutes as i64);

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
                        println!("[OK] Synced | Block: {} | Lag: {}", local, lag);
                        last_alert_time = None;
                    } else {
                        // Problem: Lagging too far behind
                        let msg = format!("🚨[WARN] NODE LAGGING! Local: {} | Remote: {} | Lag: {} blocks", local, remote, lag);
                        println!("{}", msg);

                        // Send alert, with cooldown check
                        process_alert(&client, &config.discord_webhook, &msg, &mut last_alert_time, alert_cooldown).await;   
                    }
                } else {
                        // Local ahead: a reorg or if remote is slow 
                        let lead = local - remote; 
                        println!("[INFO] Local is ahead | Local: {} | Remote: {} | Lead: {}", local, remote, lead);
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
        tokio::time::sleep(Duration::from_secs(config.poll_interval_seconds)).await;
    }
}

// --- Helpers ---

/// Fetches an environment variable or exits if not found
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

    // Send Request & Check HTTP Status
    let resp = client.post(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    // Parse as Generic JSON Value
    let body: Value = resp.json().await?;

    // Check for RPC error
    if let Some(err) = body.get("error") {
        let err_msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown RPC error");
        return Err(format!("RPC Error: {}", err_msg).into());
    }
    
    // Extract result 
    let result_str = body.get("result")
        .and_then(|v| v.as_str())
        .ok_or("Invalid response: 'result' field missing or not a string")?;
    
    // Parse Hex
    let block_number = parse_hex_to_u64(result_str)?;

    Ok(block_number)
}

/// Sends a Discord alert via webhook
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

/// Checks cooldown logic and sends an alert if necessary. Updates last_alert_time
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

/// Converts a hex string (with or without '0x' prefix) to u64
fn parse_hex_to_u64(hex: &str) -> Result<u64, std::num::ParseIntError> {
    let clean_hex = hex.trim_start_matches("0x");
    u64::from_str_radix(clean_hex, 16)
}

/// Hides the path/query of a URL to prevent leaking API keys in logs.
/// Input: https://eth-mainnet.alchemyapi.io/v2/SECRET
/// Output: https://eth-mainnet.alchemyapi.io/[REDACTED]

fn redact_url(url_str: &str) -> String {
    match Url::parse(url_str) {
        Ok(u) => {
            if u.has_host() {
                format!("{}://{}/[REDACTED]", u.scheme(), u.host_str().unwrap_or("unknown"))
            } else {
                "[INVALID URL]".to_string()
            }
        },
        Err(_) => "[INVALID URL]".to_string(),
    }
}


// --- TESTS ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing_with_prefix() {
        // 0x10a = 266
        let input = "0x10a";
        let result = parse_hex_to_u64(input);
        assert_eq!(result.unwrap(), 266);
    }

    #[test]
    fn test_hex_parsing_without_prefix() {
        // 10a = 266
        let input = "10a";
        let result = parse_hex_to_u64(input);
        assert_eq!(result.unwrap(), 266);
    }

    #[test]
    fn test_hex_parsing_uppercase() {
        // 0x10A = 266
        let input = "0x10A";
        let result = parse_hex_to_u64(input);
        assert_eq!(result.unwrap(), 266);
    }

    #[test]
    fn test_hex_parsing_zero() {
        let input = "0x0";
        let result = parse_hex_to_u64(input);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_invalid_hex() {
        let input = "0xZZZ"; // Not a hex number
        let result = parse_hex_to_u64(input);
        assert!(result.is_err());
    }
}