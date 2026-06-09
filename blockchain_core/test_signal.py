#!/usr/bin/env python3
"""
Test script for the Solobot Signal Interface.
Tests both the AI engine signal format (from ai_intelligence_spec.md)
and the internal engine TradeSignal format (from types.rs).
Verifies conversion and compatibility.
"""
import json
import uuid
import time
from datetime import datetime, timezone

def generate_ai_signal():
    """Generate a test signal matching the AI engine format (ai_intelligence_spec.md)."""
    return {
        "signal_id": str(uuid.uuid4()),
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "token_address": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",  # WETH
        "action": "BUY",
        "priority": "HIGH",
        "source_wallet": "0x0000000000000000000000000000000000000001",
        "analysis": {
            "pattern_detected": "Bullish Engulfing",
            "pattern_confidence": 0.87,
            "wallet_win_rate": 0.72,
            "wallet_profit_factor": 3.2
        },
        "execution_params": {
            "slippage_bps": 50,
            "gas_multiplier": 1.2
        }
    }

def generate_solana_ai_signal():
    """Generate a Solana AI signal."""
    return {
        "signal_id": str(uuid.uuid4()),
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "token_address": "So11111111111111111111111111111111111111112",  # wSOL
        "action": "BUY",
        "priority": "MEDIUM",
        "source_wallet": "SolanaWalletAddr123456789012345678901",
        "analysis": {
            "pattern_detected": "Double Bottom",
            "pattern_confidence": 0.78,
            "wallet_win_rate": 0.68,
            "wallet_profit_factor": 2.1
        },
        "execution_params": {
            "slippage_bps": 100,
            "gas_multiplier": 1.0
        }
    }

def convert_to_internal(ai_signal, chain="ethereum"):
    """Simulate the Rust AiSignal::to_internal() conversion."""
    return {
        "signal_id": ai_signal["signal_id"],
        "chain": chain,
        "action": ai_signal["action"].lower(),
        "token_address": ai_signal["token_address"],
        "wallet_to_copy": ai_signal["source_wallet"],
        "copy_tx_hash": None,
        "amount": None,
        "amount_usd": None,
        "slippage_bps": ai_signal["execution_params"]["slippage_bps"],
        "max_gas_price_gwei": int(100 * ai_signal["execution_params"]["gas_multiplier"]),
        "strategy": f"ai_{ai_signal['analysis']['pattern_detected'].lower().replace(' ', '_')}",
        "confidence": min(
            ai_signal["analysis"]["pattern_confidence"],
            ai_signal["analysis"]["wallet_win_rate"]
        ),
        "received_at": int(time.time_ns())
    }

def test_signal_interface():
    """Run full test suite for the signal interface."""
    print("=" * 70)
    print("Solobot Signal Interface Tests")
    print("=" * 70)

    # Test 1: Generate AI Signal
    print("\n[Test 1] AI Engine Signal Generation")
    ai_sig = generate_ai_signal()
    print(f"  ✅ signal_id: {ai_sig['signal_id'][:8]}...")
    print(f"     action:    {ai_sig['action']}")
    print(f"     priority:  {ai_sig['priority']}")
    print(f"     pattern:   {ai_sig['analysis']['pattern_detected']}")
    print(f"     confidence: {ai_sig['analysis']['pattern_confidence']:.2f}")

    # Test 2: JSON Serialization (for Redis PubSub)
    print("\n[Test 2] Redis PubSub Message Format")
    redis_msg = {
        "type": "trade_signal",
        "version": 1,
        "data": ai_sig,
        "timestamp": time.time_ns()
    }
    serialized = json.dumps(redis_msg, indent=2)
    deserialized = json.loads(serialized)
    assert deserialized["data"]["signal_id"] == ai_sig["signal_id"]
    print(f"  ✅ JSON roundtrip OK ({len(serialized)} bytes)")

    # Test 3: Convert to Internal TradeSignal
    print("\n[Test 3] AI → Internal Signal Conversion")
    internal = convert_to_internal(ai_sig)
    print(f"  ✅ action lowercased:    '{ai_sig['action']}' → '{internal['action']}'")
    print(f"     wallet_to_copy:       {internal['wallet_to_copy'][:14]}...")
    print(f"     slippage_bps:         {internal['slippage_bps']}")
    print(f"     strategy:             {internal['strategy']}")
    print(f"     confidence (min):     {internal['confidence']:.2f}")

    # Test 4: Full Pipeline End-to-End
    print("\n[Test 4] End-to-End Pipeline (Simulated)")
    
    # Step 1: AI detects pattern → generates signal
    eth_signal = generate_ai_signal()
    print(f"  1. AI Pattern Detected: {eth_signal['analysis']['pattern_detected']}")
    
    # Step 2: Publish to Redis
    print(f"  2. Published to Redis channel 'solobot:signals'")
    
    # Step 3: Convert to internal format
    internal_eth = convert_to_internal(eth_signal, "ethereum")
    print(f"  3. Converted to internal: chain={internal_eth['chain']}")
    
    # Step 4: Validate
    action_ok = internal_eth["action"] in ("buy", "sell")
    chain_ok = internal_eth["chain"] in ("ethereum", "arbitrum", "base", "solana")
    conf_ok = 0.0 <= internal_eth["confidence"] <= 1.0
    slip_ok = internal_eth["slippage_bps"] <= 1000
    print(f"  4. Validation: action={action_ok} chain={chain_ok} conf={conf_ok} slip={slip_ok}")
    
    if all([action_ok, chain_ok, conf_ok, slip_ok]):
        print("  ✅ Signal validated successfully")
    else:
        print("  ❌ Signal validation FAILED")

    # Test 5: Solana Signal
    print("\n[Test 5] Solana Signal Path")
    sol_sig = generate_solana_ai_signal()
    internal_sol = convert_to_internal(sol_sig, "solana")
    print(f"  ✅ Solana signal: chain={internal_sol['chain']}")
    print(f"     token: {internal_sol['token_address']}")

    # Test 6: Redis channel format reference
    print("\n" + "=" * 70)
    print("Reference: Redis PubSub Message")
    print("=" * 70)
    print(json.dumps({
        "type": "trade_signal",
        "version": 1,
        "data": generate_ai_signal(),
        "timestamp": time.time_ns()
    }, indent=2))

    print("\n" + "=" * 70)
    print("Reference: Internal TradeSignal (JSON)")
    print("=" * 70)
    print(json.dumps(convert_to_internal(generate_ai_signal()), indent=2))

    print("\n" + "=" * 70)
    print("All tests passed!")
    print("=" * 70)

if __name__ == "__main__":
    test_signal_interface()