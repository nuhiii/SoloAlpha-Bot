# Deployment & Setup Instructions: SoloAlpha Bot v1.0

This guide provides step-by-step instructions for setting up and running the SoloAlpha Bot in a dry-run environment.

## Prerequisites
- **Rust Toolchain:** Installed (latest stable).
- **Python 3.10+:** Installed.
- **Redis:** Installed and running locally (for PubSub messaging).
- **Node.js (Optional):** For additional monitoring tools if applicable.

## 1. Repository Structure
- `/blockchain_core`: Rust-based execution engine (EVM & Solana).
- `/ai_engine`: Python-based technical analysis and wallet scoring.
- `/docs`: Technical specifications and integration test logs.

## 2. Environment Configuration
Create a `.env` file in both `/blockchain_core` and `/ai_engine` directories using the `.env.example` template provided in the repo.

### Key Variables:
- `REDIS_URL`: `redis://127.0.0.1:6379`
- `EVM_RPC_URL`: Your Alchemy/QuickNode Ethereum/Base RPC.
- `SOLANA_RPC_URL`: Your Solana RPC.
- `FLASHBOTS_RELAY_URL`: (Optional for dry-run) Flashbots relay endpoint.
- `TARGET_WALLETS`: Comma-separated list of addresses to track.

## 3. Setup Instructions

### AI Engine (Python)
1. Navigate to `/ai_engine`.
2. Create a virtual environment: `python3 -m venv venv`.
3. Activate it: `source venv/bin/activate`.
4. Install dependencies: `pip install -r requirements.txt`. (Note: Ensure pandas, numpy, and redis-py are included).
5. Run the engine: `python engine.py`.

### Blockchain Core (Rust)
1. Navigate to `/blockchain_core`.
2. Build the project: `cargo build --release`.
3. Run the service: `cargo run --release`.

## 4. Running Dry-Run Integration Tests
To verify the full pipeline without spending real funds:
1. Ensure Redis is running.
2. Start the AI Engine in one terminal.
3. Start the Blockchain Core in another terminal.
4. Run the integration script (located in `/docs` or the root): `python integration_test.py`.
5. Check `integration_test_results.json` for a 100% pass rate across the 18 verification points.

## 5. Security Note
- **NEVER** share your `.env` file or private keys.
- For live testing, use a burner wallet with minimal balance.
- RPC keys should be entered directly into your local environment.
