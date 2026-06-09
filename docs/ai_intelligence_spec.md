# AI Intelligence Layer Design Spec

## 1. Technical Analysis: Chart Pattern Recognition
- **Methodology:** Use a Deep Learning approach for robust pattern detection.
- **Model Architecture:** Vision Transformer (ViT) or ResNet-50 trained on candlestick chart images.
- **Data Pipeline:**
    - Fetch OHLCV data for target tokens.
    - Render data into fixed-size images (e.g., 224x224) using `mplfinance` or a custom fast renderer.
    - Normalize and augment data.
- **Patterns Targeted:** Bullish/Bearish Engulfing, Head and Shoulders, Double Bottom/Top, Cup and Handle, and Trendline Breakouts.
- **Output:** Probability score for each pattern.

## 2. Wallet Win-Rate Verification Logic
- **Process:**
    1. **Data Ingestion:** Retrieve last 100-500 transactions for a given wallet address.
    2. **Trade Grouping:** Group transactions by token to identify complete "trades" (buy to sell).
    3. **Metrics Calculation:**
        - **Win Rate:** `(Winning Trades / Total Trades) * 100`
        - **ROI per Trade:** `((Sell Value - Buy Value) / Buy Value) * 100`
        - **Average Hold Time:** Duration between first buy and last sell.
        - **Profit Factor:** `Sum of Gross Profits / Sum of Gross Losses`
        - **Consistency Score:** Standard deviation of returns.
    4. **Filtering:** Exclude wallets with high volume but low profit (wash trading), or those with only one lucky moonshot.
- **Verification Thresholds:** Win Rate > 65%, Profit Factor > 2.0, Minimum 20 unique trades.

## 3. Decision Engine & Signal Format
- **Logic:** Signal is generated only when:
    - `Wallet Confidence > 0.8` AND `Pattern Confidence > 0.7`.
- **Signal Format (JSON):**
```json
{
  "signal_id": "uuid-v4",
  "timestamp": "ISO8601",
  "token_address": "string",
  "action": "BUY | SELL",
  "priority": "HIGH | MEDIUM | LOW",
  "source_wallet": "string",
  "analysis": {
    "pattern_detected": "string",
    "pattern_confidence": 0.0-1.0,
    "wallet_win_rate": 0.0-1.0,
    "wallet_profit_factor": 0.0-N
  },
  "execution_params": {
    "slippage_bps": 50,
    "gas_multiplier": 1.2
  }
}
```

## 4. Technology Stack
- **Language:** Python 3.10+
- **ML Framework:** PyTorch
- **Data Handling:** Pandas, NumPy
- **Blockchain Interface:** Web3.py / Solana-py
- **Visuals:** mplfinance (for training data generation)
