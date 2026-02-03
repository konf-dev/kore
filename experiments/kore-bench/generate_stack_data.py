#!/usr/bin/env python3
"""
Generate training data for Stack-Conditioned LLM.

Unlike standard code generation training (problem → solution), we generate:
    (stack_before, goal_stack, program_so_far) → next_token

This trains the LLM to predict tokens conditioned on actual execution state.

Usage:
    python generate_stack_data.py --output stack_transitions.jsonl --count 10000
"""

import json
import random
import itertools
from dataclasses import dataclass, asdict
from typing import List, Tuple, Optional, Dict, Any
from pathlib import Path
import argparse


# ============================================================================
# Kore Simulator (pure Python, matches Kore semantics)
# ============================================================================

class KoreSimulator:
    """Simulates Kore execution for training data generation."""
    
    # Token vocabulary with effects
    TOKENS = {
        # Literals: (effect, value_generator)
        "0": ((0, 1), lambda: 0),
        "1": ((0, 1), lambda: 1),
        "2": ((0, 1), lambda: 2),
        "3": ((0, 1), lambda: 3),
        "4": ((0, 1), lambda: 4),
        "5": ((0, 1), lambda: 5),
        "6": ((0, 1), lambda: 6),
        "7": ((0, 1), lambda: 7),
        "8": ((0, 1), lambda: 8),
        "9": ((0, 1), lambda: 9),
        "10": ((0, 1), lambda: 10),
        
        # Stack ops: (effect, operation)
        "dup": ((1, 2), None),
        "drop": ((1, 0), None),
        "swap": ((2, 2), None),
        "over": ((2, 3), None),
        "rot": ((3, 3), None),
        "nip": ((2, 1), None),
        
        # Arithmetic
        "add": ((2, 1), None),
        "sub": ((2, 1), None),
        "mul": ((2, 1), None),
        "div": ((2, 1), None),
        "mod": ((2, 1), None),
        "neg": ((1, 1), None),
        
        # Comparison
        "eq": ((2, 1), None),
        "lt": ((2, 1), None),
        "gt": ((2, 1), None),
        "le": ((2, 1), None),
        "ge": ((2, 1), None),
        
        # Logic
        "and": ((2, 1), None),
        "or": ((2, 1), None),
        "not": ((1, 1), None),
        
        # Booleans
        "true": ((0, 1), lambda: True),
        "false": ((0, 1), lambda: False),
    }
    
    def effect_of(self, token: str) -> Tuple[int, int]:
        """Get (consumes, produces) for token."""
        if token in self.TOKENS:
            return self.TOKENS[token][0]
        try:
            int(token)
            return (0, 1)
        except:
            return (0, 0)
    
    def apply(self, stack: List, token: str) -> Tuple[List, bool]:
        """Apply token to stack. Returns (new_stack, success)."""
        stack = stack.copy()
        consumes, _ = self.effect_of(token)
        
        if len(stack) < consumes:
            return stack, False
        
        try:
            # Literals
            if token.lstrip('-').isdigit():
                stack.append(int(token))
            elif token == "true":
                stack.append(True)
            elif token == "false":
                stack.append(False)
            
            # Stack operations
            elif token == "dup":
                stack.append(stack[-1])
            elif token == "drop":
                stack.pop()
            elif token == "swap":
                stack[-1], stack[-2] = stack[-2], stack[-1]
            elif token == "over":
                stack.append(stack[-2])
            elif token == "rot":
                a = stack.pop()
                b = stack.pop()
                c = stack.pop()
                stack.extend([b, a, c])
            elif token == "nip":
                del stack[-2]
            
            # Arithmetic
            elif token == "add":
                b, a = stack.pop(), stack.pop()
                stack.append(a + b)
            elif token == "sub":
                b, a = stack.pop(), stack.pop()
                stack.append(a - b)
            elif token == "mul":
                b, a = stack.pop(), stack.pop()
                stack.append(a * b)
            elif token == "div":
                b, a = stack.pop(), stack.pop()
                if b == 0:
                    return stack, False
                stack.append(a // b)
            elif token == "mod":
                b, a = stack.pop(), stack.pop()
                if b == 0:
                    return stack, False
                stack.append(a % b)
            elif token == "neg":
                stack[-1] = -stack[-1]
            
            # Comparison
            elif token == "eq":
                b, a = stack.pop(), stack.pop()
                stack.append(a == b)
            elif token == "lt":
                b, a = stack.pop(), stack.pop()
                stack.append(a < b)
            elif token == "gt":
                b, a = stack.pop(), stack.pop()
                stack.append(a > b)
            elif token == "le":
                b, a = stack.pop(), stack.pop()
                stack.append(a <= b)
            elif token == "ge":
                b, a = stack.pop(), stack.pop()
                stack.append(a >= b)
            
            # Logic
            elif token == "and":
                b, a = stack.pop(), stack.pop()
                stack.append(bool(a and b))
            elif token == "or":
                b, a = stack.pop(), stack.pop()
                stack.append(bool(a or b))
            elif token == "not":
                stack[-1] = not stack[-1]
            
            else:
                return stack, False
            
            return stack, True
            
        except Exception:
            return stack, False
    
    def execute(self, program: str, initial_stack: List = None) -> Tuple[List, bool]:
        """Execute full program. Returns (final_stack, success)."""
        stack = list(initial_stack) if initial_stack else []
        
        for token in program.split():
            stack, success = self.apply(stack, token)
            if not success:
                return stack, False
        
        return stack, True


# ============================================================================
# Training Data Types
# ============================================================================

@dataclass
class StackTransition:
    """A single stack transition for training."""
    stack_before: List[Any]
    stack_after: List[Any]
    token: str
    program_so_far: str
    full_program: str
    step_index: int
    total_steps: int


@dataclass
class ProgramWithStates:
    """A program with all intermediate stack states."""
    program: str
    initial_stack: List[Any]
    final_stack: List[Any]
    transitions: List[StackTransition]


@dataclass
class GoalConditionedSample:
    """Training sample for goal-conditioned prediction."""
    stack_before: List[Any]
    goal_stack: List[Any]
    program_so_far: str
    next_token: str
    distance_to_goal: int  # Steps remaining
    # Alternative tokens and their resulting stacks (for contrastive learning)
    alternatives: List[Dict[str, Any]]


# ============================================================================
# Program Generator
# ============================================================================

class ProgramGenerator:
    """Generates valid Kore programs with stack traces."""
    
    def __init__(self, max_depth: int = 8, max_value: int = 10):
        self.simulator = KoreSimulator()
        self.max_depth = max_depth
        self.max_value = max_value
        
        # Token weights for sampling
        self.token_weights = {
            # Literals (common)
            "0": 2, "1": 5, "2": 5, "3": 4, "4": 3, "5": 3,
            "6": 2, "7": 2, "8": 2, "9": 2, "10": 2,
            # Stack ops
            "dup": 5, "drop": 2, "swap": 4, "over": 3, "rot": 2,
            # Arithmetic (very common)
            "add": 8, "sub": 5, "mul": 6, "div": 3, "mod": 2, "neg": 2,
            # Comparison
            "eq": 3, "lt": 3, "gt": 3, "le": 2, "ge": 2,
            # Logic
            "and": 2, "or": 2, "not": 2,
            # Booleans
            "true": 2, "false": 2,
        }
    
    def generate_program(
        self,
        target_length: int = None,
        initial_stack: List = None
    ) -> Optional[ProgramWithStates]:
        """Generate a random valid program with stack trace."""
        if target_length is None:
            target_length = random.randint(2, self.max_depth)
        
        if initial_stack is None:
            initial_stack = []
        
        stack = list(initial_stack)
        tokens = []
        transitions = []
        
        for step in range(target_length):
            # Get valid tokens for current stack
            valid_tokens = self._get_valid_tokens(stack)
            
            if not valid_tokens:
                break
            
            # Sample token with weights
            weights = [self.token_weights.get(t, 1) for t in valid_tokens]
            token = random.choices(valid_tokens, weights=weights, k=1)[0]
            
            # Record transition
            stack_before = stack.copy()
            stack, success = self.simulator.apply(stack, token)
            
            if not success:
                break
            
            # Check for overflow
            if any(isinstance(v, int) and abs(v) > 10000 for v in stack):
                break
            
            tokens.append(token)
            transitions.append(StackTransition(
                stack_before=stack_before,
                stack_after=stack.copy(),
                token=token,
                program_so_far=" ".join(tokens[:-1]),
                full_program=" ".join(tokens),
                step_index=step,
                total_steps=target_length
            ))
        
        if not tokens:
            return None
        
        return ProgramWithStates(
            program=" ".join(tokens),
            initial_stack=initial_stack,
            final_stack=stack,
            transitions=transitions
        )
    
    def _get_valid_tokens(self, stack: List) -> List[str]:
        """Get tokens that can be applied to current stack."""
        valid = []
        depth = len(stack)
        
        for token, (effect, _) in self.simulator.TOKENS.items():
            consumes, _ = effect
            if depth >= consumes:
                # Additional checks
                if token == "div" or token == "mod":
                    # Avoid division by zero
                    if depth >= 1 and stack[-1] == 0:
                        continue
                valid.append(token)
        
        # Add random numbers
        for i in range(self.max_value + 1):
            valid.append(str(i))
        
        return valid
    
    def generate_goal_conditioned_sample(
        self,
        program: ProgramWithStates
    ) -> List[GoalConditionedSample]:
        """Convert program to goal-conditioned training samples."""
        samples = []
        
        for i, transition in enumerate(program.transitions):
            # Goal is the final stack
            goal_stack = program.final_stack
            
            # Get alternatives (other valid tokens)
            valid_tokens = self._get_valid_tokens(transition.stack_before)
            alternatives = []
            
            for alt_token in random.sample(valid_tokens, min(5, len(valid_tokens))):
                if alt_token != transition.token:
                    alt_stack, success = self.simulator.apply(
                        transition.stack_before.copy(), 
                        alt_token
                    )
                    if success:
                        alternatives.append({
                            "token": alt_token,
                            "stack_after": alt_stack
                        })
            
            samples.append(GoalConditionedSample(
                stack_before=transition.stack_before,
                goal_stack=goal_stack,
                program_so_far=transition.program_so_far,
                next_token=transition.token,
                distance_to_goal=len(program.transitions) - i,
                alternatives=alternatives
            ))
        
        return samples


# ============================================================================
# Specific Task Generators
# ============================================================================

class TaskGenerator:
    """Generates specific computational tasks."""
    
    def __init__(self):
        self.simulator = KoreSimulator()
        self.program_gen = ProgramGenerator()
    
    def generate_arithmetic_task(self) -> GoalConditionedSample:
        """Generate: compute a specific result from numbers."""
        # Random target
        target = random.randint(0, 100)
        
        # Generate program that produces target
        for _ in range(100):
            length = random.randint(2, 6)
            prog = self.program_gen.generate_program(length)
            if prog and len(prog.final_stack) == 1:
                if isinstance(prog.final_stack[0], int):
                    if prog.final_stack[0] == target:
                        samples = self.program_gen.generate_goal_conditioned_sample(prog)
                        if samples:
                            return samples[0]
        
        return None
    
    def generate_stack_manipulation_task(self) -> GoalConditionedSample:
        """Generate: rearrange stack elements."""
        # Start with some numbers
        initial = [random.randint(1, 9) for _ in range(random.randint(2, 4))]
        
        # Apply random stack operations
        stack = initial.copy()
        tokens = []
        
        for _ in range(random.randint(1, 4)):
            ops = ["dup", "swap", "over", "rot", "drop"]
            valid_ops = [op for op in ops if self.simulator.effect_of(op)[0] <= len(stack)]
            
            if not valid_ops:
                break
            
            op = random.choice(valid_ops)
            new_stack, success = self.simulator.apply(stack, op)
            if success and len(new_stack) <= 6:
                stack = new_stack
                tokens.append(op)
        
        if not tokens:
            return None
        
        program = " ".join(str(v) for v in initial) + " " + " ".join(tokens)
        final_stack, success = self.simulator.execute(program)
        
        if success:
            return GoalConditionedSample(
                stack_before=initial,
                goal_stack=final_stack,
                program_so_far=" ".join(str(v) for v in initial),
                next_token=tokens[0] if tokens else "",
                distance_to_goal=len(tokens),
                alternatives=[]
            )
        
        return None
    
    def generate_comparison_task(self) -> GoalConditionedSample:
        """Generate: compare two values."""
        a = random.randint(0, 20)
        b = random.randint(0, 20)
        
        ops = ["eq", "lt", "gt", "le", "ge"]
        op = random.choice(ops)
        
        stack, _ = self.simulator.execute(f"{a} {b} {op}")
        
        return GoalConditionedSample(
            stack_before=[a, b],
            goal_stack=stack,
            program_so_far=f"{a} {b}",
            next_token=op,
            distance_to_goal=1,
            alternatives=[
                {"token": alt_op, "stack_after": self.simulator.execute(f"{a} {b} {alt_op}")[0]}
                for alt_op in ops if alt_op != op
            ]
        )


# ============================================================================
# Dataset Generator
# ============================================================================

def generate_dataset(
    output_path: str,
    count: int = 10000,
    include_tasks: bool = True
) -> None:
    """Generate full training dataset."""
    
    generator = ProgramGenerator()
    task_gen = TaskGenerator()
    
    samples = []
    
    print(f"Generating {count} samples...")
    
    for i in range(count):
        if i % 1000 == 0:
            print(f"  Progress: {i}/{count}")
        
        # Mix of generation strategies
        strategy = random.choice(["random", "arithmetic", "stack", "comparison"])
        
        try:
            if strategy == "random":
                prog = generator.generate_program(random.randint(2, 8))
                if prog:
                    new_samples = generator.generate_goal_conditioned_sample(prog)
                    samples.extend(new_samples)
            
            elif strategy == "arithmetic" and include_tasks:
                sample = task_gen.generate_arithmetic_task()
                if sample:
                    samples.append(sample)
            
            elif strategy == "stack" and include_tasks:
                sample = task_gen.generate_stack_manipulation_task()
                if sample:
                    samples.append(sample)
            
            elif strategy == "comparison" and include_tasks:
                sample = task_gen.generate_comparison_task()
                if sample:
                    samples.append(sample)
        except Exception as e:
            continue
    
    # Shuffle and write
    random.shuffle(samples)
    
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    
    with open(output_file, 'w') as f:
        for sample in samples:
            f.write(json.dumps(asdict(sample)) + "\n")
    
    print(f"Generated {len(samples)} samples → {output_path}")
    
    # Print statistics
    print("\nDataset Statistics:")
    print(f"  Total samples: {len(samples)}")
    print(f"  Unique tokens: {len(set(s.next_token for s in samples))}")
    print(f"  Avg distance to goal: {sum(s.distance_to_goal for s in samples) / len(samples):.2f}")
    print(f"  Samples with alternatives: {sum(1 for s in samples if s.alternatives)}")


# ============================================================================
# Training Data Format Converter
# ============================================================================

def convert_to_huggingface_format(
    input_path: str,
    output_path: str
) -> None:
    """Convert to HuggingFace datasets format for LLM training."""
    
    samples = []
    with open(input_path) as f:
        for line in f:
            sample = json.loads(line)
            
            # Format as text prompt
            prompt = f"""Current stack: {sample['stack_before']}
Goal stack: {sample['goal_stack']}
Program so far: {sample['program_so_far'] or '(empty)'}

What is the next token?"""
            
            completion = sample['next_token']
            
            samples.append({
                "prompt": prompt,
                "completion": completion,
                "stack_before": sample['stack_before'],
                "goal_stack": sample['goal_stack'],
            })
    
    with open(output_path, 'w') as f:
        for sample in samples:
            f.write(json.dumps(sample) + "\n")
    
    print(f"Converted {len(samples)} samples → {output_path}")


# ============================================================================
# Main
# ============================================================================

def main():
    parser = argparse.ArgumentParser(description="Generate stack-conditioned training data")
    parser.add_argument("--output", type=str, default="./data/stack_transitions.jsonl",
                        help="Output path for training data")
    parser.add_argument("--count", type=int, default=10000,
                        help="Number of samples to generate")
    parser.add_argument("--convert-hf", type=str, default=None,
                        help="Also convert to HuggingFace format at this path")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed")
    args = parser.parse_args()
    
    random.seed(args.seed)
    
    generate_dataset(args.output, args.count)
    
    if args.convert_hf:
        convert_to_huggingface_format(args.output, args.convert_hf)


if __name__ == "__main__":
    main()
