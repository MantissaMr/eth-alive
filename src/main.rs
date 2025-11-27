
use std::time::Duration;
use std::process;
use std::env;
use dotenvy::dotenv;

// Holds immutable configuration values
struct Config {
    local_rpc: String,
    remote_rpc: String,
    discord_webhook: String,
}

fn get_env (key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        eprintln!("Error: Required environment variable '{}' not set.", key);
        process::exit(1);
    })
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok(); // Load .env file if present

        Config {
            local_rpc: get_env("LOCAL_RPC_URL"),
            remote_rpc: get_env("REMOTE_RPC_URL"),
            discord_webhook: get_env("DISCORD_WEBHOOK_URL"),
        }
    } 
    
}

#[tokio::main]
async fn main() {
    println!("eth-alive daemon starting up...");

    let config = Config::from_env();

    println!("Configuration Loaded:");
    println!("  Local Node:  {}", config.local_rpc);
    println!("  Remote Node: {}", config.remote_rpc);
    println!("  Webhook:     [REDACTED]");

    loop {
        println!("Pulse: Checking status...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}