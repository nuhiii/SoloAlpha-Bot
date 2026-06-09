use crate::types::{ChainConfig, ExecutionResult, GasStrategy, SwapParams, TradeSignal, EvmBundle};
use anyhow::Result;
use tracing::{info, warn, error, debug};

/// EVM Transaction Builder
/// Constructs and signs transactions for EVM-compatible chains.
/// Supports Flashbots bundle submission for same-block execution.
pub struct EvmTransactionBuilder {
    chain_config: ChainConfig,
    client: reqwest::Client,
}

impl EvmTransactionBuilder {
    pub fn new(chain_config: ChainConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client"),
            chain_config,
        }
    }

    /// Get current gas price from the chain
    pub async fn get_gas_price(&self) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_gasPrice",
            "params": []
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(price) = json["result"].as_str() {
            let price = u64::from_str_radix(price.strip_prefix("0x").unwrap_or("0"), 16)?;
            Ok(price)
        } else {
            Err(anyhow::anyhow!("Failed to get gas price: {:?}", json))
        }
    }

    /// Get current chain ID
    pub async fn get_chain_id(&self) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_chainId",
            "params": []
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(chain_id) = json["result"].as_str() {
            let chain_id = u64::from_str_radix(chain_id.strip_prefix("0x").unwrap_or("0"), 16)?;
            Ok(chain_id)
        } else {
            Err(anyhow::anyhow!("Failed to get chain ID"))
        }
    }

    /// Get the nonce for a wallet
    pub async fn get_nonce(&self, wallet: &str) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionCount",
            "params": [wallet, "pending"]
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(nonce) = json["result"].as_str() {
            let nonce = u64::from_str_radix(nonce.strip_prefix("0x").unwrap_or("0"), 16)?;
            Ok(nonce)
        } else {
            Err(anyhow::anyhow!("Failed to get nonce for {}", wallet))
        }
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, from: &str, to: &str, data: &str) -> Result<u64> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_estimateGas",
            "params": [{
                "from": from,
                "to": to,
                "data": data
            }]
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(gas) = json["result"].as_str() {
            let gas = u64::from_str_radix(gas.strip_prefix("0x").unwrap_or("0"), 16)?;
            Ok(gas)
        } else {
            // Default to a reasonable estimate
            Ok(250_000)
        }
    }

    /// Decode a target transaction to extract swap parameters
    pub async fn decode_target_tx(&self, tx_hash: &str) -> Result<SwapParams> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionByHash",
            "params": [tx_hash]
        });

        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&req)
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        
        if let Some(tx) = json["result"].as_object() {
            let input = tx["input"].as_str().unwrap_or("0x");
            let to = tx["to"].as_str().unwrap_or("0x0").to_string();
            let value = tx["value"].as_str().unwrap_or("0x0").to_string();
            let gas_price = tx["gasPrice"].as_str().unwrap_or("0x0").to_string();
            
            // Parse calldata (simplified for MVP)
            let (token_in, token_out, amount_in, amount_out_min) = self.parse_calldata(input, &to).await;
            
            Ok(SwapParams {
                router: to,
                token_in,
                token_out,
                amount_in,
                amount_out_min,
                deadline: 0,
                to: String::new(),
            })
        } else {
            Err(anyhow::anyhow!("Transaction {} not found", tx_hash))
        }
    }

    /// Parse calldata from a DEX transaction
    async fn parse_calldata(&self, input: &str, router: &str) -> (String, String, String, String) {
        let input = input.strip_prefix("0x").unwrap_or(input);
        
        if input.len() < 8 {
            return ("0x0".into(), "0x0".into(), "0".into(), "0".into());
        }
        
        let sig = &input[..8];
        
        // Uniswap V2 swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] path, address to, uint deadline)
        // Offset: sig(4) + amountIn(32) + amountOutMin(32) + pathOffset(32) + to(32) + deadline(32)
        if sig == "38ed1739" && input.len() >= 140 {
            let amount_in = self.hex_to_u256(&input[8..72]);
            let amount_out_min = self.hex_to_u256(&input[72..136]);
            
            // Extract token addresses from path (simplified)
            let path_offset = self.hex_to_u256(&input[136..168]) as usize;
            if path_offset > 0 && (8 + path_offset + 64 + 64) < input.len() {
                let path_len_start = 8 + path_offset;
                let path_len = self.hex_to_u256(&input[path_len_start..path_len_start + 64]) as usize;
                let first_token_start = path_len_start + 64;
                
                if path_len >= 2 && (first_token_start + 128) <= input.len() {
                    let token_in = format!("0x{}", &input[first_token_start..first_token_start + 40]);
                    let token_out_start = first_token_start + 64;
                    let token_out = format!("0x{}", &input[token_out_start..token_out_start + 40]);
                    
                    return (token_in, token_out, amount_in, amount_out_min);
                }
            }
        }
        
        // Uniswap V3 exactInputSingle
        if sig == "414bf389" && input.len() >= 68 {
            let params_hex = &input[8..];  // Rest is the struct
            if params_hex.len() >= 64 {
                let token_in = format!("0x{}", &params_hex[24..64]); // last 20 bytes of first 32-byte word
                // More complex parsing would be needed for full struct
            }
        }
        
        ("0x0".into(), "0x0".into(), "0".into(), "0".into())
    }

    fn hex_to_u256(&self, hex_str: &str) -> String {
        let hex = hex_str.trim_start_matches('0');
        if hex.is_empty() {
            return "0".to_string();
        }
        u128::from_str_radix(&hex[..hex.len().min(32)], 16)
            .unwrap_or(0)
            .to_string()
    }

    /// Construct a mirror transaction for Flashbots bundle
    pub async fn build_mirror_tx(
        &self,
        target_tx_hash: &str,
        signal: &TradeSignal,
        gas_strategy: GasStrategy,
        wallet_addr: &str,
    ) -> Result<EvmBundle> {
        info!("Building mirror tx for target: {}", target_tx_hash);
        
        // Decode target transaction
        let swap_params = self.decode_target_tx(target_tx_hash).await?;
        
        // Get current gas price
        let base_gas = self.get_gas_price().await?;
        
        // Calculate gas price based on strategy
        let gas_price = match gas_strategy {
            GasStrategy::Match => base_gas,
            GasStrategy::Premium(premium) => base_gas + (premium * 1_000_000_000), // premium in gwei to wei
            GasStrategy::Aggressive => base_gas * 12 / 10, // 20% premium
            GasStrategy::EIP1559 { max_fee, priority_fee } => {
                // For EIP-1559, we set maxFeePerGas and maxPriorityFeePerGas
                // This is a simplified version - in production we'd use proper EIP-1559 fields
                max_fee
            }
        };
        
        let gas_limit = self.estimate_gas(wallet_addr, &swap_params.router, "0x").await?;
        let nonce = self.get_nonce(wallet_addr).await?;
        let chain_id = self.get_chain_id().await?;
        
        // In production, we would:
        // 1. Build the complete transaction object with proper calldata
        // 2. Sign it with the wallet's private key
        // 3. Submit as a Flashbots bundle
        //
        // For the MVP, we construct the bundle metadata
        info!("Mirror tx constructed: router={} gas={} nonce={}", swap_params.router, gas_price, nonce);
        
        Ok(EvmBundle {
            target_tx_hash: target_tx_hash.to_string(),
            mirror_tx_hex: String::new(), // Would be RLP-encoded signed tx
            flashbots_relay: self.chain_config.flashbots_relay.clone().unwrap_or_default(),
            block_number: 0, // Would be current + 1
            min_timestamp: 0,
            max_timestamp: 0,
        })
    }

    /// Submit a Flashbots bundle for same-block execution
    pub async fn submit_flashbots_bundle(&self, bundle: &EvmBundle) -> Result<ExecutionResult> {
        info!("Submitting Flashbots bundle for target tx: {}", bundle.target_tx_hash);
        
        if bundle.flashbots_relay.is_empty() {
            warn!("No Flashbots relay configured. Using public mempool as fallback.");
            return self.submit_public_mempool(bundle).await;
        }
        
        let bundle_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendBundle",
            "params": [{
                "txs": [bundle.mirror_tx_hex],
                "blockNumber": format!("0x{:x}", bundle.block_number),
                "minTimestamp": bundle.min_timestamp,
                "maxTimestamp": bundle.max_timestamp
            }]
        });
        
        let resp = self.client
            .post(&bundle.flashbots_relay)
            .json(&bundle_req)
            .send()
            .await?;
        
        let json: serde_json::Value = resp.json().await?;
        
        if let Some(result) = json["result"].as_str() {
            info!("✅ Bundle submitted successfully. Bundle hash: {}", result);
            Ok(ExecutionResult::success(
                "bundle".to_string(),
                result.to_string(),
                bundle.block_number,
                0, // gas will be known after inclusion
            ))
        } else {
            let error = json["error"].to_string();
            error!("❌ Bundle submission failed: {}", error);
            Ok(ExecutionResult::failed("bundle".to_string(), error))
        }
    }

    /// Fallback: Submit transaction directly to public mempool
    async fn submit_public_mempool(&self, bundle: &EvmBundle) -> Result<ExecutionResult> {
        info!("Submitting to public mempool (no Flashbots relay configured)");
        
        let tx_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendRawTransaction",
            "params": [bundle.mirror_tx_hex]
        });
        
        let resp = self.client
            .post(&self.chain_config.rpc_url)
            .json(&tx_req)
            .send()
            .await?;
        
        let json: serde_json::Value = resp.json().await?;
        
        if let Some(tx_hash) = json["result"].as_str() {
            info!("✅ Transaction submitted to public mempool. Tx: {}", tx_hash);
            Ok(ExecutionResult::success(
                "public".to_string(),
                tx_hash.to_string(),
                0,
                0,
            ))
        } else {
            let error = json["error"].to_string();
            error!("❌ Public mempool submission failed: {}", error);
            Ok(ExecutionResult::failed("public".to_string(), error))
        }
    }

    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(&self, tx_hash: &str, max_blocks: u64) -> Result<ExecutionResult> {
        info!("Waiting for confirmation of tx: {} (max {} blocks)", tx_hash, max_blocks);
        
        for _ in 0..max_blocks {
            let req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "eth_getTransactionReceipt",
                "params": [tx_hash]
            });
            
            if let Ok(resp) = self.client.post(&self.chain_config.rpc_url).json(&req).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(receipt) = json["result"].as_object() {
                        if let Some(block_num) = receipt["blockNumber"].as_str() {
                            let block = u64::from_str_radix(block_num.strip_prefix("0x").unwrap_or("0"), 16)?;
                            let gas_used = if let Some(gas) = receipt["gasUsed"].as_str() {
                                u64::from_str_radix(gas.strip_prefix("0x").unwrap_or("0"), 16)?
                            } else {
                                0
                            };
                            
                            info!("✅ Transaction confirmed in block {}", block);
                            return Ok(ExecutionResult::success(
                                tx_hash.to_string(),
                                tx_hash.to_string(),
                                block,
                                gas_used,
                            ));
                        }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
        
        Ok(ExecutionResult::failed(
            tx_hash.to_string(),
            "Transaction not confirmed within max blocks".to_string(),
        ))
    }
}