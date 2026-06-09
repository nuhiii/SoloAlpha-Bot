use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── AI Engine Signal Format (from ai_intelligence_spec.md) ──

/// Signal as published by the AI engine to Redis channel `solobot:signals`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSignal {
    pub signal_id: String,
    pub timestamp: String,         // ISO8601
    pub token_address: String,
    pub action: String,            // "BUY" | "SELL"
    pub priority: String,          // "HIGH" | "MEDIUM" | "LOW"
    pub source_wallet: String,
    pub analysis: AiAnalysis,
    pub execution_params: AiExecutionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnalysis {
    pub pattern_detected: String,
    pub pattern_confidence: f64,
    pub wallet_win_rate: f64,
    pub wallet_profit_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExecutionParams {
    pub slippage_bps: u64,
    pub gas_multiplier: f64,
}

// ── Internal Engine Signal (used within the blockchain core) ──

/// Normalized trade signal used internally by the execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub signal_id: String,
    pub chain: String,
    pub action: String,            // "buy" | "sell" (lowercase, normalized)
    pub token_address: String,
    pub wallet_to_copy: Option<String>,
    pub copy_tx_hash: Option<String>,
    pub amount: Option<String>,
    pub amount_usd: Option<String>,
    pub slippage_bps: u64,
    pub max_gas_price_gwei: u64,
    pub strategy: String,
    pub confidence: f64,
    pub received_at: i64,          // Unix nanos
}

impl AiSignal {
    /// Convert an AI signal to the internal TradeSignal format
    pub fn to_internal(&self, chain: &str) -> TradeSignal {
        TradeSignal {
            signal_id: self.signal_id.clone(),
            chain: chain.to_string(),
            action: self.action.to_lowercase(),
            token_address: self.token_address.clone(),
            wallet_to_copy: Some(self.source_wallet.clone()),
            copy_tx_hash: None,
            amount: None,
            amount_usd: None,
            slippage_bps: self.execution_params.slippage_bps,
            max_gas_price_gwei: (100.0 * self.execution_params.gas_multiplier) as u64,
            strategy: format!("ai_{}", self.analysis.pattern_detected.to_lowercase().replace(' ', "_")),
            confidence: self.analysis.pattern_confidence.min(self.analysis.wallet_win_rate),
            received_at: chrono::DateTime::parse_from_rfc3339(&self.timestamp)
                .map(|dt| dt.timestamp_nanos())
                .unwrap_or_else(|_| chrono::Utc::now().timestamp_nanos()),
        }
    }
}

/// Result of executing a trade signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub signal_id: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
    pub block_number: u64,
    pub gas_used: u64,
    pub amount_executed: Option<String>,
    pub price_impact_bps: f64,
    pub executed_at: i64,
}

/// Parsed DEX swap parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapParams {
    pub router: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out_min: String,
    pub deadline: u64,
    pub to: String,
}

/// Execution configuration per chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain: String,
    pub chain_id: u64,
    pub rpc_url: String,
    pub ws_url: String,
    pub flashbots_relay: Option<String>,       // EVM only
    pub jito_url: Option<String>,              // Solana only
    pub private_mempool_url: Option<String>,
}

/// Gas optimization strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GasStrategy {
    /// Match the target wallet's gas price
    Match,
    /// Use a fixed premium over the target
    Premium(u64),  // premium in gwei
    /// Use aggressive bidding (max price)
    Aggressive,
    /// Use EIP-1559 with priority fee
    EIP1559 {
        max_fee: u64,
        priority_fee: u64,
    },
}

/// EVM Bundle construction parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmBundle {
    pub target_tx_hash: String,
    pub mirror_tx_hex: String,   // RLP-encoded signed tx
    pub flashbots_relay: String,
    pub block_number: u64,
    pub min_timestamp: u64,
    pub max_timestamp: u64,
}

/// Solana Bundle construction parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaBundle {
    pub transactions: Vec<String>,
    pub jito_url: String,
    pub tip_amount: u64,  // In lamports
}

impl TradeSignal {
    /// Validate the trade signal has all required fields
    pub fn validate(&self) -> Result<(), String> {
        if self.signal_id.is_empty() {
            return Err("signal_id is required".to_string());
        }
        if self.chain.is_empty() {
            return Err("chain is required".to_string());
        }
        if self.action != "buy" && self.action != "sell" {
            return Err(format!("action must be 'buy' or 'sell', got '{}'", self.action));
        }
        if self.token_address.is_empty() {
            return Err("token_address is required".to_string());
        }
        if self.slippage_bps > 1000 {
            return Err("slippage_bps exceeds maximum (1000 = 10%)".to_string());
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err("confidence must be between 0.0 and 1.0".to_string());
        }
        Ok(())
    }

    /// Check if this is a copy-trade signal
    pub fn is_copy_trade(&self) -> bool {
        self.wallet_to_copy.is_some() || self.copy_tx_hash.is_some()
    }
}

impl ExecutionResult {
    pub fn success(signal_id: String, tx_hash: String, block_number: u64, gas_used: u64) -> Self {
        Self {
            signal_id,
            status: "success".to_string(),
            tx_hash: Some(tx_hash),
            error: None,
            block_number,
            gas_used,
            amount_executed: None,
            price_impact_bps: 0.0,
            executed_at: chrono::Utc::now().timestamp_nanos(),
        }
    }

    pub fn failed(signal_id: String, error: String) -> Self {
        Self {
            signal_id,
            status: "failed".to_string(),
            tx_hash: None,
            error: Some(error),
            block_number: 0,
            gas_used: 0,
            amount_executed: None,
            price_impact_bps: 0.0,
            executed_at: chrono::Utc::now().timestamp_nanos(),
        }
    }
}

/// Known DEX router configurations
pub fn get_dex_router_abis() -> HashMap<String, Vec<String>> {
    let mut routers: HashMap<String, Vec<String>> = HashMap::new();
    
    routers.insert(
        "uniswap_v2".to_string(),
        vec![
            "0x38ed1739".to_string(), // swapExactTokensForTokens
            "0x7ff36ab5".to_string(), // swapExactETHForTokens
            "0x18cbafe5".to_string(), // swapExactTokensForETH
        ],
    );
    
    routers.insert(
        "uniswap_v3".to_string(),
        vec![
            "0x414bf389".to_string(), // exactInputSingle
            "0xc04b8d59".to_string(), // exactInput
        ],
    );
    
    routers.insert(
        "sushiswap".to_string(),
        vec![
            "0x38ed1739".to_string(), // Same sig as Uniswap V2
            "0x18cbafe5".to_string(),
        ],
    );
    
    routers
}
