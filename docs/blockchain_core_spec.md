# Solobot — Blockchain Core Architecture Specification

> **Version:** 1.0  
> **Owner:** Blockchain Engineer  
> **Status:** Draft  

---

## 1. Overview

This document defines the architecture for the blockchain monitoring and execution core of SoloAlpha Bot. The system is designed to:

1. **Monitor** targeted "smart money" wallets across EVM (Ethereum, Arbitrum, Base, etc.) and Solana.
2. **Receive** AI-vetted trade signals from the AI engine (chart pattern analysis, strategy triggers).
3. **Execute** mirror trades with **same-block precision** using proprietary low-latency RPCs.
4. **Optimize** for gas efficiency, front-run resistance, and zero-latency execution.

---

## 2. High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     AI Engine                            │
│  (Pattern Recognition / Strategy Signals)                │
│         │                                                │
│         │  Signal Interface (gRPC / Redis PubSub)        │
│         ▼                                                │
├─────────────────────────────────────────────────────────┤
│               BLOCKCHAIN CORE (This Spec)                │
│                                                         │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Mempool  │  │ Smart Wallet │  │ Execution Engine │   │
│  │ Monitor  │──│ Tracker      │─▶│ (Same-Block)     │   │
│  └──────────┘  └──────────────┘  └──────────────────┘   │
│       │                                                  │
│       ▼                                                  │
│  ┌──────────┐  ┌──────────────┐                          │
│  │ RPC      │  │ Wallet       │                          │
│  │ Manager  │  │ Manager      │                          │
│  └──────────┘  └──────────────┘                          │
└─────────────────────────────────────────────────────────┘
         │
         ▼
  Proprietary / Flashbots / Priority RPCs → On-chain
```

---

## 3. Core Components

### 3.1 Mempool Monitor

**Purpose:** Continuously scan pending and new-block transactions for activity from tracked wallets.

**Capabilities:**
- Subscribe to pending transaction streams via WebSocket (EVM `eth_subscribe` / Solana `logsSubscribe`)
- Filter by tracked wallet addresses
- Parse transaction calldata for token swaps, approvals, transfers
- Detect new liquidity pools, token mints, and DEX interactions
- **Latency target:** < 50ms from mempool detection to signal emission

**Data emitted per detected event:**

```json
{
  "chain": "ethereum" | "base" | "arbitrum" | "solana",
  "tx_hash": "0x...",
  "wallet": "0x...",
  "block_number": 12345678,
  "action": "swap" | "transfer" | "lp_add" | "mint",
  "token_in": "0x...",
  "token_out": "0x...",
  "amount_in": "1000000000000000000",
  "amount_out_min": "...",
  "router": "0x...",
  "gas_price": "50000000000",
  "priority_fee": "1000000000"
}
```

---

### 3.2 Smart Wallet Tracker

**Purpose:** Maintain a curated list of tracked wallets with history, trust scores, and performance metrics.

**Data model:**

| Field | Type | Description |
|---|---|---|
| address | string | Wallet address |
| chain | string | Primary chain |
| label | string | Human-readable label |
| first_seen_block | uint64 | When we started tracking |
| total_trades | uint64 | Count of detected trades |
| win_rate | float | Historical success rate (source: AI engine) |
| avg_entry_size | string | Average trade size in USD |
| pnl_7d | string | 7-day profit/loss |
| tags | string[] | Categorization tags (e.g., "whale", "insider", "bot") |
| active | bool | Whether to currently monitor |

**Storage:** SQLite via Turso/team-db (config table for wallet list).

---

### 3.3 AI Signal Receiver — Interface Definition

**Purpose:** Receive AI-vetted trade signals from the AI engine and convert them into executable orders.

**Transport options:**
- **Primary:** Redis PubSub (channel: `solobot:signals`) — lowest latency, in-memory
- **Fallback:** gRPC bidirectional stream (protobuf schema below)
- **Persistent:** JSON messages via team-db `signals` table (for audit log)

**Protobuf Schema (gRPC):**

```protobuf
syntax = "proto3";

package solobot;

message TradeSignal {
  string signal_id = 1;           // UUID
  string chain = 2;               // "ethereum", "solana", "base", "arbitrum"
  string action = 3;              // "buy", "sell"
  string token_address = 4;       // Target token
  string wallet_to_copy = 5;      // Smart money wallet to mirror (optional)
  string copy_tx_hash = 6;        // Specific tx to mirror (optional)
  string amount = 7;              // Amount in wei/lamports (optional)
  string amount_usd = 8;          // USD value target
  string slippage_bps = 9;        // Slippage tolerance in basis points
  int64 max_gas_price_gwei = 10;  // Max gas price in gwei
  string strategy = 11;           // Strategy name (for logging)
  double confidence = 12;         // AI confidence score 0.0–1.0
  int64 received_at = 13;         // Unix timestamp nanos
}

message ExecutionResult {
  string signal_id = 1;
  string status = 2;              // "success", "failed", "partial"
  string tx_hash = 3;
  string error = 4;
  uint64 block_number = 5;
  uint64 gas_used = 6;
  string amount_executed = 7;
  string price_impact_bps = 8;
  int64 executed_at = 9;
}
```

**Redis message format (JSON, for PubSub):**

```json
{
  "type": "trade_signal",
  "version": 1,
  "data": {
    "signal_id": "uuid",
    "chain": "ethereum",
    "action": "buy",
    "token_address": "0x...",
    "wallet_to_copy": "0x...",
    "copy_tx_hash": null,
    "amount_usd": "5000",
    "slippage_bps": 50,
    "max_gas_price_gwei": 100,
    "strategy": "whale_track_01",
    "confidence": 0.85
  },
  "timestamp": 1717000000123456789
}
```

---

### 3.4 Execution Engine (Same-Block)

**Purpose:** Execute trades in the **same block** as the target wallet's transaction, or as the next block if same-block isn't feasible.

#### 3.4.1 EVM (Ethereum, Arbitrum, Base)

**Strategy:** Flashbots / MEV-Share Bundle Submission

1. **Detect** target wallet transaction in mempool or pending block
2. **Decode** the transaction to extract:
   - DEX router address
   - Token path (tokenIn → tokenOut)
   - Amount parameters
   - Calldata signature (swapExactTokensForTokens, etc.)
3. **Construct mirror transaction:**
   - Same token path
   - Mirrored amount (configurable multiplier: 0.1x to 1.0x)
   - Slightly adjusted slippage to land competitively
4. **Bundle submission:**
   - Place mirror tx AFTER target tx in the same block
   - Submit via Flashbots relay or proprietary private mempool RPC
   - Use `eth_sendBundle` for Flashbots
   - Gas: `maxPriorityFeePerGas` = target_gas + 10% bump
5. **Fallback:** If bundle rejected, submit as standalone tx with aggressive gas for next-block inclusion

**Gas optimization:**
- Pre-compute gas estimates for common DEX routes
- Maintain warm EOA nonces
- Cache calldata for repeat patterns
- Use `type 2` (EIP-1559) transactions

#### 3.4.2 Solana

**Strategy:** Priority Fee Auction + Jito Bundles

1. **Detect** via `logsSubscribe` or geyser plugin for tracked wallet
2. **Decode** instruction data from known DEX programs (Raydium, Jupiter, Orca)
3. **Construct mirror instruction** using same DEX program + token accounts
4. **Submit bundle** via Jito Block Engine or private RPC:
   - Place txs in dependency order
   - Use `computeBudget.setComputeUnitPrice` for priority
   - Use `computeBudget.setComputeUnitLimit` for gas budgeting
5. **Latency target:** <200ms from detection to submission

---

### 3.5 RPC Manager

**Purpose:** Manage connections to multiple RPC endpoints for redundancy and speed.

**RPC tiers:**

| Tier | Use | Examples | SLA |
|---|---|---|---|
| Tier 1 (Private) | Transaction submission | Custom Flashbots relay, Jito, private mempool RPC | < 100ms |
| Tier 2 (Primary) | Block data, streaming | Alchemy, QuickNode, Helius, Triton | < 500ms |
| Tier 3 (Fallback) | Read-only queries | Public RPCs, Infura | best-effort |

**Connection pool:**
- Maintain persistent WebSocket connections for each chain
- Round-robin health checks every 30s
- Auto-failover to next available endpoint on timeout/disconnect
- Rate-limit aware: track requests per second per endpoint

---

### 3.6 Wallet Manager

**Purpose:** Manage the bot's own trading wallets securely.

**Features:**
- Multiple wallets for parallel execution (avoid nonce conflicts)
- Nonce management per chain
- Encrypted key storage (AWS KMS / local encrypted keystore)
- Balance tracking and auto-refill for gas
- **Hot wallet:** For execution (low latency)
- **Cold wallet:** For capital storage (periodic top-ups)

---

## 4. Same-Block Execution Flow (Detailed)

```
[AI Engine] ──Signal──▶ [Signal Receiver]
                             │
                             ▼
                    [Mempool Monitor] ── Detects target tx
                             │
                             ▼
                    [Tx Decoder] ── Extracts trade params
                             │
                             ▼
                    [Order Builder] ── Constructs mirror tx
                             │
                             ▼
                    [Bundle Constructor]
                      ├─ EVM: Flashbots bundle
                      └─ Sol: Jito bundle
                             │
                             ▼
                    [Submission Layer] ── Via Tier 1 RPCs
                             │
                             ▼
                    [Confirmation Watcher]
                      ├─ Success → Log + update P&L
                      └─ Failure → Retry logic (max 3 blocks)
                             │
                             ▼
                    [Result Publisher] ── To AI Engine & Dashboard
```

---

## 5. Required Libraries & SDKs

| Chain | Language | Library | Purpose |
|---|---|---|---|
| EVM | TypeScript | `ethers.js` v6 | Core EVM interaction |
| EVM | TypeScript | `viem` | Low-level EVM (faster for calldata) |
| EVM | TypeScript | `@flashbots/ethers-provider-bundle` | Flashbots bundle submission |
| EVM | Rust | `ethers-rs` | High-perf EVM (optional perf layer) |
| Solana | TypeScript | `@solana/web3.js` | Core Solana interaction |
| Solana | TypeScript | `@solana/spl-token` | Token operations |
| Solana | Rust | `solana-sdk` | High-perf Solana (optional perf layer) |
| Solana | TypeScript | `jito-ts` | Jito bundle submission |
| Shared | TypeScript | `ioredis` | Redis PubSub for signals |
| Shared | TypeScript | `better-sqlite3` | Local SQLite for config/audit |
| Shared | Rust | `tokio` | Async runtime (Rust components) |
| Shared | Rust | `tonic` | gRPC (Rust components) |
| Shared | TypeScript | `@grpc/grpc-js` | gRPC (TypeScript components) |

---

## 6. Performance Requirements

| Metric | Target | Measurement |
|---|---|---|
| MemPool detection → Execution | < 200ms (same-block) | End-to-end timing |
| Signal → Submission (AI path) | < 100ms | Redis PubSub latency |
| Same-block hit rate | > 80% | % of trades landing in target block |
| Transaction success rate | > 95% | % of submitted txs confirmed |
| RPC uptime | > 99.9% | Per-chain, all tiers |
| Max concurrent chains | 4 (ETH, ARB, BASE, SOL) | Instance capacity |

---

## 7. Security & Risk Mitigation

| Risk | Mitigation |
|---|---|
| MEV / Sandwich attacks | Use private mempool / Flashbots; never public txpool |
| Failed tx (bad slippage) | Conservative slippage defaults; retry with adjusted params |
| Wallet compromise | Encrypted keystore; separate hot/cold wallets; daily withdrawal limits |
| RPC rate limiting | Distributed key rotation; multiple provider fallbacks |
| Reorgs | Wait for `safe` block confirmation before logging P&L |
| Smart wallet tracked is wrong | Confidence threshold in AI interface; manual blacklist capability |

---

## 8. Configuration Structure (config table schema)

```json
{
  "chains": {
    "ethereum": {
      "rpc_tier1": "https://...",
      "rpc_tier2": "wss://...",
      "flashbots_relay": "https://relay.flashbots.net",
      "chain_id": 1
    },
    "solana": {
      "rpc_primary": "https://...",
      "jito_url": "https://mainnet.block-engine.jito.wtf",
      "jito_auth_keypair": "..."
    }
  },
  "execution": {
    "default_slippage_bps": 50,
    "max_gas_gwei": 200,
    "bundle_timeout_ms": 1000,
    "max_retry_blocks": 3,
    "default_copy_multiplier": 0.5
  },
  "wallets": {
    "hot_wallets": ["...encrypted..."],
    "cold_wallet": "...encrypted..."
  },
  "tracking": {
    "smart_wallets": ["0x...", "0x..."],
    "min_confidence": 0.7
  }
}
```

---

## 9. Future Enhancements (Post-MVP)

- Custom geyser plugin for Solana (zero-latency tx detection)
- Rust-based execution core for sub-ms processing
- Cross-chain arbitrage detection
- On-chain Jito tip optimization via ML
- Auto-Multi-sig for large trades

---

## 10. Interface Contract with AI Engine

The AI engine MUST publish signals to Redis channel `solobot:signals` using the JSON format defined in §3.3. The blockchain core will:
1. Subscribe to the channel on startup
2. Acknowledge signals with an `ExecutionResult` on channel `solobot:results`
3. Log all signals and results to the team-db `signals` table for audit

For the MVP, the signal format is the binding contract. gRPC can be introduced in v2 for stricter typing and backpressure handling.

---

*End of Spec*
