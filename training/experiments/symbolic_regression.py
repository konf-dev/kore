"""
Symbolic Regression via Kore Effect-Guided Search

Goal: Rediscover physics formulas from data
Key advantage: Effect system prunes invalid programs before execution

Example:
  Data: [(m, v, E)] where E = 0.5 * m * v²
  Target effect: (2 -- 1)  [2 inputs → 1 output]
  
  Wrong effect programs rejected immediately:
    - "dup" → (1 -- 2) ✗
    - "add add" → (3 -- 1) ✗
  
  Right effect programs evaluated on data:
    - "mul" → (2 -- 1) ✓ but wrong formula
    - "swap dup mul mul 2 div" → (2 -- 1) ✓ correct!
"""

import json
import numpy as np
from dataclasses import dataclass, field
from typing import List, Dict, Tuple, Optional, Callable
from pathlib import Path
import subprocess
import random


# =============================================================================
# Feynman Benchmark Equations
# =============================================================================

@dataclass
class SymbolicTask:
    """A symbolic regression task."""
    name: str
    description: str
    n_inputs: int
    formula: str  # Human readable
    kore_solution: str  # Known Kore solution
    data_generator: Callable  # Generates (inputs, output) pairs
    
    @property
    def target_effect(self) -> Tuple[int, int]:
        return (self.n_inputs, 1)


def generate_kinetic_energy_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """E = 0.5 * m * v²"""
    data = []
    for _ in range(n):
        m = random.uniform(0.1, 100)
        v = random.uniform(0.1, 50)
        E = 0.5 * m * v * v
        data.append(([m, v], E))
    return data


def generate_coulomb_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """F = k * q1 * q2 / r² (we'll set k=1 for simplicity)"""
    data = []
    for _ in range(n):
        q1 = random.uniform(-10, 10)
        q2 = random.uniform(-10, 10)
        r = random.uniform(0.1, 10)
        F = q1 * q2 / (r * r)
        data.append(([q1, q2, r], F))
    return data


def generate_gravitational_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """F = G * m1 * m2 / r² (we'll set G=1)"""
    data = []
    for _ in range(n):
        m1 = random.uniform(1, 100)
        m2 = random.uniform(1, 100)
        r = random.uniform(0.1, 10)
        F = m1 * m2 / (r * r)
        data.append(([m1, m2, r], F))
    return data


def generate_ideal_gas_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """PV = nRT → P = nRT/V (R=1)"""
    data = []
    for _ in range(n):
        n_mol = random.uniform(0.1, 10)
        T = random.uniform(100, 500)
        V = random.uniform(1, 100)
        P = n_mol * T / V
        data.append(([n_mol, T, V], P))
    return data


def generate_de_broglie_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """λ = h / (m * v) (h=1)"""
    data = []
    for _ in range(n):
        m = random.uniform(0.1, 10)
        v = random.uniform(0.1, 100)
        wavelength = 1.0 / (m * v)
        data.append(([m, v], wavelength))
    return data


def generate_simple_quadratic_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """y = x²"""
    data = []
    for _ in range(n):
        x = random.uniform(-10, 10)
        y = x * x
        data.append(([x], y))
    return data


def generate_pythagorean_data(n: int = 100) -> List[Tuple[List[float], float]]:
    """c = √(a² + b²)"""
    data = []
    for _ in range(n):
        a = random.uniform(0.1, 10)
        b = random.uniform(0.1, 10)
        c = np.sqrt(a*a + b*b)
        data.append(([a, b], c))
    return data


# Task registry
SYMBOLIC_TASKS = {
    "square": SymbolicTask(
        name="square",
        description="y = x²",
        n_inputs=1,
        formula="x²",
        kore_solution="dup mul",
        data_generator=generate_simple_quadratic_data,
    ),
    "kinetic_energy": SymbolicTask(
        name="kinetic_energy",
        description="E = 0.5 * m * v²",
        n_inputs=2,
        formula="0.5 * m * v²",
        kore_solution="swap dup mul mul 0.5 mul",  # [m v] → [0.5*m*v²]
        data_generator=generate_kinetic_energy_data,
    ),
    "coulomb": SymbolicTask(
        name="coulomb",
        description="F = q1 * q2 / r²",
        n_inputs=3,
        formula="q1 * q2 / r²",
        kore_solution="rot rot mul swap dup mul div",  # [q1 q2 r] → [q1*q2/r²]
        data_generator=generate_coulomb_data,
    ),
    "gravity": SymbolicTask(
        name="gravity", 
        description="F = m1 * m2 / r²",
        n_inputs=3,
        formula="m1 * m2 / r²",
        kore_solution="rot rot mul swap dup mul div",
        data_generator=generate_gravitational_data,
    ),
    "ideal_gas": SymbolicTask(
        name="ideal_gas",
        description="P = n * T / V",
        n_inputs=3,
        formula="n * T / V",
        kore_solution="rot rot mul swap div",  # [n T V] → [n*T/V]
        data_generator=generate_ideal_gas_data,
    ),
    "de_broglie": SymbolicTask(
        name="de_broglie",
        description="λ = 1 / (m * v)",
        n_inputs=2,
        formula="1 / (m * v)",
        kore_solution="mul 1 swap div",  # [m v] → [1/(m*v)]
        data_generator=generate_de_broglie_data,
    ),
    "pythagorean": SymbolicTask(
        name="pythagorean",
        description="c = √(a² + b²)",
        n_inputs=2,
        formula="√(a² + b²)",
        kore_solution="dup mul swap dup mul add 0.5 pow",  # need sqrt
        data_generator=generate_pythagorean_data,
    ),
}


# =============================================================================
# Kore Interface
# =============================================================================

class KoreSymbolicRuntime:
    """Kore runtime specialized for symbolic regression."""
    
    def __init__(self, binary: str = "target/release/kore"):
        self.binary = binary
        self.timeout = 2.0
        
    def get_effect(self, program: str) -> Optional[Tuple[int, int]]:
        """Get effect of program without executing."""
        analysis_prog = f"[ {program} ] effect-infer"
        result = self.execute(analysis_prog, [])
        
        if result and "stack" in result:
            stack = result["stack"]
            if stack and isinstance(stack[0], dict):
                effect = stack[0].get("effect", {})
                return (effect.get("consumes", 0), effect.get("produces", 0))
        return None
    
    def execute(self, program: str, inputs: List[float]) -> Optional[Dict]:
        """Execute program with numeric inputs."""
        try:
            # Prepare input: put numbers on stack first
            stack_setup = " ".join(str(x) for x in inputs)
            full_program = f"{stack_setup} {program}"
            
            input_data = json.dumps({
                "program": full_program,
                "stack": [],
                "trace": False,
            })
            
            result = subprocess.run(
                [self.binary],
                input=input_data,
                capture_output=True,
                text=True,
                timeout=self.timeout
            )
            
            if result.returncode == 0:
                return json.loads(result.stdout)
            return None
            
        except (subprocess.TimeoutExpired, json.JSONDecodeError, Exception):
            return None
    
    def evaluate_on_data(
        self, 
        program: str, 
        data: List[Tuple[List[float], float]]
    ) -> Tuple[float, List[float]]:
        """Evaluate program on data, return (MSE, predictions)."""
        predictions = []
        errors = []
        
        for inputs, expected in data:
            result = self.execute(program, inputs)
            
            if result and "stack" in result and result["stack"]:
                pred = result["stack"][0]
                if isinstance(pred, (int, float)):
                    predictions.append(pred)
                    errors.append((pred - expected) ** 2)
                else:
                    # Non-numeric result
                    predictions.append(None)
                    errors.append(1e10)
            else:
                # Execution failed
                predictions.append(None)
                errors.append(1e10)
        
        mse = np.mean(errors) if errors else 1e10
        return mse, predictions


# =============================================================================
# Effect-Guided Search
# =============================================================================

class EffectGuidedGenerator:
    """
    Generate programs guided by effect constraints.
    
    Key insight: We can reject programs with wrong effect before execution,
    massively pruning the search space.
    """
    
    # Primitive operations with their effects
    PRIMITIVES = {
        # Stack ops
        "dup": (1, 2),    # (a -- a a)
        "drop": (1, 0),   # (a -- )
        "swap": (2, 2),   # (a b -- b a)
        "over": (2, 3),   # (a b -- a b a)
        "rot": (3, 3),    # (a b c -- b c a)
        
        # Arithmetic
        "add": (2, 1),    # (a b -- a+b)
        "sub": (2, 1),    # (a b -- a-b)
        "mul": (2, 1),    # (a b -- a*b)
        "div": (2, 1),    # (a b -- a/b)
        "neg": (1, 1),    # (a -- -a)
        
        # Constants (effect depends on value)
        # "0.5", "1", "2", etc. have effect (0, 1)
    }
    
    CONSTANTS = ["0.5", "1", "2", "3", "0.25"]
    
    def __init__(self, runtime: KoreSymbolicRuntime):
        self.runtime = runtime
    
    def effect_of_sequence(self, ops: List[str]) -> Tuple[int, int]:
        """Compute effect of operation sequence."""
        consumes, produces = 0, 0
        stack_depth = 0
        
        for op in ops:
            if op in self.PRIMITIVES:
                c, p = self.PRIMITIVES[op]
            elif op in self.CONSTANTS or self._is_number(op):
                c, p = 0, 1
            else:
                # Unknown op, assume identity
                c, p = 0, 0
            
            # Can we apply this op?
            if stack_depth < c:
                # Need to consume from inputs
                needed = c - stack_depth
                consumes += needed
                stack_depth = 0
            else:
                stack_depth -= c
            
            stack_depth += p
        
        produces = stack_depth
        return (consumes, produces)
    
    def _is_number(self, s: str) -> bool:
        try:
            float(s)
            return True
        except:
            return False
    
    def generate_candidates(
        self, 
        target_effect: Tuple[int, int],
        max_length: int = 8,
        n_candidates: int = 100
    ) -> List[str]:
        """Generate candidate programs with target effect."""
        candidates = []
        ops = list(self.PRIMITIVES.keys()) + self.CONSTANTS
        
        for _ in range(n_candidates * 10):  # Generate more, filter by effect
            length = random.randint(1, max_length)
            program_ops = [random.choice(ops) for _ in range(length)]
            
            effect = self.effect_of_sequence(program_ops)
            
            if effect == target_effect:
                candidates.append(" ".join(program_ops))
                
                if len(candidates) >= n_candidates:
                    break
        
        return candidates
    
    def guided_search(
        self,
        target_effect: Tuple[int, int],
        data: List[Tuple[List[float], float]],
        max_iterations: int = 1000,
        beam_size: int = 50
    ) -> List[Tuple[str, float]]:
        """
        Beam search guided by effect and MSE.
        
        1. Generate candidates with correct effect
        2. Evaluate on data
        3. Keep top beam_size
        4. Mutate and repeat
        """
        
        # Initialize beam with random candidates
        candidates = self.generate_candidates(target_effect, n_candidates=beam_size * 2)
        
        # Evaluate
        scored = []
        for prog in candidates:
            mse, _ = self.runtime.evaluate_on_data(prog, data[:20])  # Quick eval
            if mse < 1e9:
                scored.append((prog, mse))
        
        scored.sort(key=lambda x: x[1])
        beam = scored[:beam_size]
        
        best_ever = beam[0] if beam else ("", 1e10)
        
        for iteration in range(max_iterations):
            # Mutate beam
            new_candidates = []
            
            for prog, _ in beam:
                # Mutation 1: Replace random op
                mutated = self._mutate_replace(prog, target_effect)
                if mutated:
                    new_candidates.append(mutated)
                
                # Mutation 2: Insert op
                mutated = self._mutate_insert(prog, target_effect)
                if mutated:
                    new_candidates.append(mutated)
                
                # Mutation 3: Delete op
                mutated = self._mutate_delete(prog, target_effect)
                if mutated:
                    new_candidates.append(mutated)
            
            # Add random candidates
            new_candidates.extend(
                self.generate_candidates(target_effect, n_candidates=beam_size)
            )
            
            # Evaluate all
            for prog in new_candidates:
                if prog not in [p for p, _ in beam]:
                    mse, _ = self.runtime.evaluate_on_data(prog, data[:20])
                    if mse < 1e9:
                        scored.append((prog, mse))
            
            # Keep top
            scored.sort(key=lambda x: x[1])
            beam = scored[:beam_size]
            
            if beam and beam[0][1] < best_ever[1]:
                best_ever = beam[0]
                print(f"Iter {iteration}: New best MSE={best_ever[1]:.6f}: {best_ever[0]}")
            
            # Early stopping
            if best_ever[1] < 1e-6:
                break
        
        # Final evaluation on full data
        final_scored = []
        for prog, _ in beam[:10]:
            mse, _ = self.runtime.evaluate_on_data(prog, data)
            final_scored.append((prog, mse))
        
        final_scored.sort(key=lambda x: x[1])
        return final_scored
    
    def _mutate_replace(self, prog: str, target: Tuple[int, int]) -> Optional[str]:
        """Replace random op while maintaining effect."""
        ops = prog.split()
        if not ops:
            return None
        
        idx = random.randint(0, len(ops) - 1)
        all_ops = list(self.PRIMITIVES.keys()) + self.CONSTANTS
        
        for _ in range(20):  # Try 20 random replacements
            new_op = random.choice(all_ops)
            new_ops = ops[:idx] + [new_op] + ops[idx+1:]
            if self.effect_of_sequence(new_ops) == target:
                return " ".join(new_ops)
        
        return None
    
    def _mutate_insert(self, prog: str, target: Tuple[int, int]) -> Optional[str]:
        """Insert op while maintaining effect."""
        ops = prog.split()
        idx = random.randint(0, len(ops))
        all_ops = list(self.PRIMITIVES.keys()) + self.CONSTANTS
        
        for _ in range(20):
            new_op = random.choice(all_ops)
            new_ops = ops[:idx] + [new_op] + ops[idx:]
            if self.effect_of_sequence(new_ops) == target:
                return " ".join(new_ops)
        
        return None
    
    def _mutate_delete(self, prog: str, target: Tuple[int, int]) -> Optional[str]:
        """Delete op while maintaining effect."""
        ops = prog.split()
        if len(ops) <= 1:
            return None
        
        for _ in range(20):
            idx = random.randint(0, len(ops) - 1)
            new_ops = ops[:idx] + ops[idx+1:]
            if self.effect_of_sequence(new_ops) == target:
                return " ".join(new_ops)
        
        return None


# =============================================================================
# LLM-Guided Symbolic Regression
# =============================================================================

class LLMSymbolicRegressor:
    """
    Use LLM with effect constraints for symbolic regression.
    
    Key innovations:
    1. Effect check before execution (fast rejection)
    2. Trace as chain-of-thought
    3. Algebraic simplification as post-processing
    """
    
    def __init__(self, model, runtime: KoreSymbolicRuntime):
        self.model = model
        self.runtime = runtime
        
    def make_prompt(self, task: SymbolicTask, data_sample: List) -> str:
        """Create prompt for LLM."""
        # Show a few data points
        examples = "\n".join(
            f"  Inputs: {inputs} → Output: {output:.4f}"
            for inputs, output in data_sample[:5]
        )
        
        return f"""You are solving a symbolic regression problem.
Find a Kore program that computes a formula matching the data.

Data:
{examples}
  ... ({len(data_sample)} total points)

Constraints:
- Program must have effect: ({task.n_inputs} -- 1)
  (Takes {task.n_inputs} inputs, produces 1 output)
- Use only: dup drop swap over rot add sub mul div neg
- May use constants: 0.5 1 2 3

Think step by step:
1. What pattern do you see in the data?
2. What formula might produce this?
3. How to express it in stack operations?

Your Kore program (just the operations, no explanation):"""

    def solve(
        self, 
        task: SymbolicTask,
        n_attempts: int = 20,
        temperature: float = 0.7
    ) -> List[Tuple[str, float]]:
        """Attempt to solve symbolic regression task."""
        
        data = task.data_generator(200)  # 200 data points
        train_data = data[:150]
        test_data = data[150:]
        
        results = []
        
        for attempt in range(n_attempts):
            # Generate program
            prompt = self.make_prompt(task, train_data)
            program = self.model.generate(
                prompt, 
                temperature=temperature,
                max_tokens=50
            ).strip()
            
            # Clean up
            program = self._clean_program(program)
            
            # Effect check (fast)
            effect = self.runtime.get_effect(program)
            if effect != task.target_effect:
                print(f"  Attempt {attempt}: Wrong effect {effect}, expected {task.target_effect}")
                continue
            
            # Evaluate on train data
            train_mse, _ = self.runtime.evaluate_on_data(program, train_data)
            
            if train_mse < 1e9:
                # Evaluate on test data
                test_mse, _ = self.runtime.evaluate_on_data(program, test_data)
                
                results.append({
                    "program": program,
                    "train_mse": train_mse,
                    "test_mse": test_mse,
                    "effect": effect,
                })
                
                print(f"  Attempt {attempt}: Train MSE={train_mse:.6f}, Test MSE={test_mse:.6f}")
                print(f"    Program: {program}")
                
                if test_mse < 1e-4:
                    print(f"  Found solution!")
                    break
        
        # Sort by test MSE
        results.sort(key=lambda x: x["test_mse"])
        return results
    
    def _clean_program(self, program: str) -> str:
        """Clean LLM output to valid Kore."""
        # Remove common artifacts
        program = program.split("\n")[0]  # First line only
        program = program.strip()
        
        # Remove backticks, quotes
        program = program.replace("`", "").replace('"', "").replace("'", "")
        
        # Keep only valid tokens
        valid_ops = {"dup", "drop", "swap", "over", "rot", 
                     "add", "sub", "mul", "div", "neg",
                     "0.5", "1", "2", "3", "0.25", "0.1"}
        
        tokens = program.split()
        cleaned = []
        for tok in tokens:
            if tok in valid_ops:
                cleaned.append(tok)
            else:
                try:
                    float(tok)
                    cleaned.append(tok)
                except:
                    pass
        
        return " ".join(cleaned)


# =============================================================================
# Reward Function for RL Training
# =============================================================================

def compute_symbolic_reward(
    program: str,
    task: SymbolicTask,
    runtime: KoreSymbolicRuntime,
    train_data: List,
    test_data: List
) -> Dict:
    """
    Compute reward for symbolic regression with effect shaping.
    
    Reward components:
    1. Effect match (binary, but fast)
    2. Train MSE (continuous)
    3. Test MSE (generalization)
    4. Complexity penalty (Occam's razor)
    """
    
    # 1. Effect check (fast, no execution)
    effect = runtime.get_effect(program)
    
    if effect is None:
        return {
            "reward": -1.0,
            "reason": "invalid_program",
            "components": {}
        }
    
    effect_match = 1.0 if effect == task.target_effect else 0.0
    
    if effect_match == 0.0:
        # Wrong effect - partial credit based on distance
        c_diff = abs(effect[0] - task.target_effect[0])
        p_diff = abs(effect[1] - task.target_effect[1])
        effect_distance = c_diff + p_diff
        
        return {
            "reward": -0.5 - 0.1 * effect_distance,
            "reason": "wrong_effect",
            "effect": effect,
            "target_effect": task.target_effect,
            "components": {"effect_distance": effect_distance}
        }
    
    # 2. Evaluate on train data
    train_mse, _ = runtime.evaluate_on_data(program, train_data)
    
    if train_mse > 1e9:
        return {
            "reward": -0.3,
            "reason": "execution_failed",
            "components": {}
        }
    
    # 3. Evaluate on test data
    test_mse, _ = runtime.evaluate_on_data(program, test_data)
    
    # 4. Complexity
    complexity = len(program.split())
    
    # Compute reward
    accuracy_reward = np.exp(-train_mse)  # 1.0 for perfect, 0 for bad
    generalization_reward = np.exp(-test_mse)
    complexity_penalty = 0.01 * complexity
    
    total_reward = (
        0.5 * accuracy_reward +
        0.4 * generalization_reward -
        0.1 * complexity_penalty
    )
    
    return {
        "reward": total_reward,
        "reason": "evaluated",
        "program": program,
        "train_mse": train_mse,
        "test_mse": test_mse,
        "complexity": complexity,
        "components": {
            "accuracy": accuracy_reward,
            "generalization": generalization_reward,
            "complexity_penalty": complexity_penalty,
        }
    }


# =============================================================================
# Benchmark Runner
# =============================================================================

def run_benchmark(
    tasks: List[str] = None,
    n_attempts: int = 50,
    method: str = "search"  # "search" or "llm"
):
    """Run symbolic regression benchmark."""
    
    if tasks is None:
        tasks = list(SYMBOLIC_TASKS.keys())
    
    runtime = KoreSymbolicRuntime()
    
    results = {}
    
    for task_name in tasks:
        task = SYMBOLIC_TASKS[task_name]
        print(f"\n{'='*60}")
        print(f"Task: {task.name} - {task.description}")
        print(f"Target effect: {task.target_effect}")
        print(f"Known solution: {task.kore_solution}")
        print(f"{'='*60}")
        
        data = task.data_generator(200)
        
        if method == "search":
            generator = EffectGuidedGenerator(runtime)
            solutions = generator.guided_search(
                task.target_effect,
                data,
                max_iterations=n_attempts
            )
        else:
            # LLM method would go here
            solutions = []
        
        if solutions:
            best = solutions[0]
            print(f"\nBest solution found:")
            print(f"  Program: {best[0]}")
            print(f"  MSE: {best[1]:.8f}")
            
            # Compare to known solution
            known_mse, _ = runtime.evaluate_on_data(task.kore_solution, data)
            print(f"  Known solution MSE: {known_mse:.8f}")
            
            results[task_name] = {
                "found": best[0],
                "found_mse": best[1],
                "known": task.kore_solution,
                "known_mse": known_mse,
                "success": best[1] < 1e-4,
            }
        else:
            print("\nNo solution found")
            results[task_name] = {"success": False}
    
    # Summary
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    
    for task_name, result in results.items():
        status = "✓" if result.get("success") else "✗"
        print(f"{status} {task_name}: ", end="")
        if result.get("success"):
            print(f"MSE={result['found_mse']:.6f}")
        else:
            print("failed")
    
    return results


# =============================================================================
# Main
# =============================================================================

if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(description="Symbolic Regression Benchmark")
    parser.add_argument("--tasks", nargs="*", default=None)
    parser.add_argument("--attempts", type=int, default=100)
    parser.add_argument("--method", choices=["search", "llm"], default="search")
    args = parser.parse_args()
    
    results = run_benchmark(
        tasks=args.tasks,
        n_attempts=args.attempts,
        method=args.method
    )
