"""
Task Generators for Kore RL Training

Each phase has specific task types that progressively teach Kore concepts.
"""

import random
from dataclasses import dataclass
from typing import List, Tuple, Optional, Callable, Any
from kore_runtime import EffectSignature


@dataclass
class Task:
    """A training task for the LLM."""
    description: str
    expected_effect: EffectSignature
    test_cases: List[Tuple[List[Any], Any]]  # (input_stack, expected_output)
    hints: List[str] = None
    difficulty: int = 1  # 1-5
    phase: int = 1
    
    def verify(self, program: str, runtime) -> Tuple[bool, str]:
        """Verify program solves this task."""
        
        # Check effect
        effect = runtime.get_effect(program)
        if effect is None:
            return False, "Invalid program (could not parse)"
        
        if not effect.matches(self.expected_effect):
            return False, f"Wrong effect: got {effect.to_string()}, expected {self.expected_effect.to_string()}"
        
        # Check test cases
        for inputs, expected in self.test_cases:
            result = runtime.execute(program, inputs.copy(), trace=False)
            
            if not result.success:
                return False, f"Execution failed on input {inputs}: {result.error}"
            
            if not result.stack:
                return False, f"Empty stack on input {inputs}"
            
            actual = result.stack[-1] if len(result.stack) == 1 else result.stack
            
            # Allow for floating point tolerance
            if isinstance(expected, float) and isinstance(actual, (int, float)):
                if abs(actual - expected) > 1e-6:
                    return False, f"Wrong output on {inputs}: got {actual}, expected {expected}"
            elif actual != expected:
                return False, f"Wrong output on {inputs}: got {actual}, expected {expected}"
        
        return True, "Correct!"


# =============================================================================
# Phase 1: Stack & Arithmetic
# =============================================================================

def generate_arithmetic_task() -> Task:
    """Generate a basic arithmetic task."""
    
    templates = [
        # Two-number operations
        {
            "op": "add",
            "desc": "Add two numbers",
            "effect": EffectSignature(2, 1),
            "gen": lambda: (a := random.randint(1, 100), b := random.randint(1, 100), a + b),
            "solution": "add",
        },
        {
            "op": "sub",
            "desc": "Subtract second from first (a - b)",
            "effect": EffectSignature(2, 1),
            "gen": lambda: (a := random.randint(10, 100), b := random.randint(1, a), a - b),
            "solution": "sub",
        },
        {
            "op": "mul",
            "desc": "Multiply two numbers",
            "effect": EffectSignature(2, 1),
            "gen": lambda: (a := random.randint(1, 20), b := random.randint(1, 20), a * b),
            "solution": "mul",
        },
        # Compound operations
        {
            "op": "square",
            "desc": "Compute the square of a number (n²)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(1, 20), None, n * n),
            "solution": "dup mul",
        },
        {
            "op": "double",
            "desc": "Double a number (n × 2)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(1, 100), None, n * 2),
            "solution": "dup add",
        },
        {
            "op": "sum_three",
            "desc": "Sum three numbers (a + b + c)",
            "effect": EffectSignature(3, 1),
            "gen": lambda: (a := random.randint(1, 50), b := random.randint(1, 50), c := random.randint(1, 50), a + b + c),
            "solution": "add add",
        },
    ]
    
    template = random.choice(templates)
    
    # Generate test cases
    test_cases = []
    for _ in range(5):
        result = template["gen"]()
        if len(result) == 3:
            a, b, expected = result
            inputs = [a] if b is None else [a, b]
        else:
            a, b, c, expected = result
            inputs = [a, b, c]
        test_cases.append((inputs, expected))
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=test_cases,
        hints=[f"Hint: solution uses {len(template['solution'].split())} operation(s)"],
        difficulty=1,
        phase=1,
    )


def generate_stack_manipulation_task() -> Task:
    """Generate a stack manipulation task."""
    
    templates = [
        {
            "desc": "Swap the top two elements",
            "effect": EffectSignature(2, 2),
            "verify": lambda inp, out: out == [inp[1], inp[0]],
            "solution": "swap",
        },
        {
            "desc": "Duplicate the top element",
            "effect": EffectSignature(1, 2),
            "verify": lambda inp, out: out == [inp[0], inp[0]],
            "solution": "dup",
        },
        {
            "desc": "Remove the top element, keep the second",
            "effect": EffectSignature(2, 1),
            "verify": lambda inp, out: out == [inp[0]],
            "solution": "drop",
        },
        {
            "desc": "Given (a b), produce (a b a) - copy second to top",
            "effect": EffectSignature(2, 3),
            "verify": lambda inp, out: out == [inp[0], inp[1], inp[0]],
            "solution": "over",
        },
        {
            "desc": "Given (a b c), produce (b c a) - rotate top three",
            "effect": EffectSignature(3, 3),
            "verify": lambda inp, out: out == [inp[1], inp[2], inp[0]],
            "solution": "rot",
        },
        {
            "desc": "Given (a b), produce (b) - remove second, keep top",
            "effect": EffectSignature(2, 1),
            "verify": lambda inp, out: out == [inp[1]],
            "solution": "swap drop",
        },
    ]
    
    template = random.choice(templates)
    
    # Generate test cases
    test_cases = []
    for _ in range(5):
        n_inputs = template["effect"].consumes
        inputs = [random.randint(1, 100) for _ in range(n_inputs)]
        
        # Compute expected output
        if template["effect"].produces == 1:
            expected = template["verify"](inputs, None)
            # Re-run to get actual expected
            if "drop" in template["solution"] and "swap" in template["solution"]:
                expected = inputs[1]
            elif template["solution"] == "drop":
                expected = inputs[0]
        else:
            # Multiple outputs - verify function checks list
            if template["solution"] == "swap":
                expected = [inputs[1], inputs[0]]
            elif template["solution"] == "dup":
                expected = [inputs[0], inputs[0]]
            elif template["solution"] == "over":
                expected = [inputs[0], inputs[1], inputs[0]]
            elif template["solution"] == "rot":
                expected = [inputs[1], inputs[2], inputs[0]]
            else:
                expected = inputs  # fallback
        
        test_cases.append((inputs, expected))
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=test_cases,
        difficulty=1,
        phase=1,
    )


# =============================================================================
# Phase 2: Control Flow
# =============================================================================

def generate_conditional_task() -> Task:
    """Generate a conditional logic task."""
    
    templates = [
        {
            "desc": "Return the absolute value of a number",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(-50, 50), abs(n)),
            "solution": "dup 0 lt [ neg ] [ ] if",
        },
        {
            "desc": "Return the maximum of two numbers",
            "effect": EffectSignature(2, 1),
            "gen": lambda: (a := random.randint(1, 100), b := random.randint(1, 100), max(a, b)),
            "solution": "over over lt [ swap ] [ ] if drop",
        },
        {
            "desc": "Return the minimum of two numbers",
            "effect": EffectSignature(2, 1),
            "gen": lambda: (a := random.randint(1, 100), b := random.randint(1, 100), min(a, b)),
            "solution": "over over lt [ ] [ swap ] if drop",
        },
        {
            "desc": "Return 1 if positive, -1 if negative, 0 if zero (sign function)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(-50, 50), 1 if n > 0 else (-1 if n < 0 else 0)),
            "solution": "dup 0 lt [ drop -1 ] [ 0 gt [ 1 ] [ 0 ] if ] if",
        },
    ]
    
    template = random.choice(templates)
    
    test_cases = []
    for _ in range(5):
        result = template["gen"]()
        inputs = [result[0]] if len(result) == 2 else [result[0], result[1]]
        expected = result[-1]
        test_cases.append((inputs, expected))
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=test_cases,
        difficulty=2,
        phase=2,
    )


def generate_loop_task() -> Task:
    """Generate a loop-based task."""
    
    templates = [
        {
            "desc": "Compute factorial of n (n!)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(1, 10), 
                          (lambda x: 1 if x <= 1 else x * (lambda f, y: f(f, y))(lambda f, y: 1 if y <= 1 else y * f(f, y-1), x-1))(n)),
            "solution": "1 swap [ dup 1 gt ] [ dup rot mul swap 1 sub ] while drop",
        },
        {
            "desc": "Compute the sum 1 + 2 + ... + n",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(1, 20), n * (n + 1) // 2),
            "solution": "0 swap [ dup 0 gt ] [ dup rot add swap 1 sub ] while drop",
        },
        {
            "desc": "Compute n-th Fibonacci number (fib(1)=1, fib(2)=1)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(1, 15), 
                          (lambda x: [0,1,1,2,3,5,8,13,21,34,55,89,144,233,377,610][x] if x <= 15 else 0)(n)),
            "solution": "1 1 rot 2 sub [ over over add rot drop swap ] times drop",
        },
        {
            "desc": "Compute 2^n (power of 2)",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (n := random.randint(0, 10), 2 ** n),
            "solution": "1 swap [ dup 0 gt ] [ swap 2 mul swap 1 sub ] while drop",
        },
    ]
    
    template = random.choice(templates)
    
    test_cases = []
    for _ in range(5):
        n, expected = template["gen"]()
        test_cases.append(([n], expected))
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=test_cases,
        hints=["Use 'times' for fixed iterations or 'while' for conditional loops"],
        difficulty=3,
        phase=2,
    )


# =============================================================================
# Phase 3: Data Structures
# =============================================================================

def generate_list_task() -> Task:
    """Generate a list manipulation task."""
    
    templates = [
        {
            "desc": "Compute the sum of all elements in a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 20) for _ in range(random.randint(2, 6))], sum(lst)),
            "solution": "0 swap [ add ] fold",
        },
        {
            "desc": "Compute the length of a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 20) for _ in range(random.randint(1, 10))], len(lst)),
            "solution": "list-len",
        },
        {
            "desc": "Get the first element of a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 100) for _ in range(random.randint(1, 5))], lst[0]),
            "solution": "0 list-get",
        },
        {
            "desc": "Double each element in a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 20) for _ in range(random.randint(2, 5))], [x * 2 for x in lst]),
            "solution": "[ dup add ] map",
        },
        {
            "desc": "Keep only even numbers from a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 20) for _ in range(random.randint(3, 8))], [x for x in lst if x % 2 == 0]),
            "solution": "[ 2 mod 0 eq ] filter",
        },
        {
            "desc": "Compute the product of all elements in a list",
            "effect": EffectSignature(1, 1),
            "gen": lambda: (lst := [random.randint(1, 5) for _ in range(random.randint(2, 4))], 
                          (lambda l: 1 if not l else l[0] * (lambda f, rest: 1 if not rest else rest[0] * f(f, rest[1:]))(lambda f, rest: 1 if not rest else rest[0] * f(f, rest[1:]), l[1:]))(lst)),
            "solution": "1 swap [ mul ] fold",
        },
    ]
    
    template = random.choice(templates)
    
    test_cases = []
    for _ in range(5):
        lst, expected = template["gen"]()
        test_cases.append(([lst], expected))
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=test_cases,
        hints=["Use map/filter/fold for list processing"],
        difficulty=2,
        phase=3,
    )


# =============================================================================
# Phase 4: Introspection
# =============================================================================

def generate_introspection_task() -> Task:
    """Generate an introspection/meta-programming task."""
    
    templates = [
        {
            "desc": "Check if a quoted program is pure (has no IO effects)",
            "effect": EffectSignature(1, 1),
            "test_cases": [
                ([[["dup", "mul"]], True]),
                ([[["fs-read"]], False]),
            ],
            "solution": "effect-infer \"pure\" map-get",
        },
        {
            "desc": "Get the net stack change of a quoted program",
            "effect": EffectSignature(1, 1),
            "test_cases": [
                ([[["dup"]], 1]),  # (1--2) = +1
                ([[["drop"]], -1]),  # (1--0) = -1
                ([[["add"]], -1]),  # (2--1) = -1
                ([[["dup", "mul"]], 0]),  # (1--1) = 0
            ],
            "solution": "effect-infer \"effect\" map-get dup \"produces\" map-get swap \"consumes\" map-get sub",
        },
    ]
    
    template = random.choice(templates)
    
    return Task(
        description=template["desc"],
        expected_effect=template["effect"],
        test_cases=template["test_cases"],
        hints=["Use effect-infer to analyze quoted programs"],
        difficulty=3,
        phase=4,
    )


# =============================================================================
# Task Generator Registry
# =============================================================================

PHASE_GENERATORS = {
    1: [generate_arithmetic_task, generate_stack_manipulation_task],
    2: [generate_conditional_task, generate_loop_task],
    3: [generate_list_task],
    4: [generate_introspection_task],
    # Phases 5-6 would add IO and advanced tasks
}


def generate_task(phase: int) -> Task:
    """Generate a random task for the given phase."""
    
    # Include all tasks from this and previous phases
    available_generators = []
    for p in range(1, phase + 1):
        if p in PHASE_GENERATORS:
            available_generators.extend(PHASE_GENERATORS[p])
    
    if not available_generators:
        available_generators = PHASE_GENERATORS[1]
    
    generator = random.choice(available_generators)
    return generator()


def generate_task_batch(phase: int, n: int) -> List[Task]:
    """Generate a batch of tasks."""
    return [generate_task(phase) for _ in range(n)]


# =============================================================================
# Test
# =============================================================================

if __name__ == "__main__":
    print("Testing Task Generators...")
    
    for phase in range(1, 5):
        print(f"\n=== Phase {phase} ===")
        for _ in range(3):
            task = generate_task(phase)
            print(f"  {task.description}")
            print(f"    Effect: {task.expected_effect.to_string()}")
            print(f"    Example: {task.test_cases[0]}")
    
    print("\n✓ Task generator test passed!")
