import pandas as pd
import numpy as np
import redis
import json
import uuid
from datetime import datetime
from strategies.rsi import RSIStrategy
from metrics.calculator import MetricsCalculator
from providers.mock import MockProvider

class IntegratedStrategyEngine:
    def __init__(self, redis_host='localhost', redis_port=6379, confidence_threshold=0.7):
        self.strategies = [RSIStrategy()]
        self.calculator = MetricsCalculator()
        self.provider = MockProvider()
        self.redis_client = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        self.confidence_threshold = confidence_threshold

    def get_wallet_confidence(self, wallet_address):
        transfers = self.provider.get_token_transfers(wallet_address)
        trades = self.calculator.group_by_token(transfers)
        complete_trades = [t for t in trades if t['is_complete']]
        
        win_rate = self.calculator.calculate_win_rate(complete_trades)
        
        confidence_score = 0.0
        if len(complete_trades) >= 10:
            if win_rate > 60:
                confidence_score = (win_rate / 100.0) * 0.8 + 0.2
            else:
                confidence_score = (win_rate / 100.0) * 0.5
        return confidence_score

    def publish_signal(self, chain, action, token_address, wallet_address, confidence):
        signal = {
            "type": "trade_signal",
            "version": 1,
            "data": {
                "signal_id": str(uuid.uuid4()),
                "chain": chain,
                "action": action.lower(),
                "token_address": token_address,
                "wallet_to_copy": wallet_address,
                "amount_usd": "5000", # Configurable
                "confidence": round(confidence, 2)
            },
            "timestamp": datetime.now().isoformat()
        }
        
        try:
            self.redis_client.publish('solobot:signals', json.dumps(signal))
            print(f"Published signal to Redis: {action} {token_address}")
        except Exception as e:
            print(f"Failed to publish signal: {e}")

    def run(self, data: pd.DataFrame, wallet_address, token_address, chain='ethereum'):
        # 1. Check wallet confidence first
        confidence = self.get_wallet_confidence(wallet_address)
        print(f"Wallet confidence for {wallet_address}: {confidence:.2f}")
        
        if confidence < self.confidence_threshold:
            print(f"Confidence {confidence:.2f} below threshold {self.confidence_threshold}. Skipping.")
            return

        # 2. Run strategies
        for strategy in self.strategies:
            signal_data = strategy.generate_signals(data.copy())
            last_signal = signal_data.iloc[-1]
            print(f"RSI: {signal_data['rsi'].iloc[-1]:.2f}, Signal: {last_signal['signal']}")
            
            if last_signal['signal'] != 0:
                action = 'BUY' if last_signal['signal'] == 1 else 'SELL'
                print(f"Strategy {strategy.__class__.__name__} triggered {action}")
                
                # 3. Publish to Redis
                self.publish_signal(chain, action, token_address, wallet_address, confidence)

if __name__ == "__main__":
    # Mock data
    dates = pd.date_range('2023-01-01', periods=100)
    # Price crosses 30 from below
    # Let's create a "V" shape
    close_prices = list(np.linspace(100, 10, 50)) + list(np.linspace(10, 100, 50))
    data = pd.DataFrame({'close': close_prices}, index=dates)
    
    # Wallet that will have high confidence in our mock
    # Need to modify MockProvider or Metrics to ensure > 0.7 confidence
    # Currently MockProvider gives 20% loss (0 win rate)
    
    engine = IntegratedStrategyEngine(confidence_threshold=0.0) # Lower threshold for testing
    engine.run(data, "0xSmartWallet", "0xTargetToken")
