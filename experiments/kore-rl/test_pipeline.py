#!/usr/bin/env python3
"""
Test the full Kore-RL pipeline (without model).

This tests:
1. Task generation
2. Executor (local binary)
3. Reward computation
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "training"))

from docker_executor import KoreLocalExecutor, ExecuteResult
from trainer import (
    generate_arithmetic_tasks,
    generate_stack_tasks,
    generate_control_tasks,
    generate_loop_tasks,
    extract_program,
)

def test_executor():
    """Test the local executor"""
    print("=" * 60)
    print("Testing Local Executor")
    print("=" * 60)
    
    exe = KoreLocalExecutor(
        kore_binary="./target/release/kore-train",
        max_workers=8
    )
    
    # Basic tests
    tests = [
        ("3 4 add", [7]),
        ("10 3 sub", [7]),
        ("6 7 mul", [42]),
        ("5 dup mul", [25]),
        ("1 2 3 swap", [1, 3, 2]),
        ("[ 1 2 3 ] len", ['[quote:3]']),  # List literal
    ]
    
    print("\nBasic tests:")
    for program, expected in tests:
        result = exe.execute(program, trace=False)
        status = "✓" if result.success and result.final_stack == expected else "✗"
        print(f"  {status} {program!r:30} -> {result.final_stack} (expected {expected})")
    
    # Batch test
    import time
    programs = [f"{i} {i+1} add" for i in range(500)]
    start = time.time()
    results = exe.execute_batch(programs, trace=False)
    elapsed = time.time() - start
    
    success_count = sum(1 for r in results if r.success)
    print(f"\nBatch execution (500 programs):")
    print(f"  Time: {elapsed:.3f}s")
    print(f"  Rate: {500/elapsed:.0f} programs/sec")
    print(f"  Success: {success_count}/500")
    
    exe.close()
    return True


def test_task_generation():
    """Test curriculum task generation"""
    print("\n" + "=" * 60)
    print("Testing Task Generation")
    print("=" * 60)
    
    arithmetic = generate_arithmetic_tasks(5)
    stack = generate_stack_tasks(5)
    control = generate_control_tasks(5)
    loop = generate_loop_tasks(5)
    
    print(f"\nArithmetic tasks: {len(arithmetic)}")
    for task in arithmetic[:3]:
        print(f"  - {task['prompt'][:60]}...")
        print(f"    Target: {task['target']}, Difficulty: {task['difficulty']}")
    
    print(f"\nStack tasks: {len(stack)}")
    for task in stack[:3]:
        print(f"  - {task['prompt'][:60]}...")
        print(f"    Target: {task['target']}, Difficulty: {task['difficulty']}")
    
    print(f"\nControl tasks: {len(control)}")
    for task in control[:3]:
        print(f"  - {task['prompt'][:60]}...")
        print(f"    Target: {task['target']}, Difficulty: {task['difficulty']}")
    
    print(f"\nLoop tasks: {len(loop)}")
    for task in loop[:3]:
        print(f"  - {task['prompt'][:60]}...")
        print(f"    Target: {task['target']}, Difficulty: {task['difficulty']}")
    
    return True


def test_program_extraction():
    """Test program extraction from model output"""
    print("\n" + "=" * 60)
    print("Testing Program Extraction")
    print("=" * 60)
    
    samples = [
        "3 4 add\n```",
        "3 4 add",
        "3\n4\nadd\n```\n\nThis computes the sum.",
        "# First push numbers\n3 4\n# Then add\nadd",
    ]
    
    print("\nExtraction tests:")
    for sample in samples:
        extracted = extract_program(sample)
        print(f"  Input: {sample[:40]!r}...")
        print(f"  Extracted: {extracted!r}")
        print()
    
    return True


def test_reward_computation():
    """Test reward computation logic"""
    print("\n" + "=" * 60)
    print("Testing Reward Computation")
    print("=" * 60)
    
    exe = KoreLocalExecutor(
        kore_binary="./target/release/kore-train",
        max_workers=4
    )
    
    test_cases = [
        # (program, target, expected_reward_description)
        ("3 4 add", 7, "perfect (1.0)"),
        ("3 4 mul", 7, "wrong answer (partial credit)"),
        ("invalid syntax !@#", 7, "error (-1.0)"),
        ("3 4", 7, "wrong stack shape"),
    ]
    
    print("\nReward cases:")
    for program, target, description in test_cases:
        result = exe.execute(program, trace=False)
        
        if not result.success:
            reward = -1.0 + 0.1 * min(result.steps_executed / 20, 0.5)
        elif len(result.final_stack) == 1 and result.final_stack[0] == target:
            reward = 1.0
        elif len(result.final_stack) == 1:
            got = result.final_stack[0]
            if isinstance(got, (int, float)) and isinstance(target, (int, float)):
                diff = abs(got - target)
                reward = 0.5 / (1 + diff * 0.1)
            else:
                reward = -0.3
        else:
            reward = -0.5
        
        print(f"  {program!r:25} -> {result.final_stack}, reward={reward:.2f} ({description})")
    
    exe.close()
    return True


def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)) + "/../..")  # kore root
    
    all_passed = True
    
    try:
        all_passed &= test_executor()
    except Exception as e:
        print(f"Executor test failed: {e}")
        all_passed = False
    
    try:
        all_passed &= test_task_generation()
    except Exception as e:
        print(f"Task generation test failed: {e}")
        all_passed = False
    
    try:
        all_passed &= test_program_extraction()
    except Exception as e:
        print(f"Program extraction test failed: {e}")
        all_passed = False
    
    try:
        all_passed &= test_reward_computation()
    except Exception as e:
        print(f"Reward computation test failed: {e}")
        all_passed = False
    
    print("\n" + "=" * 60)
    if all_passed:
        print("All pipeline tests passed! ✓")
    else:
        print("Some tests failed! ✗")
    print("=" * 60)
    
    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
