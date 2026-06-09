from typing import List, Dict, Any
from decimal import Decimal

class MetricsCalculator:
    @staticmethod
    def calculate_win_rate(trades: List[Dict[str, Any]]) -> float:
        if not trades:
            return 0.0
        wins = sum(1 for trade in trades if trade.get('profit', 0) > 0)
        return (wins / len(trades)) * 100

    @staticmethod
    def calculate_pnl(trades: List[Dict[str, Any]]) -> Decimal:
        return sum(Decimal(str(trade.get('profit', 0))) for trade in trades)

    @staticmethod
    def calculate_drawdown(pnl_history: List[Decimal]) -> Decimal:
        if not pnl_history:
            return Decimal('0')
        
        peak = Decimal('-inf')
        max_drawdown = Decimal('0')
        current_pnl = Decimal('0')
        
        for pnl in pnl_history:
            current_pnl += pnl
            if current_pnl > peak:
                peak = current_pnl
            
            drawdown = peak - current_pnl
            if drawdown > max_drawdown:
                max_drawdown = drawdown
                
        return max_drawdown

    @staticmethod
    def group_by_token(transfers: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        # Simplified grouping logic: group buys and sells by token address
        # In a real scenario, this would be much more complex (handling FIFO/LIFO, partial sells, etc.)
        trades = {}
        for transfer in transfers:
            token = transfer.get('tokenAddress')
            if not token:
                continue
            
            if token not in trades:
                trades[token] = {'buys': [], 'sells': []}
            
            # Assuming 'value' is positive for buys and negative for sells for simplicity in this mock-up
            # In real data, we'd check 'from' and 'to' addresses
            if Decimal(transfer.get('value', 0)) > 0:
                trades[token]['buys'].append(transfer)
            else:
                trades[token]['sells'].append(transfer)
        
        # Calculate profit for each token trade
        results = []
        for token, data in trades.items():
            buy_val = sum(Decimal(str(b.get('value', 0))) for b in data['buys'])
            sell_val = abs(sum(Decimal(str(s.get('value', 0))) for s in data['sells']))
            results.append({
                'token': token,
                'profit': sell_val - buy_val,
                'is_complete': buy_val > 0 and sell_val > 0
            })
        return results
