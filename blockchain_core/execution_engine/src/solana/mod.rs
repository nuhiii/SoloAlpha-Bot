use crate::types::{ChainConfig, ExecutionResult, GasStrategy, SolanaBundle, TradeSignal};
use anyhow::Result;
use tracing::{info, warn, error, debug};

/// Solana Transaction Builder
/// Constructs and submits transactions to Solana, with Jito bundle support
/// for same-block execution.
pub struct SolanaTransactionBuilder {
    chain_config: ChainConfig,
    client: reqwest::Client,
}

impl SolanaTransactionBuilder {
    pub fn new(chain_config: ChainConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
            chain_config,
        }
    }

    /// Get current blockhash (required for Solana tx construction)
    pub async fn get_recent_blockhash(&self) -> Result<String> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getRecentBlockhash",
            "params": []
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(blockhash) = json["result"]["value"]["blockhash"].as_str() {
            Ok(blockhash.to_string())
        } else {
            Err(anyhow::anyhow!("Failed to get recent blockhash"))
        }
    }

    /// Get the latest slot number
    pub async fn get_slot(&self) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": []
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(slot) = json["result"].as_u64() {
            Ok(slot)
        } else {
            Err(anyhow::anyhow!("Failed to get slot"))
        }
    }

    /// Get token account balance for a wallet
    pub async fn get_token_balance(&self, token_mint: &str, wallet: &str) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTokenAccountsByOwner",
            "params": [
                wallet,
                {"mint": token_mint},
                {"encoding": "jsonParsed"}
            ]
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(accounts) = json["result"]["value"].as_array() {
            if let Some(account) = accounts.first() {
                if let Some(amount) = account["account"]["data"]["parsed"]["info"]["tokenAmount"]["amount"].as_str() {
                    return Ok(amount.parse::<u64>().unwrap_or(0));
                }
            }
        }
        
        Ok(0)
    }

    /// Build a priority fee instruction for Solana
    fn build_priority_fee_instruction(&self, micro_lamports: u64) -> serde_json::Value {
        serde_json::json!({
            "programId": "ComputeBudget111111111111111111111111111111",
            "data": [
                micro_lamports
            ],
            "keys": []
        })
    }

    /// Build a compute unit limit instruction
    fn build_compute_unit_limit_instruction(&self, units: u32) -> serde_json::Value {
        serde_json::json!({
            "programId": "ComputeBudget111111111111111111111111111111",
            "data": [
                units
            ],
            "keys": []
        })
    }

    /// Build a swap instruction for a Solana DEX (simplified for MVP)
    pub async fn build_swap_instruction(
        &self,
        signal: &TradeSignal,
        wallet: &str,
    ) -> Result<serde_json::Value> {
        // In production, this would:
        // 1. Query Jupiter quote API for swap route
        // 2. Construct the DEX program instruction with proper accounts
        // 3. Return the serialized instruction
        
        // For MVP, we return a placeholder - the real implementation
        // would use @solana/web3.js or jupiter SDK
        
        info!("Building Solana swap instruction for {} (not implemented - requires Jupiter SDK)", signal.signal_id);
        
        Ok(serde_json::json!({
            "type": "swap",
            "owner": wallet,
            "token_in": signal.token_address,
            "amount": signal.amount,
            "slippage": signal.slippage_bps,
        }))
    }

    /// Construct and submit a Jito bundle for same-block execution
    pub async fn submit_jito_bundle(&self, bundle: &SolanaBundle) -> Result<ExecutionResult> {
        info!("Submitting Jito bundle: {} transactions", bundle.transactions.len());
        
        let jito_url = self.chain_config.jito_url.as_ref()
            .map(|u| format!("{}/api/v1/bundles", u))
            .unwrap_or_else(|| "https://mainnet.block-engine.jito.wtf/api/v1/bundles".to_string());
        
        let bundle_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendBundle",
            "params": [bundle.transactions]
        });
        
        let resp = self.client
            .post(&jito_url)
            .json(&bundle_req)
            .send()
            .await?;
        
        let json: serde_json::Value = resp.json().await?;
        
        if let Some(bundle_id) = json["result"].as_str() {
            info!("✅ Jito bundle submitted. Bundle ID: {}", bundle_id);
            Ok(ExecutionResult::success(
                "jito_bundle".to_string(),
                bundle_id.to_string(),
                0,
                0,
            ))
        } else {
            let error = json["error"].to_string();
            error!("❌ Jito bundle submission failed: {}", error);
            Ok(ExecutionResult::failed("jito_bundle".to_string(), error))
        }
    }

    /// Submit a single transaction directly to Solana
    pub async fn submit_transaction(&self, tx_data: &str) -> Result<ExecutionResult> {
        info!("Submitting Solana transaction directly");
        
        let tx_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                tx_data,
                {
                    "encoding": "base64",
                    "skipPreflight": true,
                    "preflightCommitment": "processed",
                    "maxRetries": 3
                }
            ]
        });
        
        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&tx_req)
            .send()
            .await?;
        
        let json: serde_json::Value = resp.json().await?;
        
        if let Some(sig) = json["result"].as_str() {
            info!("✅ Transaction submitted. Signature: {}", sig);
            Ok(ExecutionResult::success(
                "solana_tx".to_string(),
                sig.to_string(),
                0,
                0,
            ))
        } else {
            let error = json["error"].to_string();
            error!("❌ Transaction submission failed: {}", error);
            Ok(ExecutionResult::failed("solana_tx".to_string(), error))
        }
    }

    /// Wait for transaction confirmation on Solana
    pub async fn wait_for_confirmation(&self, signature: &str, max_retries: u32) -> Result<ExecutionResult> {
        info!("Waiting for Solana tx confirmation: {} (max {} retries)", signature, max_retries);
        
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignatureStatuses",
            "params": [[signature]]
        });
        
        for _ in 0..max_retries {
            if let Ok(resp) = self.client.post(&self.chain_config.rpc_url).json(&req).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(statuses) = json["result"]["value"].as_array() {
                        if let Some(status) = statuses.first() {
                            if let Some(confirmation) = status.as_object() {
                                if let Some(slot) = confirmation.get("slot").and_then(|s| s.as_u64()) {
                                    info!("✅ Solana tx confirmed in slot {}", slot);
                                    return Ok(ExecutionResult::success(
                                        signature.to_string(),
                                        signature.to_string(),
                                        slot,
                                        0,
                                    ));
                                }
                                if let Some(err) = confirmation.get("err") {
                                    if !err.is_null() {
                                        error!("❌ Solana tx failed: {:?}", err);
                                        return Ok(ExecutionResult::failed(
                                            signature.to_string(),
                                            format!("Transaction failed: {:?}", err),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        
        Ok(ExecutionResult::failed(
            signature.to_string(),
            "Transaction not confirmed within max retries".to_string(),
        ))
    }
}