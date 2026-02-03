"""
E002: Verified Algorithm Synthesis (v2 - Smarter Search)

Uses iterative deepening and better pruning.
"""

from z3 import *
from dataclasses import dataclass
from typing import Callable, Optional
from enum import Enum, auto
import time
import json
from pathlib import Path


# ============================================================================
# Operations
# ============================================================================

class Op(Enum):
    ADD = auto()
    SUB = auto()
    MUL = auto()
    DUP = auto()
    DROP = auto()
    SWAP = auto()
    ROT = auto()
    OVER = auto()
    
    def __str__(self):
        return self.name.lower()


@dataclass
class Program:
    ops: list[Op]
    
    def __str__(self):
        return " ".join(str(op) for op in self.ops)
    
    def mul_count(self):
        return sum(1 for op in self.ops if op == Op.MUL)


# ============================================================================
# Fast Stack Simulation (integers only, no Z3)
# ============================================================================

def simulate_stack_shape(ops: list[Op], input_count: int) -> Optional[int]:
    """Simulate stack depth changes. Returns final depth or None if invalid."""
    depth = input_count
    
    for op in ops:
        match op:
            case Op.ADD | Op.SUB | Op.MUL:
                if depth < 2:
                    return None
                depth -= 1
            case Op.DUP | Op.OVER:
                if depth < (1 if op == Op.DUP else 2):
                    return None
                depth += 1
            case Op.DROP:
                if depth < 1:
                    return None
                depth -= 1
            case Op.SWAP:
                if depth < 2:
                    return None
            case Op.ROT:
                if depth < 3:
                    return None
    
    return depth


# ============================================================================
# Z3 Symbolic Execution
# ============================================================================

def symbolic_execute(ops: list[Op], inputs: list) -> Optional[list]:
    """Execute ops symbolically on Z3 expressions. Returns final stack or None."""
    stack = list(inputs)
    
    for op in ops:
        match op:
            case Op.ADD:
                if len(stack) < 2:
                    return None
                b, a = stack.pop(), stack.pop()
                stack.append(a + b)
            case Op.SUB:
                if len(stack) < 2:
                    return None
                b, a = stack.pop(), stack.pop()
                stack.append(a - b)
            case Op.MUL:
                if len(stack) < 2:
                    return None
                b, a = stack.pop(), stack.pop()
                stack.append(a * b)
            case Op.DUP:
                if len(stack) < 1:
                    return None
                stack.append(stack[-1])
            case Op.DROP:
                if len(stack) < 1:
                    return None
                stack.pop()
            case Op.SWAP:
                if len(stack) < 2:
                    return None
                stack[-1], stack[-2] = stack[-2], stack[-1]
            case Op.ROT:
                if len(stack) < 3:
                    return None
                a, b, c = stack[-3], stack[-2], stack[-1]
                stack[-3], stack[-2], stack[-1] = b, c, a
            case Op.OVER:
                if len(stack) < 2:
                    return None
                stack.append(stack[-2])
    
    return stack


def verify_equivalence(
    spec_outputs: list,
    candidate_ops: list[Op],
    inputs: list,
    output_count: int
) -> bool:
    """Verify candidate produces same outputs as spec."""
    stack = symbolic_execute(candidate_ops, inputs)
    if stack is None or len(stack) != output_count:
        return False
    
    candidate_outputs = stack[-output_count:]
    
    # Check equivalence with Z3
    solver = Solver()
    solver.set("timeout", 5000)  # 5 second timeout
    
    # If any output differs, that's a counterexample
    constraints = [spec != cand for spec, cand in zip(spec_outputs, candidate_outputs)]
    solver.add(Or(constraints))
    
    result = solver.check()
    return result == unsat  # unsat means no counterexample, so equivalent!


# ============================================================================
# Iterative Deepening Search
# ============================================================================

ALL_OPS = [Op.ADD, Op.SUB, Op.MUL, Op.DUP, Op.DROP, Op.SWAP, Op.ROT, Op.OVER]
CHEAP_OPS = [Op.ADD, Op.SUB, Op.DUP, Op.DROP, Op.SWAP, Op.ROT, Op.OVER]  # No MUL


def synthesize(
    spec_func: Callable,
    input_names: list[str],
    output_count: int,
    max_muls: int,
    max_length: int = 20,
    timeout_sec: float = 60.0,
) -> Optional[Program]:
    """
    Synthesize a program equivalent to spec with at most max_muls multiplications.
    Uses iterative deepening to find shortest program first.
    """
    # Create symbolic inputs
    inputs = [Int(name) for name in input_names]
    input_count = len(inputs)
    
    # Get spec outputs
    spec_outputs = spec_func(*inputs)
    if not isinstance(spec_outputs, (list, tuple)):
        spec_outputs = [spec_outputs]
    spec_outputs = list(spec_outputs)
    
    start_time = time.time()
    total_checked = 0
    total_pruned = 0
    
    def search(ops: list[Op], depth: int, muls_left: int, stack_depth: int) -> Optional[list[Op]]:
        nonlocal total_checked, total_pruned
        
        if time.time() - start_time > timeout_sec:
            return None
        
        # Check if current program is valid
        if stack_depth == output_count:
            total_checked += 1
            if verify_equivalence(spec_outputs, ops, inputs, output_count):
                return ops
        
        # Reached max depth
        if depth == 0:
            return None
        
        # Try each operation
        available_ops = ALL_OPS if muls_left > 0 else CHEAP_OPS
        
        for op in available_ops:
            # Calculate new stack depth
            new_stack = stack_depth
            new_muls = muls_left
            
            match op:
                case Op.ADD | Op.SUB | Op.MUL:
                    if stack_depth < 2:
                        total_pruned += 1
                        continue
                    new_stack = stack_depth - 1
                    if op == Op.MUL:
                        new_muls = muls_left - 1
                case Op.DUP:
                    if stack_depth < 1:
                        total_pruned += 1
                        continue
                    new_stack = stack_depth + 1
                case Op.DROP:
                    if stack_depth < 1:
                        total_pruned += 1
                        continue
                    new_stack = stack_depth - 1
                case Op.SWAP:
                    if stack_depth < 2:
                        total_pruned += 1
                        continue
                case Op.ROT:
                    if stack_depth < 3:
                        total_pruned += 1
                        continue
                case Op.OVER:
                    if stack_depth < 2:
                        total_pruned += 1
                        continue
                    new_stack = stack_depth + 1
            
            # Prune: stack too large
            if new_stack > 10:
                total_pruned += 1
                continue
            
            # Prune: can't possibly reach output_count
            # Min ops needed to reduce stack to output_count
            if new_stack > output_count:
                min_ops_to_reduce = new_stack - output_count
                if depth - 1 < min_ops_to_reduce:
                    total_pruned += 1
                    continue
            
            ops.append(op)
            result = search(ops, depth - 1, new_muls, new_stack)
            if result is not None:
                return result
            ops.pop()
        
        return None
    
    # Iterative deepening
    for length in range(1, max_length + 1):
        if time.time() - start_time > timeout_sec:
            break
        
        print(f"  Trying length {length}...", end=" ", flush=True)
        result = search([], length, max_muls, input_count)
        
        elapsed = time.time() - start_time
        print(f"checked={total_checked}, pruned={total_pruned}, time={elapsed:.1f}s")
        
        if result is not None:
            return Program(result)
    
    return None


# ============================================================================
# Specifications
# ============================================================================

def simple_multiply_spec(a, b):
    """a * b"""
    return [a * b]


def karatsuba_spec(a1, a0, b1, b0):
    """
    (a1*10 + a0) * (b1*10 + b0) = r2*100 + r1*10 + r0
    
    Standard needs 4 muls: a1*b1, a0*b0, a1*b0, a0*b1
    Karatsuba uses 3 muls: m1=a1*b1, m2=a0*b0, m3=(a1+a0)*(b1+b0)
    Then: r2=m1, r0=m2, r1=m3-m1-m2
    """
    r2 = a1 * b1
    r0 = a0 * b0
    r1 = a1 * b0 + a0 * b1
    return [r2, r1, r0]


def sum_of_products(a, b, c, d):
    """a*b + c*d - common subexpression"""
    return [a*b + c*d]


# ============================================================================
# Main
# ============================================================================

def run_experiment(name: str, spec_func, input_names: list[str], output_count: int, max_muls: int, max_length: int, timeout: float):
    print(f"\n{'='*60}")
    print(f"SYNTHESIZING: {name}")
    print(f"{'='*60}")
    print(f"  Inputs: {input_names}")
    print(f"  Outputs: {output_count}")
    print(f"  Max muls: {max_muls}, Max length: {max_length}")
    print()
    
    start = time.time()
    result = synthesize(spec_func, input_names, output_count, max_muls, max_length, timeout)
    elapsed = time.time() - start
    
    if result:
        print(f"\n✅ FOUND: {result}")
        print(f"   Length: {len(result.ops)}")
        print(f"   Muls: {result.mul_count()}")
        print(f"   Time: {elapsed:.2f}s")
        
        # Double-check verification
        inputs = [Int(n) for n in input_names]
        spec_out = spec_func(*inputs)
        if not isinstance(spec_out, list):
            spec_out = [spec_out]
        verified = verify_equivalence(spec_out, result.ops, inputs, output_count)
        print(f"   Verified: {'✅' if verified else '❌'}")
        
        return {"found": True, "program": str(result), "muls": result.mul_count(), "time": elapsed}
    else:
        print(f"\n❌ Not found (timeout={timeout}s)")
        return {"found": False, "time": elapsed}


def main():
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║          E002: VERIFIED ALGORITHM SYNTHESIS               ║")
    print("║              Using Z3 + Iterative Deepening               ║")
    print("╚═══════════════════════════════════════════════════════════╝")
    
    results = {}
    
    # Test 1: Simple multiply (1 mul)
    results["simple_multiply"] = run_experiment(
        "Simple Multiply (a*b)",
        simple_multiply_spec,
        ["a", "b"],
        output_count=1,
        max_muls=1,
        max_length=5,
        timeout=30.0,
    )
    
    # Test 2: Sum of products (2 muls)
    results["sum_of_products"] = run_experiment(
        "Sum of Products (a*b + c*d)",
        sum_of_products,
        ["a", "b", "c", "d"],
        output_count=1,
        max_muls=2,
        max_length=10,
        timeout=60.0,
    )
    
    # Test 3: Karatsuba (3 muls)
    results["karatsuba"] = run_experiment(
        "Karatsuba Multiplication",
        karatsuba_spec,
        ["a1", "a0", "b1", "b0"],
        output_count=3,
        max_muls=3,
        max_length=20,
        timeout=300.0,
    )
    
    # Summary
    print("\n" + "="*60)
    print("SUMMARY")
    print("="*60)
    for name, r in results.items():
        status = "✅" if r["found"] else "❌"
        details = f"({r.get('muls', '?')} muls, {r['time']:.1f}s)" if r["found"] else f"(timeout)"
        print(f"  {status} {name}: {details}")
    
    # Save
    results_dir = Path(__file__).parent / "results"
    results_dir.mkdir(exist_ok=True)
    filename = results_dir / f"synthesis_{time.strftime('%Y%m%d_%H%M%S')}.json"
    with open(filename, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to: {filename}")


if __name__ == "__main__":
    main()
