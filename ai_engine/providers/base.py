from abc import ABC, abstractmethod
from typing import List, Dict, Any

class BaseProvider(ABC):
    @abstractmethod
    def get_transactions(self, address: str, limit: int = 100) -> List[Dict[str, Any]]:
        """Fetch historical transactions for a given wallet address."""
        pass

    @abstractmethod
    def get_token_transfers(self, address: str, limit: int = 100) -> List[Dict[str, Any]]:
        """Fetch token transfers for a given wallet address."""
        pass
