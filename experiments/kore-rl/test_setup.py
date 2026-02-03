#!/usr/bin/env python3
"""
Quick test of the Kore-RL setup.

Run with:
    python test_setup.py

Requires:
    - Kore runtime running on localhost:8080
    - pip install httpx
"""

import sys
sys.path.insert(0, "training")

from runtime_client import KoreRuntimeClient


def test_basic_execution():
    """Test basic Kore execution"""
    client = KoreRuntimeClient("http://localhost:8080")
    
    # Test 1: Simple arithmetic
    print("Test 1: 3 4 add")
    result = client.execute("3 4 add")
    assert result.success, f"Failed: {result.error}"
    assert result.final_stack == [7], f"Expected [7], got {result.final_stack}"
    print(f"  ✓ Result: {result.final_stack}")
    
    # Test 2: Multiple operations
    print("\nTest 2: 5 3 mul 2 add")
    result = client.execute("5 3 mul 2 add")
    assert result.success, f"Failed: {result.error}"
    assert result.final_stack == [17], f"Expected [17], got {result.final_stack}"
    print(f"  ✓ Result: {result.final_stack}")
    
    # Test 3: Stack manipulation
    print("\nTest 3: 10 dup add")
    result = client.execute("10 dup add")
    assert result.success, f"Failed: {result.error}"
    assert result.final_stack == [20], f"Expected [20], got {result.final_stack}"
    print(f"  ✓ Result: {result.final_stack}")
    
    # Test 4: Conditional
    print("\nTest 4: 5 3 gt [\"yes\"] [\"no\"] if")
    result = client.execute('5 3 gt ["yes"] ["no"] if')
    assert result.success, f"Failed: {result.error}"
    assert result.final_stack == ["yes"], f"Expected ['yes'], got {result.final_stack}"
    print(f"  ✓ Result: {result.final_stack}")
    
    # Test 5: Error handling
    print("\nTest 5: add (should fail - stack underflow)")
    result = client.execute("add")
    assert not result.success, "Should have failed"
    print(f"  ✓ Error caught: {result.error}")
    
    # Test 6: Trace inspection
    print("\nTest 6: Trace for '2 3 mul 4 add'")
    result = client.execute("2 3 mul 4 add")
    print(f"  Steps executed: {result.steps_executed}")
    for entry in result.trace:
        print(f"    {entry.step}: {entry.op:10} {entry.stack_before} → {entry.stack_after}")
    
    print("\n" + "="*50)
    print("All tests passed!")
    print("="*50)


def test_batch_execution():
    """Test batch execution"""
    client = KoreRuntimeClient("http://localhost:8080")
    
    programs = [
        "1 2 add",
        "5 5 mul",
        "10 3 sub",
        "100 10 div",
    ]
    expected = [3, 25, 7, 10]
    
    print("\nBatch execution test:")
    results = client.execute_batch(programs)
    
    for prog, result, exp in zip(programs, results, expected):
        status = "✓" if result.final_stack == [exp] else "✗"
        print(f"  {status} {prog:15} → {result.final_stack} (expected {exp})")
    
    all_correct = all(r.final_stack == [e] for r, e in zip(results, expected))
    assert all_correct, "Some batch tests failed"
    print("Batch tests passed!")


if __name__ == "__main__":
    print("Testing Kore-RL Setup")
    print("="*50)
    
    try:
        test_basic_execution()
        test_batch_execution()
    except Exception as e:
        print(f"\n✗ Test failed: {e}")
        print("\nMake sure the Kore runtime is running:")
        print("  cd docker && docker compose up -d")
        sys.exit(1)
