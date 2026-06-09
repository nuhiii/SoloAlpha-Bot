import pandas as pd
import numpy as np

class RSIStrategy:
    def __init__(self, period=14, overbought=70, oversold=30):
        self.period = period
        self.overbought = overbought
        self.oversold = oversold

    def calculate_rsi(self, data: pd.DataFrame) -> pd.Series:
        delta = data['close'].diff()
        gain = (delta.where(delta > 0, 0)).rolling(window=self.period).mean()
        loss = (-delta.where(delta < 0, 0)).rolling(window=self.period).mean()

        rs = gain / loss
        rsi = 100 - (100 / (1 + rs))
        return rsi

    def generate_signals(self, data: pd.DataFrame) -> pd.DataFrame:
        data['rsi'] = self.calculate_rsi(data)
        data['signal'] = 0  # 0: hold, 1: buy, -1: sell
        
        # Buy signal: RSI crosses above oversold
        data.loc[(data['rsi'] > self.oversold) & (data['rsi'].shift(1) <= self.oversold), 'signal'] = 1
        
        # Sell signal: RSI crosses below overbought
        data.loc[(data['rsi'] < self.overbought) & (data['rsi'].shift(1) >= self.overbought), 'signal'] = -1
        
        return data
