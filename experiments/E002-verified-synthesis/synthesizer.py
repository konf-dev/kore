"""
E002: Verified Algorithm Synthesis

Discovers optimal algorithms by:
1. Enumerating programs with bounded multiplication count
2. Using Z3 to verify equivalence to spec
3. Linear resource tracking (muls can't be reused)

Targets:
- Karatsuba: 3 muls for two-digit multiplication (vs 4 standard)
- Strassen: 7 muls for 2x2 matrix multiplication (vs 8 standard)
"""

from z3 import *
from dataclasses import dataclass
from typing import Callable, Optional
from enum import Enum
import time
import json
from pathlib import Path


# ============================================================================
# Stack Machine for Symbolic Execution
# ============================================================================

class Op(Enum):
    ADD = "add"
    SUB = "sub"
    MUL = "mul"
    DUP = "dup"
    DROP = "drop"
    SWAP = "swap"
    ROT = "rot"
    OVER = "over"
    NIP = "nip"
    PICK = "pick"  # pick n - copy n-th item to top


@dataclass
class Program:
    ops: list[Op | int]  # int for PICK index
    
    def __str__(self):
        return " ".join(str(o.value if isinstance(o, Op) else o) for o in self.ops)


class SymbolicStack:
    """Stack of Z3 expressions for symbolic execution."""
    
    def __init__(self, initial: list):
        self.stack = list(initial)
        self.mul_count = 0
        self.valid = True
    
    def copy(self):
        new = SymbolicStack([])
        new.stack = list(self.stack)
        new.mul_count = self.mul_count
        new.valid = self.valid
        return new
    
    def depth(self):
        return len(self.stack)
    
    def push(self, val):
        self.stack.append(val)
    
    def pop(self):
        if not self.stack:
            self.valid = False
            return IntVal(0)
        return self.stack.pop()
    
    def peek(self, n=0):
        if n >= len(self.stack):
            self.valid = False
            return IntVal(0)
        return self.stack[-(n+1)]
    
    def execute(self, op: Op | int, pick_index: Optional[int] = None) -> bool:
        """Execute single op. Returns False if invalid."""
        if not self.valid:
            return False
            
        if isinstance(op, int):
            # It's a PICK index
            if op >= len(self.stack):
                self.valid = False
                return False
            self.push(self.stack[-(op+1)])
            return True
            
        match op:
            case Op.ADD:
                if self.depth() < 2:
                    self.valid = False
                    return False
                b, a = self.pop(), self.pop()
                self.push(a + b)
                
            case Op.SUB:
                if self.depth() < 2:
                    self.valid = False
                    return False
                b, a = self.pop(), self.pop()
                self.push(a - b)
                
            case Op.MUL:
                if self.depth() < 2:
                    self.valid = False
                    return False
                b, a = self.pop(), self.pop()
                self.push(a * b)
                self.mul_count += 1
                
            case Op.DUP:
                if self.depth() < 1:
                    self.valid = False
                    return False
                self.push(self.peek())
                
            case Op.DROP:
                if self.depth() < 1:
                    self.valid = False
                    return False
                self.pop()
                
            case Op.SWAP:
                if self.depth() < 2:
                    self.valid = False
                    return False
                a, b = self.pop(), self.pop()
                self.push(a)
                self.push(b)
                
            case Op.ROT:
                if self.depth() < 3:
                    self.valid = False
                    return False
                c, b, a = self.pop(), self.pop(), self.pop()
                self.push(b)
                self.push(c)
                self.push(a)
                
            case Op.OVER:
                if self.depth() < 2:
                    self.valid = False
                    return False
                self.push(self.peek(1))
                
            case Op.NIP:
                if self.depth() < 2:
                    self.valid = False
                    return False
                a = self.pop()
                self.pop()
                self.push(a)
                
            case Op.PICK:
                # Handled above as int
                pass
                
        return self.valid


def symbolic_execute(program: Program, inputs: list) -> Optional[SymbolicStack]:
    """Execute program symbolically, return final stack or None if invalid."""
    stack = SymbolicStack(inputs)
    
    for op in program.ops:
        if not stack.execute(op):
            return None
    
    return stack


# ============================================================================
# Z3 Verification
# ============================================================================

def verify_equivalence(
    spec_func: Callable,
    candidate: Program,
    input_names: list[str],
    output_count: int
) -> tuple[bool, Optional[list]]:
    """
    Verify that candidate is equivalent to spec.
    Returns (is_equivalent, counterexample_or_none).
    """
    # Create symbolic inputs
    inputs = [Int(name) for name in input_names]
    
    # Execute spec
    spec_outputs = spec_func(*inputs)
    if not isinstance(spec_outputs, (list, tuple)):
        spec_outputs = [spec_outputs]
    
    # Execute candidate symbolically
    stack = symbolic_execute(candidate, list(inputs))
    if stack is None or not stack.valid:
        return False, None
    
    if stack.depth() != output_count:
        return False, None
    
    # Get candidate outputs (top of stack)
    candidate_outputs = stack.stack[-output_count:]
    
    # Create solver and check equivalence
    solver = Solver()
    
    # Add constraints: outputs must differ for some input
    constraints = []
    for spec_out, cand_out in zip(spec_outputs, candidate_outputs):
        constraints.append(spec_out != cand_out)
    
    solver.add(Or(constraints))
    
    result = solver.check()
    
    if result == unsat:
        # No counterexample exists - programs are equivalent!
        return True, None
    elif result == sat:
        # Found counterexample
        model = solver.model()
        counterexample = {name: model[Int(name)] for name in input_names}
        return False, counterexample
    else:
        # Unknown
        return False, None


# ============================================================================
# Synthesis Engine
# ============================================================================

@dataclass
class SynthesisConfig:
    max_muls: int = 3
    max_length: int = 20
    max_stack: int = 8
    ops: list[Op] = None
    timeout_sec: float = 60.0
    
    def __post_init__(self):
        if self.ops is None:
            self.ops = [Op.ADD, Op.SUB, Op.MUL, Op.DUP, Op.SWAP, Op.ROT, Op.OVER]


def enumerate_programs(
    config: SynthesisConfig,
    input_count: int,
    output_count: int,
    spec_func: Callable,
    input_names: list[str],
) -> Optional[Program]:
    """
    Enumerate programs and find one equivalent to spec with bounded muls.
    """
    start_time = time.time()
    checked = 0
    pruned_type = 0
    pruned_muls = 0
    
    def search(ops_so_far: list, current_muls: int, current_stack: int):
        nonlocal checked, pruned_type, pruned_muls
        
        # Timeout check
        if time.time() - start_time > config.timeout_sec:
            return None
        
        # Check current program
        if len(ops_so_far) > 0:
            program = Program(list(ops_so_far))
            
            # Quick stack simulation check
            stack_sim = input_count
            muls_sim = 0
            valid = True
            
            for op in ops_so_far:
                if isinstance(op, int):
                    if op >= stack_sim:
                        valid = False
                        break
                    stack_sim += 1
                else:
                    match op:
                        case Op.ADD | Op.SUB | Op.MUL | Op.NIP:
                            if stack_sim < 2:
                                valid = False
                                break
                            stack_sim -= 1
                            if op == Op.MUL:
                                muls_sim += 1
                        case Op.DUP | Op.OVER:
                            if stack_sim < 1:
                                valid = False
                                break
                            stack_sim += 1
                        case Op.DROP:
                            if stack_sim < 1:
                                valid = False
                                break
                            stack_sim -= 1
                        case Op.SWAP:
                            if stack_sim < 2:
                                valid = False
                                break
                        case Op.ROT:
                            if stack_sim < 3:
                                valid = False
                                break
            
            if not valid:
                pruned_type += 1
            elif muls_sim > config.max_muls:
                pruned_muls += 1
            elif stack_sim == output_count:
                # Right output size, verify!
                checked += 1
                equiv, _ = verify_equivalence(spec_func, program, input_names, output_count)
                if equiv:
                    return program
        
        # Try extending
        if len(ops_so_far) >= config.max_length:
            return None
        
        for op in config.ops:
            # Prune: don't add mul if already at limit
            if op == Op.MUL and current_muls >= config.max_muls:
                continue
            
            # Prune: stack would overflow
            new_stack = current_stack
            new_muls = current_muls
            match op:
                case Op.ADD | Op.SUB | Op.MUL | Op.NIP:
                    if current_stack < 2:
                        continue
                    new_stack -= 1
                    if op == Op.MUL:
                        new_muls += 1
                case Op.DUP | Op.OVER:
                    if current_stack < 1:
                        continue
                    new_stack += 1
                    if new_stack > config.max_stack:
                        continue
                case Op.DROP:
                    if current_stack < 1:
                        continue
                    new_stack -= 1
                case Op.SWAP:
                    if current_stack < 2:
                        continue
                case Op.ROT:
                    if current_stack < 3:
                        continue
            
            ops_so_far.append(op)
            result = search(ops_so_far, new_muls, new_stack)
            if result:
                return result
            ops_so_far.pop()
        
        return None
    
    result = search([], 0, input_count)
    
    elapsed = time.time() - start_time
    print(f"  Checked: {checked}, Pruned (type): {pruned_type}, Pruned (muls): {pruned_muls}")
    print(f"  Time: {elapsed:.2f}s")
    
    return result


# ============================================================================
# Specifications
# ============================================================================

def karatsuba_spec(a1, a0, b1, b0):
    """
    Standard 2-digit multiplication: (a1*10 + a0) * (b1*10 + b0)
    = a1*b1*100 + (a1*b0 + a0*b1)*10 + a0*b0
    Returns (r2, r1, r0) where result = r2*100 + r1*10 + r0
    """
    r2 = a1 * b1           # coefficient of 100
    r0 = a0 * b0           # coefficient of 1
    r1 = a1 * b0 + a0 * b1 # coefficient of 10
    return [r2, r1, r0]


def matmul_2x2_spec(a, b, c, d, e, f, g, h):
    """
    2x2 matrix multiplication: [[a,b],[c,d]] * [[e,f],[g,h]]
    = [[ae+bg, af+bh], [ce+dg, cf+dh]]
    """
    r00 = a*e + b*g
    r01 = a*f + b*h
    r10 = c*e + d*g
    r11 = c*f + d*h
    return [r00, r01, r10, r11]


def simple_multiply_spec(a, b):
    """Simple multiplication - baseline test."""
    return [a * b]


# ============================================================================
# Main
# ============================================================================

def run_synthesis(name: str, spec_func, input_names: list[str], output_count: int, max_muls: int, max_length: int = 25, timeout: float = 60.0):
    """Run synthesis for a given specification."""
    print(f"\n{'='*60}")
    print(f"SYNTHESIZING: {name}")
    print(f"{'='*60}")
    print(f"  Inputs: {input_names}")
    print(f"  Outputs: {output_count}")
    print(f"  Max muls: {max_muls}")
    print(f"  Max length: {max_length}")
    print()
    
    config = SynthesisConfig(
        max_muls=max_muls,
        max_length=max_length,
        timeout_sec=timeout,
    )
    
    result = enumerate_programs(
        config,
        len(input_names),
        output_count,
        spec_func,
        input_names,
    )
    
    if result:
        print(f"\n✅ FOUND: {result}")
        print(f"   Muls used: {sum(1 for op in result.ops if op == Op.MUL)}")
        
        # Verify once more
        equiv, _ = verify_equivalence(spec_func, result, input_names, output_count)
        print(f"   Verified: {'✅' if equiv else '❌'}")
        return result
    else:
        print(f"\n❌ Not found within constraints")
        return None


def main():
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║          E002: VERIFIED ALGORITHM SYNTHESIS               ║")
    print("╚═══════════════════════════════════════════════════════════╝")
    
    results = {}
    
    # Test 1: Simple multiplication (baseline)
    result = run_synthesis(
        "Simple Multiply",
        simple_multiply_spec,
        ["a", "b"],
        output_count=1,
        max_muls=1,
        max_length=10,
        timeout=10.0,
    )
    results["simple_multiply"] = str(result) if result else None
    
    # Test 2: Karatsuba (3 muls instead of 4)
    result = run_synthesis(
        "Karatsuba",
        karatsuba_spec,
        ["a1", "a0", "b1", "b0"],
        output_count=3,
        max_muls=3,
        max_length=30,
        timeout=120.0,
    )
    results["karatsuba"] = str(result) if result else None
    
    # Save results
    results_dir = Path(__file__).parent / "results"
    results_dir.mkdir(exist_ok=True)
    
    filename = results_dir / f"synthesis_{time.strftime('%Y%m%d_%H%M%S')}.json"
    with open(filename, "w") as f:
        json.dump(results, f, indent=2)
    
    print(f"\nResults saved to: {filename}")


if __name__ == "__main__":
    main()
