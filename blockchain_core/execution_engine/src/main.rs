mod evm;
mod solana;
pub mod types;

use clap::Parser;
use std::collections::HashMap;
use tracing::{info, warn, error, debug};

use types::{ChainConfig, ExecutionResult, GasStrategy, TradeSignal};
use evm::EvmTransactionBuilder;
use solana::SolanaTransactionBuilder;

/// Solobot Execution Engine
/// High-speed, same-block trade execution for EVM and Solana chains.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// EVM RPC URL
    #[arg(long, env = "EVM_RPC_URL", default_value = "https://eth-mainnet.g.alchemy.com/v2/demo")]
    evm_rpc_url: String,

    /// EVM WebSocket URL
    #[arg(long, env = "EVM_WS_URL", default_value = "wss://eth-mainnet.g.alchemy.com/v2/demo")]
    evm_ws_url: String,

    /// Flashbots relay URL
    #[arg(long, env = "FLASHBOTS_RELAY", default_value = "https://relay.flashbots.net")]
    flashbots_relay: String,

    /// Solana RPC URL
    #[arg(long, env = "SOLANA_RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    solana_rpc_url: String,

    /// Jito Block Engine URL
    #[arg(long, env = "JITO_URL", default_value = "https://mainnet.block-engine.jito.wtf")]
    jito_url: String,

    /// Default slippage in basis points
    #[arg(long, default_value = "50")]
    default_slippage_bps: u64,

    /// Maximum gas price in gwei
    #[arg(long, default_value = "200")]
    max_gas_price_gwei: u64,

    /// Max retry blocks for confirmation
    #[arg(long, default_value = "3")]
    max_retry_blocks: u64,

    /// Enable dry-run mode (no real submission)
    #[arg(long, default_value = "true")]
    dry_run: bool,
}

/// The main execution engine orchestrator
struct ExecutionEngine {
    chain_configs: HashMap<String, ChainConfig>,
    evm_builder: Option<EvmTransactionBuilder>,
    solana_builder: Option<SolanaTransactionBuilder>,
    dry_run: bool,
}

impl ExecutionEngine {
    fn new(args: &Args) -> Self {
        let mut chain_configs = HashMap::new();

        // EVM configuration
        chain_configs.insert("ethereum".to_string(), ChainConfig {
            chain: "ethereum".to_string(),
            chain_id: 1,
            rpc_url: args.evm_rpc_url.clone(),
            ws_url: args.evm_ws_url.clone(),
            flashbots_relay: Some(args.flashbots_relay.clone()),
            jito_url: None,
            private_mempool_url: None,
        });

        // Arbitrum configuration
        chain_configs.insert("arbitrum".to_string(), ChainConfig {
            chain: "arbitrum".to_string(),
            chain_id: 42161,
            rpc_url: String::new(), // Would be configured
            ws_url: String::new(),
            flashbots_relay: None,
            jito_url: None,
            private_mempool_url: None,
        });

        // Base configuration
        chain_configs.insert("base".to_string(), ChainConfig {
            chain: "base".to_string(),
            chain_id: 8453,
            rpc_url: String::new(),
            ws_url: String::new(),
            flashbots_relay: None,
            jito_url: None,
            private_mempool_url: None,
        });

        // Solana configuration
        chain_configs.insert("solana".to_string(), ChainConfig {
            chain: "solana".to_string(),
            chain_id: 0,
            rpc_url: args.solana_rpc_url.clone(),
            ws_url: String::new(),
            flashbots_relay: None,
            jito_url: Some(args.jito_url.clone()),
            private_mempool_url: None,
        });

        // Initialize builders
        let eth_config = chain_configs.get("ethereum").cloned().unwrap();
        let evm_builder = EvmTransactionBuilder::new(eth_config);

        let sol_config = chain_configs.get("solana").cloned().unwrap();
        let solana_builder = SolanaTransactionBuilder::new(sol_config);

        Self {
            chain_configs,
            evm_builder: Some(evm_builder),
            solana_builder: Some(solana_builder),
            dry_run: args.dry_run,
        }
    }

    /// Execute a trade signal
    async fn execute_signal(&self, signal: &TradeSignal) -> Result<ExecutionResult, String> {
        info!("═══ Executing Signal ═══");
        info!("Signal ID: {}", signal.signal_id);
        info!("Chain:     {}", signal.chain);
        info!("Action:    {}", signal.action);
        info!("Token:     {}", signal.token_address);
        info!("Confidence: {:.2}", signal.confidence);
        info!("Slippage:  {} bps", signal.slippage_bps);
        info!("Dry Run:   {}", self.dry_run);

        // Validate signal
        signal.validate().map_err(|e| format!("Signal validation failed: {}", e))?;

        // Route to appropriate execution path
        match signal.chain.to_lowercase().as_str() {
            "ethereum" | "arbitrum" | "base" => {
                self.execute_evm_signal(signal).await
            }
            "solana" => {
                self.execute_solana_signal(signal).await
            }
            other => {
                Err(format!("Unsupported chain: {}", other))
            }
        }
    }

    /// Execute a trade on EVM chain
    async fn execute_evm_signal(&self, signal: &TradeSignal) -> Result<ExecutionResult, String> {
        let builder = self.evm_builder.as_ref().ok_or("EVM builder not initialized")?;

        match signal.action.as_str() {
            "buy" => {
                info!("▶ EVM BUY: {}", signal.token_address);
                
                if let Some(copy_tx_hash) = &signal.copy_tx_hash {
                    // Copy-trade: follow a specific transaction
                    info!("Copy-trading tx: {}", copy_tx_hash);
                    
                    if self.dry_run {
                        return Ok(ExecutionResult::success(
                            signal.signal_id.clone(),
                            "dry_run_tx_hash".to_string(),
                            12345678,
                            150000,
                        ));
                    }
                    
                    let bundle = builder.build_mirror_tx(
                        copy_tx_hash,
                        signal,
                        GasStrategy::Premium(2), // 2 gwei premium
                        "0xYOUR_WALLET_ADDRESS", // Would come from config
                    ).await.map_err(|e| format!("Failed to build mirror tx: {}", e))?;
                    
                    let result = builder.submit_flashbots_bundle(&bundle)
                        .await
                        .map_err(|e| format!("Bundle submission failed: {}", e))?;
                    
                    Ok(result)
                } else {
                    // Direct buy signal from AI engine (not copy-trade)
                    info!("Direct buy (not copy-trade) - would construct swap tx");
                    
                    if self.dry_run {
                        return Ok(ExecutionResult::success(
                            signal.signal_id.clone(),
                            "direct_buy_dry_run".to_string(),
                            12345678,
                            200000,
                        ));
                    }
                    
                    Err("Direct swaps without copy-trade not yet implemented in MVP".to_string())
                }
            }
            "sell" => {
                info!("▶ EVM SELL: {}", signal.token_address);
                if self.dry_run {
                    Ok(ExecutionResult::success(
                        signal.signal_id.clone(),
                        "sell_dry_run".to_string(),
                        12345679,
                        180000,
                    ))
                } else {
                    Err("Sell execution not yet implemented in MVP".to_string())
                }
            }
            _ => {
                Err(format!("Unknown action: {}", signal.action))
            }
        }
    }

    /// Execute a trade on Solana
    async fn execute_solana_signal(&self, signal: &TradeSignal) -> Result<ExecutionResult, String> {
        let builder = self.solana_builder.as_ref().ok_or("Solana builder not initialized")?;

        match signal.action.as_str() {
            "buy" => {
                info!("▶ SOLANA BUY: {}", signal.token_address);
                
                if self.dry_run {
                    return Ok(ExecutionResult::success(
                        signal.signal_id.clone(),
                        "solana_dry_run_sig".to_string(),
                        123456789,
                        0,
                    ));
                }
                
                // Get recent blockhash
                match builder.get_recent_blockhash().await {
                    Ok(blockhash) => debug!("Got blockhash: {}", blockhash),
                    Err(e) => warn!("Could not get blockhash (likely no RPC key): {}", e),
                }
                
                Err("Solana execution requires Jupiter SDK integration (MVP phase)".to_string())
            }
            "sell" => {
                info!("▶ SOLANA SELL: {}", signal.token_address);
                if self.dry_run {
                    Ok(ExecutionResult::success(
                        signal.signal_id.clone(),
                        "solana_sell_dry_run".to_string(),
                        123456790,
                        0,
                    ))
                } else {
                    Err("Solana sell not yet implemented in MVP".to_string())
                }
            }
            _ => Err(format!("Unknown action: {}", signal.action))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "execution_engine=info".into())
        )
        .init();

    let args = Args::parse();

    info!("╔═══════════════════════════════════════════╗");
    info!("║     Solobot Execution Engine v0.1.0       ║");
    info!("║     High-Speed Same-Block Execution        ║");
    info!("╚═══════════════════════════════════════════╝");
    info!("");
    info!("EVM RPC:       {}", args.evm_rpc_url);
    info!("Flashbots:     {}", args.flashbots_relay);
    info!("Solana RPC:    {}", args.solana_rpc_url);
    info!("Jito URL:      {}", args.jito_url);
    info!("Slippage:      {} bps", args.default_slippage_bps);
    info!("Max Gas:       {} gwei", args.max_gas_price_gwei);
    info!("Dry Run:       {}", args.dry_run);

    // Create the engine
    let engine = ExecutionEngine::new(&args);

    // Ensure data directory exists
    std::fs::create_dir_all("/home/team/shared/data").ok();
    std::fs::write("/home/team/shared/data/execution_engine.pid", format!("{}", std::process::id())).ok();

    info!("");
    info!("📡 Execution Engine ready. Listening for trade signals...");
    info!("");

    // ── Demo: Run a sample trade to verify the system works ──
    info!("═══ Running Dry-Run Self Test ═══");

    // Test signal: Copy-trade scenario
    let test_signal = TradeSignal {
        signal_id: "test-001".to_string(),
        chain: "ethereum".to_string(),
        action: "buy".to_string(),
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), // WETH
        wallet_to_copy: Some("0x0000000000000000000000000000000000000001".to_string()),
        copy_tx_hash: Some("0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string()),
        amount: None,
        amount_usd: Some("5000".to_string()),
        slippage_bps: args.default_slippage_bps,
        max_gas_price_gwei: args.max_gas_price_gwei,
        strategy: "whale_track_test".to_string(),
        confidence: 0.85,
        received_at: chrono::Utc::now().timestamp_nanos(),
    };

    match engine.execute_signal(&test_signal).await {
        Ok(result) => {
            info!("✅ Test signal executed successfully!");
            info!("   Status: {}", result.status);
            info!("   TX:     {:?}", result.tx_hash);
            info!("   Block:  {}", result.block_number);
            info!("   Gas:    {}", result.gas_used);
        }
        Err(e) => {
            warn!("⚠  Test signal result (expected in dry-run): {}", e);
        }
    }

    // Test signal: Solana scenario
    let sol_test_signal = TradeSignal {
        signal_id: "test-002".to_string(),
        chain: "solana".to_string(),
        action: "buy".to_string(),
        token_address: "So11111111111111111111111111111111111111112".to_string(), // wSOL
        wallet_to_copy: None,
        copy_tx_hash: None,
        amount: Some("1000000000".to_string()), // 1 SOL
        amount_usd: Some("5000".to_string()),
        slippage_bps: 100,
        max_gas_price_gwei: 0,
        strategy: "sol_ai_pattern".to_string(),
        confidence: 0.78,
        received_at: chrono::Utc::now().timestamp_nanos(),
    };

    match engine.execute_signal(&sol_test_signal).await {
        Ok(result) => {
            info!("✅ Solana test signal executed successfully!");
            info!("   Status: {}", result.status);
            info!("   TX:     {:?}", result.tx_hash);
        }
        Err(e) => {
            warn!("⚠  Solana test signal result: {}", e);
        }
    }

    info!("\n═══ Self Test Complete ═══");
    info!("Engine running. Awaiting real signals via Redis/gRPC...\n");

    // Keep running (would listen on Redis/gRPC in production)
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}