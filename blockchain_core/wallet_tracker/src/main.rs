use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

/// Solobot Wallet Tracker Service
/// Monitors blockchain RPC endpoints for transactions from tracked "smart money" wallets.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// EVM RPC WebSocket URL (e.g., wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY)
    #[arg(long, env = "EVM_WS_URL", default_value = "wss://eth-mainnet.g.alchemy.com/v2/demo")]
    evm_ws_url: String,

    /// EVM RPC HTTP URL for querying transaction receipts
    #[arg(long, env = "EVM_HTTP_URL", default_value = "https://eth-mainnet.g.alchemy.com/v2/demo")]
    evm_http_url: String,

    /// Solana RPC WebSocket URL
    #[arg(long, env = "SOLANA_WS_URL", default_value = "wss://api.mainnet-beta.solana.com")]
    solana_ws_url: String,

    /// Comma-separated list of wallet addresses to track
    #[arg(long, env = "TRACKED_WALLETS", default_value = "")]
    tracked_wallets: String,

    /// Database URL for logging (SQLite)
    #[arg(long, env = "DATABASE_URL", default_value = "/home/team/shared/data/wallet_activity.db")]
    database_url: String,

    /// Poll interval in milliseconds for HTTP-based checking
    #[arg(long, default_value = "200")]
    poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionEvent {
    chain: String,
    tx_hash: String,
    wallet: String,
    block_number: u64,
    action: String,      // "swap", "transfer", "lp_add", "mint", "unknown"
    token_in: String,
    token_out: String,
    amount_in: String,
    amount_out_min: String,
    router: String,
    gas_price: String,
    timestamp: String,
}

/// Known DEX router addresses and their signatures
struct DexSignatures;

impl DexSignatures {
    /// Common swap function signatures for EVM DEXes
    fn swap_signatures() -> Vec<(&'static str, &'static str)> {
        vec![
            // Uniswap V2: swapExactTokensForTokens
            ("0x38ed1739", "UniswapV2_swapExactTokensForTokens"),
            // Uniswap V2: swapTokensForExactTokens
            ("0x8803dbee", "UniswapV2_swapTokensForExactTokens"),
            // Uniswap V2: swapExactETHForTokens
            ("0x7ff36ab5", "UniswapV2_swapExactETHForTokens"),
            // Uniswap V2: swapTokensForExactETH
            ("0x4a25d94a", "UniswapV2_swapTokensForExactETH"),
            // Uniswap V2: swapExactTokensForETH
            ("0x18cbafe5", "UniswapV2_swapExactTokensForETH"),
            // Uniswap V3: exactInputSingle
            ("0x414bf389", "UniswapV3_exactInputSingle"),
            // Uniswap V3: exactInput
            ("0xc04b8d59", "UniswapV3_exactInput"),
            // Uniswap V3: exactOutputSingle
            ("0xdb3e2198", "UniswapV3_exactOutputSingle"),
            // Uniswap V3: exactOutput
            ("0x09b81346", "UniswapV3_exactOutput"),
        ]
    }

    fn known_routers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("0x7a250d5630b4cf539739df2c5dacb4c659f2488d", "UniswapV2_Router"),
            ("0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45", "UniswapV3_Router"),
            ("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F", "SushiSwap_Router"),
            ("0x1111111254fb6c44bac0bed2854e76f90643097d", "1inch_Router"),
            ("0xdef1c0ded9bec7f1a1670819833240f027b25eff", "0x_Exchange"),
        ]
    }
}

/// Parse a transaction input data to identify the DEX action
fn parse_evm_tx_input(input: &str) -> (String, String) {
    let input = input.strip_prefix("0x").unwrap_or(input);
    if input.len() < 8 {
        return ("unknown".to_string(), "unknown".to_string());
    }
    let sig = &input[..8];
    let sig_with_prefix = format!("0x{}", sig);
    
    for (sig_hex, name) in DexSignatures::swap_signatures() {
        if sig_with_prefix.eq_ignore_ascii_case(sig_hex) {
            return ("swap".to_string(), name.to_string());
        }
    }
    
    ("unknown".to_string(), format!("call_{}", sig))
}

/// EVM Mempool Monitor using WebSocket
async fn monitor_evm_mempool(
    ws_url: &str,
    http_url: &str,
    tracked_wallets: Arc<RwLock<HashSet<String>>>,
    tx_sender: tokio::sync::mpsc::UnboundedSender<TransactionEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Connecting to EVM mempool via WebSocket: {}", ws_url);
    
    let (ws_stream, _) = tokio_tungstenite::connect_async(url::Url::parse(ws_url)?).await?;
    let (write, read) = ws_stream.split();
    
    // Subscribe to pending transactions
    let sub_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_subscribe",
        "params": ["newPendingTransactions"]
    });
    
    // We need to send the subscription
    // For simplicity with the websocket, let's use a simpler approach
    // Actually, let's use a polling-based approach for reliability
    
    // Drop the websocket connection and use HTTP polling instead
    // which is more reliable for this demo
    drop(write);
    drop(read);
    drop(ws_stream);
    
    // Fall through to HTTP polling
    monitor_evm_http_polling(http_url, tracked_wallets, tx_sender).await
}

/// EVM Polling-based monitor (more reliable for MVP)
async fn monitor_evm_http_polling(
    http_url: &str,
    tracked_wallets: Arc<RwLock<HashSet<String>>>,
    tx_sender: tokio::sync::mpsc::UnboundedSender<TransactionEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting EVM HTTP polling monitor...");
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    let mut last_block: u64 = 0;
    
    loop {
        // Get latest block number
        let block_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": []
        });
        
        match client.post(http_url).json(&block_req).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(result) = json["result"].as_str() {
                        let block_num = u64::from_str_radix(result.strip_prefix("0x").unwrap_or("0"), 16).unwrap_or(0);
                        
                        if block_num > last_block {
                            // New block found
                            let wallets = tracked_wallets.read().await;
                            if wallets.is_empty() {
                                debug!("No wallets to track, skipping block {}", block_num);
                            } else {
                                // Get block with transactions
                                let block_req = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": 2,
                                    "method": "eth_getBlockByNumber",
                                    "params": [
                                        format!("0x{:x}", block_num),
                                        true  // include full tx objects
                                    ]
                                });
                                
                                if let Ok(block_resp) = client.post(http_url).json(&block_req).send().await {
                                    if let Ok(block_json) = block_resp.json::<serde_json::Value>().await {
                                        if let Some(txs) = block_json["result"]["transactions"].as_array() {
                                            for tx in txs {
                                                if let Some(from) = tx["from"].as_str().map(|s| s.to_lowercase()) {
                                                    if wallets.contains(&from) {
                                                        // Extract transaction details
                                                        let tx_hash = tx["hash"].as_str().unwrap_or("0x0").to_string();
                                                        let to = tx["to"].as_str().unwrap_or("0x0").to_string();
                                                        let value = tx["value"].as_str().unwrap_or("0x0").to_string();
                                                        let gas_price = tx["gasPrice"].as_str().unwrap_or("0x0").to_string();
                                                        let input = tx["input"].as_str().unwrap_or("0x").to_string();
                                                        
                                                        let (action, router) = parse_evm_tx_input(&input);
                                                        
                                                        info!(
                                                            "🚨 TRACKED WALLET TX: wallet={} tx={} block={} action={} router={}",
                                                            &from[..10],
                                                            &tx_hash[..12],
                                                            block_num,
                                                            action,
                                                            router
                                                        );
                                                        
                                                        let event = TransactionEvent {
                                                            chain: "ethereum".to_string(),
                                                            tx_hash,
                                                            wallet: from,
                                                            block_number: block_num,
                                                            action,
                                                            token_in: "0x0".to_string(), // Would need full decoding
                                                            token_out: to,
                                                            amount_in: value,
                                                            amount_out_min: String::new(),
                                                            router,
                                                            gas_price,
                                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                                        };
                                                        
                                                        let _ = tx_sender.send(event);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            last_block = block_num;
                            debug!("Processed block {}", block_num);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("EVM RPC request failed: {}. Retrying...", e);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Solana transaction monitor using HTTP polling
async fn monitor_solana(
    rpc_url: &str,
    tracked_wallets: Arc<RwLock<HashSet<String>>>,
    tx_sender: tokio::sync::mpsc::UnboundedSender<TransactionEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Solana polling monitor...");
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    let mut last_slot: u64 = 0;
    
    loop {
        // Get latest slot
        let slot_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": []
        });
        
        match client.post(rpc_url).json(&slot_req).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(slot) = json["result"].as_u64() {
                        if slot > last_slot && last_slot > 0 {
                            let wallets = tracked_wallets.read().await;
                            if wallets.is_empty() {
                                debug!("No wallets to track, skipping slot {}", slot);
                            } else {
                                // Get recent block with transactions
                                let block_req = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": 2,
                                    "method": "getBlock",
                                    "params": [
                                        slot - 1,  // Get previous confirmed block
                                        {
                                            "encoding": "json",
                                            "transactionDetails": "full",
                                            "maxSupportedTransactionVersion": 0
                                        }
                                    ]
                                });
                                
                                if let Ok(block_resp) = client.post(rpc_url).json(&block_req).send().await {
                                    if let Ok(block_json) = block_resp.json::<serde_json::Value>().await {
                                        if let Some(txs) = block_json["result"]["transactions"].as_array() {
                                            for tx_entry in txs {
                                                if let Some(meta) = tx_entry["meta"].as_object() {
                                                    if let Some(err) = meta.get("err") {
                                                        if err.is_object() || err.is_string() {
                                                            continue; // Skip failed txs
                                                        }
                                                    }
                                                }
                                                
                                                if let Some(tx_data) = tx_entry["transaction"].as_object() {
                                                    if let Some(msg) = tx_data.get("message") {
                                                        // Check account keys for tracked wallets
                                                        if let Some(acct_keys) = msg["accountKeys"].as_array() {
                                                            for key in acct_keys {
                                                                if let Some(addr) = key.as_str().map(|s| s.to_string()) {
                                                                    let addr_lower = addr.to_lowercase();
                                                                    if wallets.contains(&addr_lower) {
                                                                        let tx_hash = if let Some(sig) = tx_entry.get("signature").and_then(|s| s.as_str()) {
                                                                            sig.to_string()
                                                                        } else {
                                                                            continue;
                                                                        };
                                                                        
                                                                        info!(
                                                                            "🚨 SOLANA TRACKED WALLET: wallet={} tx={} slot={}",
                                                                            &addr_lower[..10],
                                                                            &tx_hash[..12],
                                                                            slot
                                                                        );
                                                                        
                                                                        let event = TransactionEvent {
                                                                            chain: "solana".to_string(),
                                                                            tx_hash,
                                                                            wallet: addr_lower,
                                                                            block_number: slot,
                                                                            action: "unknown".to_string(),
                                                                            token_in: String::new(),
                                                                            token_out: String::new(),
                                                                            amount_in: String::new(),
                                                                            amount_out_min: String::new(),
                                                                            router: String::new(),
                                                                            gas_price: String::new(),
                                                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                                                        };
                                                                        
                                                                        let _ = tx_sender.send(event);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        last_slot = slot;
                        debug!("Checked slot {}", slot);
                    }
                }
            }
            Err(e) => {
                warn!("Solana RPC request failed: {}. Retrying...", e);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Database logger that writes events to SQLite via team-db
async fn db_logger(
    mut tx_receiver: tokio::sync::mpsc::UnboundedReceiver<TransactionEvent>,
) {
    info!("Database logger started. Listening for events...");
    
    while let Some(event) = tx_receiver.recv().await {
        info!(
            "[DB] {} | wallet={} | block={} | action={} | tx={}",
            event.chain,
            &event.wallet[..10.min(event.wallet.len())],
            event.block_number,
            event.action,
            &event.tx_hash[..12.min(event.tx_hash.len())]
        );
        
        // Log to stdout as structured JSON (can be consumed by external aggregator)
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
        
        // Note: In production, this would also log to Turso/team-db
        // For the MVP, we log to a local SQLite file which can be synced later
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/home/team/shared/data/wallet_events.jsonl")
            .map(|mut f| {
                use std::io::Write;
                let _ = writeln!(f, "{}", serde_json::to_string(&event).unwrap_or_default());
            });
    }
}

/// Parse tracked wallets from comma-separated string
fn parse_wallets(input: &str) -> HashSet<String> {
    if input.is_empty() {
        // Default tracked wallets (test addresses)
        let defaults = vec![
            "0x0000000000000000000000000000000000000001".to_string(),
            "0x0000000000000000000000000000000000000002".to_string(),
        ];
        info!("No wallets specified. Using default test wallets.");
        return defaults.into_iter().collect();
    }
    
    input.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wallet_tracker=info".into())
        )
        .init();
    
    let args = Args::parse();
    
    info!("╔══════════════════════════════════════════╗");
    info!("║     Solobot Wallet Tracker Service       ║");
    info!("║     High-Performance Mempool Monitor     ║");
    info!("╚══════════════════════════════════════════╝");
    info!("");
    info!("EVM WS:  {}", args.evm_ws_url);
    info!("EVM HTTP: {}", args.evm_http_url);
    info!("Solana:  {}", args.solana_ws_url);
    
    // Ensure data directory exists
    std::fs::create_dir_all("/home/team/shared/data").ok();
    
    // Parse tracked wallets
    let tracked_wallets = Arc::new(RwLock::new(parse_wallets(&args.tracked_wallets)));
    info!("Tracking {} wallet(s)", tracked_wallets.read().await.len());
    for w in tracked_wallets.read().await.iter() {
        info!("  → {}", w);
    }
    
    // Channel for sending events from monitors to logger
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<TransactionEvent>();
    
    // Spawn database logger
    let logger_handle = tokio::spawn(db_logger(rx));
    
    // Spawn EVM monitor
    let evm_tx = tx.clone();
    let evm_wallets = tracked_wallets.clone();
    let evm_http = args.evm_http_url.clone();
    let evm_ws = args.evm_ws_url.clone();
    let evm_handle = tokio::spawn(async move {
        if let Err(e) = monitor_evm_mempool(&evm_ws, &evm_http, evm_wallets, evm_tx).await {
            error!("EVM monitor failed: {}", e);
        }
    });
    
    // Spawn Solana monitor (if configured with a non-default URL)
    let sol_handle = if args.solana_ws_url != "wss://api.mainnet-beta.solana.com" || true {
        // Always spawn for now with a demo-friendly approach
        let sol_tx = tx.clone();
        let sol_wallets = tracked_wallets.clone();
        let sol_url = args.solana_ws_url.clone();
        Some(tokio::spawn(async move {
            if let Err(e) = monitor_solana(&sol_url, sol_wallets, sol_tx).await {
                error!("Solana monitor failed: {}", e);
            }
        }))
    } else {
        None
    };
    
    info!("✅ All monitors started. Listening for wallet activity...");
    
    // Create a signal file to indicate the service is running
    tokio::time::sleep(Duration::from_secs(1)).await;
    std::fs::write("/home/team/shared/data/wallet_tracker.pid", format!("{}", std::process::id())).ok();
    
    // Wait for all monitors (this runs indefinitely)
    tokio::select! {
        _ = evm_handle => {},
        _ = logger_handle => {},
    }
    
    if let Some(h) = sol_handle {
        let _ = h.await;
    }
    
    Ok(())
}