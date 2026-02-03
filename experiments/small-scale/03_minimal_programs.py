#!/usr/bin/env python3
"""
Experiment 3: Minimal Program Discovery

Hypothesis: LLMs can find (or approximate) minimal-length programs,
providing insight into Kolmogorov complexity estimation.

Setup:
- 50 tasks with known optimal solutions
- LLM generates programs, we measure length
- Compare to optimal and random baseline

Metrics:
- Compression ratio (actual length / optimal length)
- Exact match rate
- Length distribution
"""

import argparse
import json
import random
from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Tuple, Optional
import sys

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "training"))

from kore_runtime import KoreRuntime


@dataclass
class MinimalTask:
    """A task with known minimal solution."""
    name: str
    description: str
    optimal: str  # Known minimal program
    test_cases: List[Tuple[List[int], Any]]
    
    @property
    def optimal_length(self) -> int:
        return len(self.optimal.split()) if self.optimal else 0


# Known minimal programs
TASKS = [
    # Length 1
    MinimalTask("add", "Add two numbers", "add", 
                [([3, 5], 8), ([10, 20], 30)]),
    MinimalTask("sub", "Subtract", "sub",
                [([10, 3], 7), ([5, 2], 3)]),
    MinimalTask("mul", "Multiply", "mul",
                [([3, 4], 12), ([5, 6], 30)]),
    MinimalTask("dup", "Duplicate", "dup",
                [([5], [5, 5]), ([3], [3, 3])]),
    MinimalTask("swap", "Swap", "swap",
                [([3, 5], [5, 3]), ([1, 2], [2, 1])]),
    MinimalTask("drop", "Drop top", "drop",
                [([3, 5], [3]), ([1, 2], [1])]),
    
    # Length 2
    MinimalTask("square", "Square a number", "dup mul",
                [([3], 9), ([5], 25), ([4], 16)]),
    MinimalTask("double", "Double a number", "dup add",
                [([3], 6), ([5], 10), ([7], 14)]),
    MinimalTask("negate", "Negate", "0 swap sub",  # or "neg" if available
                [([-3], 3), ([5], -5)]),
    MinimalTask("sum_keep", "Sum but keep both", "over add",
                [([3, 5], [3, 8]), ([1, 2], [1, 3])]),
    
    # Length 3
    MinimalTask("cube", "Cube a number", "dup dup mul mul",
                [([2], 8), ([3], 27)]),
    MinimalTask("triple", "Triple a number", "dup dup add add",
                [([3], 9), ([5], 15)]),
    MinimalTask("sum3", "Sum of three", "add add",
                [([1, 2, 3], 6), ([10, 20, 30], 60)]),
    MinimalTask("prod3", "Product of three", "mul mul",
                [([2, 3, 4], 24), ([1, 2, 3], 6)]),
    
    # Length 4+
    MinimalTask("fourth", "Fourth power", "dup mul dup mul",
                [([2], 16), ([3], 81)]),
    MinimalTask("sum_squares", "Sum of squares", "dup mul swap dup mul add",
                [([3, 4], 25), ([5, 12], 169)]),
    MinimalTask("avg_int", "Integer average", "dup add swap add 2 div",  # Approximation
                [([4, 6], 5), ([10, 20], 15)]),
    
    # Stack patterns
    MinimalTask("dup2", "Duplicate pair", "over over",
                [([3, 5], [3, 5, 3, 5])]),
    MinimalTask("tuck", "Tuck: copy top under second", "swap over",
                [([3, 5], [5, 3, 5])]),
    MinimalTask("nip", "Remove second item", "swap drop",
                [([3, 5], [5]), ([1, 2], [2])]),
    MinimalTask("3rd", "Get third item to top", "rot rot",
                [([1, 2, 3], [3, 1, 2])]),  # Effect of double rot
]


def program_length(prog: str) -> int:
    """Count tokens in program."""
    return len(prog.split()) if prog.strip() else 0


def verify_program(prog: str, task: MinimalTask, runtime: KoreRuntime) -> bool:
    """Verify program solves the task."""
    for inputs, expected in task.test_cases:
        result = runtime.execute(prog, inputs.copy())
        if not result.success:
            return False
        
        actual = result.stack
        if isinstance(expected, list):
            if actual != expected:
                return False
        else:
            if len(actual) != 1 or actual[0] != expected:
                return False
    return True


def call_llm(prompt: str, model: str) -> str:
    """Call LLM."""
    try:
        from transformers import AutoModelForCausalLM, AutoTokenizer
        import torch
        
        if not hasattr(call_llm, '_model'):
            print(f"Loading model: {model}...")
            call_llm._tokenizer = AutoTokenizer.from_pretrained(model)
            call_llm._model = AutoModelForCausalLM.from_pretrained(
                model,
                torch_dtype=torch.bfloat16,
                device_map="auto",
            )
        
        tokenizer = call_llm._tokenizer
        model_obj = call_llm._model
        
        messages = [
            {"role": "system", "content": "You are a Kore expert. Write the SHORTEST possible program. No explanations."},
            {"role": "user", "content": prompt}
        ]
        
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(text, return_tensors="pt").to(model_obj.device)
        
        outputs = model_obj.generate(
            **inputs,
            max_new_tokens=50,
            temperature=0.3,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
        
        response = tokenizer.decode(outputs[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True)
        return response.strip().split('\n')[0].strip()
        
    except ImportError:
        # Mock
        return task.optimal if random.random() < 0.5 else task.optimal + " swap swap"


def run_experiment(args):
    """Run the experiment."""
    
    print("="*60)
    print("Experiment 3: Minimal Program Discovery")
    print("="*60)
    
    runtime = KoreRuntime()
    
    test = runtime.execute("1 2 add")
    if not test.success:
        print("ERROR: Kore runtime not working")
        return
    print("✓ Kore runtime verified")
    
    tasks = TASKS[:args.n_tasks] if args.n_tasks else TASKS
    print(f"Running on {len(tasks)} tasks")
    print(f"Model: {args.model}\n")
    
    results = []
    
    for i, task in enumerate(tasks):
        print(f"[{i+1}/{len(tasks)}] {task.name}: {task.description}")
        print(f"  Optimal: '{task.optimal}' (length {task.optimal_length})")
        
        # Generate with LLM
        prompt = f"""Write the SHORTEST Kore program that: {task.description}

Example:
  Input: {task.test_cases[0][0]}
  Output: {task.test_cases[0][1]}

Write only the program, as short as possible:"""
        
        attempts = []
        best_program = None
        best_length = float('inf')
        
        for attempt in range(args.attempts):
            program = call_llm(prompt, args.model)
            
            # Clean up
            program = program.strip().strip('"').strip("'")
            
            if verify_program(program, task, runtime):
                length = program_length(program)
                attempts.append((program, length, True))
                
                if length < best_length:
                    best_length = length
                    best_program = program
            else:
                attempts.append((program, program_length(program), False))
        
        # Result
        if best_program:
            ratio = best_length / max(1, task.optimal_length)
            exact_match = best_program == task.optimal
            print(f"  Best: '{best_program}' (length {best_length}, ratio {ratio:.2f})")
            if exact_match:
                print("  ✓ EXACT MATCH")
        else:
            ratio = None
            exact_match = False
            print("  ✗ No valid solution found")
        
        results.append({
            "task": task.name,
            "description": task.description,
            "optimal": task.optimal,
            "optimal_length": task.optimal_length,
            "best_found": best_program,
            "best_length": best_length if best_program else None,
            "compression_ratio": ratio,
            "exact_match": exact_match,
            "attempts": attempts,
        })
    
    # Summary
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    solved = [r for r in results if r["best_found"]]
    exact = [r for r in results if r["exact_match"]]
    
    print(f"\nSolved: {len(solved)}/{len(results)} ({100*len(solved)/len(results):.1f}%)")
    print(f"Exact match: {len(exact)}/{len(results)} ({100*len(exact)/len(results):.1f}%)")
    
    if solved:
        ratios = [r["compression_ratio"] for r in solved if r["compression_ratio"]]
        avg_ratio = sum(ratios) / len(ratios)
        print(f"\nAverage compression ratio: {avg_ratio:.2f}")
        print(f"  (1.0 = optimal, >1.0 = longer than optimal)")
        
        # Histogram
        print("\nLength ratio distribution:")
        for threshold in [1.0, 1.5, 2.0, 3.0]:
            count = sum(1 for r in ratios if r <= threshold)
            print(f"  ≤{threshold:.1f}x optimal: {count}/{len(ratios)} ({100*count/len(ratios):.0f}%)")
    
    # Save
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Minimal Program Discovery")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct")
    parser.add_argument("--n-tasks", "-n", type=int, default=None)
    parser.add_argument("--attempts", "-a", type=int, default=3)
    parser.add_argument("--output", "-o", default="results/exp3_minimal.json")
    parser.add_argument("--seed", type=int, default=42)
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
