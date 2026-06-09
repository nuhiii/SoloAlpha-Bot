import unittest
from unittest.mock import MagicMock, patch
import pandas as pd
import numpy as np
import json
from engine import IntegratedStrategyEngine

class TestIntegration(unittest.TestCase):
    def setUp(self):
        # Patch redis.Redis to avoid actual connection
        self.redis_patcher = patch('redis.Redis')
        self.mock_redis = self.redis_patcher.start()
        self.mock_redis_instance = MagicMock()
        self.mock_redis.return_value = self.mock_redis_instance
        
        self.engine = IntegratedStrategyEngine(confidence_threshold=0.7)

    def tearDown(self):
        self.redis_patcher.stop()

    def test_signal_format(self):
        # Mock confidence to be high
        self.engine.get_wallet_confidence = MagicMock(return_value=0.85)
        
        # Mock strategy to trigger a BUY signal
        mock_strategy = MagicMock()
        mock_strategy.generate_signals.return_value = pd.DataFrame({
            'close': [100, 110],
            'rsi': [25, 35],
            'signal': [0, 1]
        })
        self.engine.strategies = [mock_strategy]
        
        # Dummy data
        data = pd.DataFrame({'close': [100, 110]})
        
        self.engine.run(data, "0xTestWallet", "0xTestToken")
        
        # Check if publish was called
        self.assertTrue(self.mock_redis_instance.publish.called)
        
        # Verify signal format
        args, kwargs = self.mock_redis_instance.publish.call_args
        channel = args[0]
        message = json.loads(args[1])
        
        self.assertEqual(channel, 'solobot:signals')
        self.assertEqual(message['type'], 'trade_signal')
        self.assertEqual(message['version'], 1)
        self.assertEqual(message['data']['chain'], 'ethereum')
        self.assertEqual(message['data']['token_address'], '0xTestToken')
        self.assertEqual(message['data']['wallet_to_copy'], '0xTestWallet')
        self.assertEqual(message['data']['confidence'], 0.85)
        self.assertIn('signal_id', message['data'])
        self.assertIn('action', message['data'])
        self.assertIn('amount_usd', message['data'])

    def test_confidence_threshold(self):
        # Mock confidence to be low
        self.engine.get_wallet_confidence = MagicMock(return_value=0.5)
        
        dates = pd.date_range('2023-01-01', periods=20)
        close_prices = [100] * 20
        data = pd.DataFrame({'close': close_prices}, index=dates)
        
        self.engine.run(data, "0xTestWallet", "0xTestToken")
        
        # Check if publish was NOT called
        self.assertFalse(self.mock_redis_instance.publish.called)

if __name__ == '__main__':
    unittest.main()
