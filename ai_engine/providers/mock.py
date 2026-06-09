from typing import List, Dict, Any
from .base import BaseProvider
import time

class MockProvider(BaseProvider):
    def get_transactions(self, address: str, limit: int = 100) -> List[Dict[str, Any]]:
        # Mock data for testing
        return [
            {
                "hash": f"0x{i:064x}",
                "from": address,
                "to": "0xTargetAddress",
                "value": "1000000000000000000",
                "timestamp": int(time.time()) - i * 3600
            } for i in range(limit)
        ]

    def get_token_transfers(self, address: str, limit: int = 100) -> List[Dict[str, Any]]:
        transfers = []
        for i in range(limit):
            is_buy = i % 2 == 0
            transfers.append({
                "hash": f"0x{i:064x}",
                "from": address if not is_buy else "0xDexRouter",
                "to": "0xDexRouter" if not is_buy else address,
                "tokenAddress": f"0xToken{i//10}",  # 10 trades per token
                "tokenSymbol": f"TKN{i//10}",
                "value": str(1000000000000000000 if is_buy else -1200000000000000000), # 20% gain for mock
                "timestamp": int(time.time()) - i * 3600
            })
        return transfers
