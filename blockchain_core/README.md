# Solobot Blockchain Core

High-performance blockchain monitoring and execution engine for SoloAlpha Bot.

## 📁 Structure

```
src/
├── wallet_tracker/          # Rust wallet tracking service
│   ├── Cargo.toml
│   └── src/main.rs          # Mempool monitor for EVM + Solana
│
└── execution_engine/        # Rust execution engine
    ├── Cargo.toml
    └── src/
        ├── main.rs          # Engine orchestrator
        ├── types.rs         # Shared types (TradeSignal, ExecutionResult, etc.)
        ├── evm/mod.rs       # EVM Flashbots transaction builder
        └── solana/mod.rs    # Solana Jito transaction builder

specs/
└── blockchain_core_spec.md  # Architecture design specification

data/                        # Runtime data (events, logs)
```

## 🚀 Wallet Tracker

Monitors blockchain mempools for activity from tracked "smart money" wallets.

**Features:**
- EVM chain monitoring via WebSocket + HTTP polling
- Solana monitoring via HTTP polling
- Transaction calldata parsing (Uniswap V2/V3 signature detection)
- JSONL logging of all wallet events

**Run:**
```bash
cd src/wallet_tracker
EVM_WS_URL="wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY" \
EVM_HTTP_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY" \
TRACKED_WALLETS="0xwallet1,0xwallet2" \
cargo run --release
```

## ⚡ Execution Engine

Executes trades with same-block precision using Flashbots (EVM) and Jito (Solana).

**Features:**
- Copy-trading: mirror a target wallet's transaction in the same block
- Flashbots bundle submission for EVM chains
- Jito bundle submission for Solana
- Gas optimization (EIP-1559, priority fee strategies)
- Transaction confirmation watcher
- Dry-run mode for testing

**Run:**
```bash
cd src/execution_engine
EVM_RPC_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY" \
FLASHBOTS_RELAY="https://relay.flashbots.net" \
SOLANA_RPC_URL="https://api.mainnet-beta.solana.com" \
cargo run --release
```

## 🔌 Signal Interface (AI Engine ↔ Blockchain Core)

| Channel | Direction | Format |
|---------|-----------|--------|
| `solobot:signals` (Redis) | AI → Engine | JSON TradeSignal |
| `solobot:results` (Redis) | Engine → AI | JSON ExecutionResult |

See the [spec](specs/blockchain_core_spec.md) for full protobuf schema and field definitions.

## 🧪 Test (Python)

```bash
python3 /home/team/shared/src/test_signal.py
```

## 🔑 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `EVM_WS_URL` | wss://eth-mainnet.g.alchemy.com/v2/demo | EVM WebSocket endpoint |
| `EVM_HTTP_URL` | https://eth-mainnet.g.alchemy.com/v2/demo | EVM HTTP endpoint |
| `EVM_RPC_URL` | https://eth-mainnet.g.alchemy.com/v2/demo | EVM RPC (execution) |
| `SOLANA_WS_URL` | wss://api.mainnet-beta.solana.com | Solana WebSocket endpoint |
| `SOLANA_RPC_URL` | https://api.mainnet-beta.solana.com | Solana RPC endpoint |
| `FLASHBOTS_RELAY` | https://relay.flashbots.net | Flashbots relay URL |
| `JITO_URL` | https://mainnet.block-engine.jito.wtf | Jito block engine URL |
| `TRACKED_WALLETS` | "" | Comma-separated wallet addresses |