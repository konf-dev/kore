#!/usr/bin/env python3
"""
Experiment 1: Effect-Guided Program Search

Hypothesis: Using effect signatures to filter candidate programs
improves synthesis success rate and reduces token usage.

Setup:
- Task: Generate programs for 100 synthesis tasks
- Condition A: LLM generates freely, we verify after
- Condition B: LLM generates, we filter by effect, then verify

Metrics:
- Success rate (correct programs)
- Tokens used per successful program
- Time to first correct solution
"""

import argparse
import json
import random
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
import sys

# Add parent paths
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "training"))

from kore_runtime import KoreRuntime, EffectSignature


@dataclass
class SynthesisTask:
    """A program synthesis task."""
    name: str
    description: str
    expected_effect: EffectSignature
    test_cases: List[Tuple[List[int], Any]]
    
    def verify(self, program: str, runtime: KoreRuntime) -> bool:
        """Check if program solves this task."""
        for inputs, expected in self.test_cases:
            result = runtime.execute(program, inputs.copy())
            if not result.success:
                return False
            actual = result.stack[-1] if len(result.stack) == 1 else result.stack
            if actual != expected:
                return False
        return True


# =============================================================================
# Task Bank
# =============================================================================

TASKS = [
    # Basic arithmetic (2 -- 1)
    SynthesisTask("add", "Add two numbers", EffectSignature(2, 1),
                  [([3, 5], 8), ([10, 20], 30), ([0, 7], 7)]),
    SynthesisTask("sub", "Subtract: a - b", EffectSignature(2, 1),
                  [([10, 3], 7), ([20, 5], 15), ([7, 7], 0)]),
    SynthesisTask("mul", "Multiply two numbers", EffectSignature(2, 1),
                  [([3, 4], 12), ([5, 6], 30), ([0, 10], 0)]),
    SynthesisTask("max", "Maximum of two numbers", EffectSignature(2, 1),
                  [([3, 5], 5), ([10, 2], 10), ([7, 7], 7)]),
    SynthesisTask("min", "Minimum of two numbers", EffectSignature(2, 1),
                  [([3, 5], 3), ([10, 2], 2), ([7, 7], 7)]),
    
    # Unary operations (1 -- 1)
    SynthesisTask("square", "Square a number (n²)", EffectSignature(1, 1),
                  [([3], 9), ([5], 25), ([0], 0), ([1], 1)]),
    SynthesisTask("double", "Double a number", EffectSignature(1, 1),
                  [([3], 6), ([5], 10), ([0], 0), ([7], 14)]),
    SynthesisTask("triple", "Triple a number", EffectSignature(1, 1),
                  [([3], 9), ([5], 15), ([0], 0), ([4], 12)]),
    SynthesisTask("negate", "Negate a number", EffectSignature(1, 1),
                  [([3], -3), ([0], 0), ([-5], 5)]),
    SynthesisTask("abs", "Absolute value", EffectSignature(1, 1),
                  [([3], 3), ([-5], 5), ([0], 0)]),
    
    # Stack manipulation (2 -- 2)
    SynthesisTask("swap", "Swap top two elements", EffectSignature(2, 2),
                  [([3, 5], [5, 3]), ([1, 2], [2, 1])]),
    
    # Duplication (1 -- 2)
    SynthesisTask("dup", "Duplicate top element", EffectSignature(1, 2),
                  [([3], [3, 3]), ([7], [7, 7])]),
    
    # Three-argument (3 -- 1)
    SynthesisTask("sum3", "Sum of three numbers", EffectSignature(3, 1),
                  [([1, 2, 3], 6), ([10, 20, 30], 60)]),
    SynthesisTask("mid", "Middle value of three", EffectSignature(3, 1),
                  [([1, 5, 3], 3), ([10, 2, 7], 7)]),
    
    # Compound (2 -- 1)
    SynthesisTask("sum_squares", "a² + b²", EffectSignature(2, 1),
                  [([3, 4], 25), ([5, 12], 169)]),
    SynthesisTask("diff_squares", "a² - b²", EffectSignature(2, 1),
                  [([5, 3], 16), ([4, 4], 0)]),
    SynthesisTask("avg", "Integer average (a+b)/2", EffectSignature(2, 1),
                  [([4, 6], 5), ([10, 20], 15)]),
]


def generate_more_tasks(n: int) -> List[SynthesisTask]:
    """Generate additional random tasks."""
    extra = []
    for i in range(n):
        # Random polynomial: ax + b
        a = random.randint(1, 5)
        b = random.randint(0, 10)
        extra.append(SynthesisTask(
            f"poly_{a}x+{b}",
            f"Compute {a}x + {b}",
            EffectSignature(1, 1),
            [([x], a * x + b) for x in random.sample(range(1, 20), 4)]
        ))
    return extra


# =============================================================================
# LLM Interface (minimal, no deps)
# =============================================================================

def call_llm(prompt: str, model: str, max_tokens: int = 128) -> str:
    """Call LLM via transformers."""
    try:
        from transformers import AutoModelForCausalLM, AutoTokenizer
        import torch
        
        # Cache model
        if not hasattr(call_llm, '_model'):
            print(f"Loading model: {model}...")
            call_llm._tokenizer = AutoTokenizer.from_pretrained(model)
            call_llm._model = AutoModelForCausalLM.from_pretrained(
                model,
                torch_dtype=torch.bfloat16,
                device_map="auto",
            )
            print("Model loaded!")
        
        tokenizer = call_llm._tokenizer
        model_obj = call_llm._model
        
        messages = [
            {"role": "system", "content": "You are a Kore programming expert. Write only the Kore program, no explanations."},
            {"role": "user", "content": prompt}
        ]
        
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(text, return_tensors="pt").to(model_obj.device)
        
        outputs = model_obj.generate(
            **inputs,
            max_new_tokens=max_tokens,
            temperature=0.7,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
        
        response = tokenizer.decode(outputs[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True)
        return response.strip().split('\n')[0].strip()
        
    except ImportError:
        print("Warning: transformers not installed, using mock responses")
        # Mock for testing without GPU
        mock_solutions = {
            "add": "add", "sub": "sub", "mul": "mul",
            "square": "dup mul", "double": "dup add",
            "swap": "swap", "dup": "dup",
        }
        for name, sol in mock_solutions.items():
            if name in prompt.lower():
                return sol
        return "dup add"  # fallback


# =============================================================================
# Experiment Conditions
# =============================================================================

def condition_a_baseline(
    task: SynthesisTask,
    runtime: KoreRuntime,
    model: str,
    max_attempts: int = 5,
) -> Dict[str, Any]:
    """
    Condition A: LLM generates freely, verify after.
    """
    prompt = f"""Write a Kore program that: {task.description}
Effect signature: {task.expected_effect.to_string()}
Example: {task.test_cases[0][0]} -> {task.test_cases[0][1]}

Write ONLY the Kore program (e.g., "dup mul" or "swap add"):"""
    
    start_time = time.time()
    attempts = []
    tokens_used = 0
    
    for attempt in range(max_attempts):
        program = call_llm(prompt, model)
        tokens_used += len(program.split())
        attempts.append(program)
        
        if task.verify(program, runtime):
            return {
                "success": True,
                "program": program,
                "attempts": len(attempts),
                "tokens_used": tokens_used,
                "time": time.time() - start_time,
                "all_attempts": attempts,
            }
    
    return {
        "success": False,
        "attempts": len(attempts),
        "tokens_used": tokens_used,
        "time": time.time() - start_time,
        "all_attempts": attempts,
    }


def condition_b_effect_guided(
    task: SynthesisTask,
    runtime: KoreRuntime,
    model: str,
    max_attempts: int = 5,
) -> Dict[str, Any]:
    """
    Condition B: LLM generates, filter by effect, then verify.
    
    Key difference: We reject programs with wrong effect BEFORE
    running test cases, and ask LLM to regenerate.
    """
    prompt_template = """Write a Kore program that: {description}
REQUIRED effect signature: {effect}
The program MUST consume {consumes} value(s) and produce {produces} value(s).
Example: {example_in} -> {example_out}

Write ONLY the Kore program:"""
    
    start_time = time.time()
    attempts = []
    effect_filtered = 0
    tokens_used = 0
    
    for attempt in range(max_attempts * 2):  # More attempts since we filter
        prompt = prompt_template.format(
            description=task.description,
            effect=task.expected_effect.to_string(),
            consumes=task.expected_effect.consumes,
            produces=task.expected_effect.produces,
            example_in=task.test_cases[0][0],
            example_out=task.test_cases[0][1],
        )
        
        # Add feedback from previous attempt
        if attempts:
            last = attempts[-1]
            last_effect = runtime.get_effect(last)
            if last_effect:
                prompt += f"\n\nPrevious attempt '{last}' had wrong effect {last_effect.to_string()}. Try again:"
        
        program = call_llm(prompt, model)
        tokens_used += len(program.split())
        
        # EFFECT FILTER: Check effect before running tests
        effect = runtime.get_effect(program)
        if effect is None or not effect.matches(task.expected_effect):
            effect_filtered += 1
            attempts.append(program)
            continue  # Don't even run tests
        
        attempts.append(program)
        
        if task.verify(program, runtime):
            return {
                "success": True,
                "program": program,
                "attempts": len(attempts),
                "effect_filtered": effect_filtered,
                "tokens_used": tokens_used,
                "time": time.time() - start_time,
                "all_attempts": attempts,
            }
        
        if len(attempts) >= max_attempts:
            break
    
    return {
        "success": False,
        "attempts": len(attempts),
        "effect_filtered": effect_filtered,
        "tokens_used": tokens_used,
        "time": time.time() - start_time,
        "all_attempts": attempts,
    }


# =============================================================================
# Main Experiment
# =============================================================================

def run_experiment(args):
    """Run the full experiment."""
    
    print("="*60)
    print("Experiment 1: Effect-Guided Program Search")
    print("="*60)
    
    runtime = KoreRuntime()
    
    # Verify runtime
    test = runtime.execute("1 2 add")
    if not test.success or test.stack != [3]:
        print(f"ERROR: Kore runtime not working: {test}")
        return
    print("✓ Kore runtime verified")
    
    # Prepare tasks
    tasks = TASKS.copy()
    if args.extra_tasks > 0:
        tasks.extend(generate_more_tasks(args.extra_tasks))
    
    if args.n_tasks:
        tasks = random.sample(tasks, min(args.n_tasks, len(tasks)))
    
    print(f"Running on {len(tasks)} tasks")
    print(f"Model: {args.model}")
    print()
    
    results = {
        "condition_a": [],
        "condition_b": [],
    }
    
    for i, task in enumerate(tasks):
        print(f"[{i+1}/{len(tasks)}] {task.name}: {task.description}")
        
        # Run both conditions
        result_a = condition_a_baseline(task, runtime, args.model, args.max_attempts)
        result_b = condition_b_effect_guided(task, runtime, args.model, args.max_attempts)
        
        results["condition_a"].append({"task": task.name, **result_a})
        results["condition_b"].append({"task": task.name, **result_b})
        
        status_a = "✓" if result_a["success"] else "✗"
        status_b = "✓" if result_b["success"] else "✗"
        print(f"  Baseline: {status_a} ({result_a['attempts']} attempts, {result_a['tokens_used']} tokens)")
        print(f"  Effect-guided: {status_b} ({result_b['attempts']} attempts, {result_b.get('effect_filtered', 0)} filtered)")
    
    # Summary
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    success_a = sum(1 for r in results["condition_a"] if r["success"])
    success_b = sum(1 for r in results["condition_b"] if r["success"])
    
    tokens_a = sum(r["tokens_used"] for r in results["condition_a"])
    tokens_b = sum(r["tokens_used"] for r in results["condition_b"])
    
    time_a = sum(r["time"] for r in results["condition_a"])
    time_b = sum(r["time"] for r in results["condition_b"])
    
    print(f"\nCondition A (Baseline):")
    print(f"  Success rate: {success_a}/{len(tasks)} ({100*success_a/len(tasks):.1f}%)")
    print(f"  Total tokens: {tokens_a}")
    print(f"  Total time: {time_a:.1f}s")
    
    print(f"\nCondition B (Effect-Guided):")
    print(f"  Success rate: {success_b}/{len(tasks)} ({100*success_b/len(tasks):.1f}%)")
    print(f"  Total tokens: {tokens_b}")
    print(f"  Total time: {time_b:.1f}s")
    print(f"  Effect filtered: {sum(r.get('effect_filtered', 0) for r in results['condition_b'])}")
    
    improvement = (success_b - success_a) / max(1, success_a) * 100
    print(f"\nImprovement: {improvement:+.1f}% success rate")
    
    # Save results
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Effect-Guided Program Search Experiment")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct",
                        help="HuggingFace model name")
    parser.add_argument("--n-tasks", "-n", type=int, default=None,
                        help="Number of tasks to run (default: all)")
    parser.add_argument("--extra-tasks", type=int, default=0,
                        help="Generate extra random polynomial tasks")
    parser.add_argument("--max-attempts", type=int, default=5,
                        help="Max attempts per task per condition")
    parser.add_argument("--output", "-o", default="results/exp1_effect_guided.json",
                        help="Output file for results")
    parser.add_argument("--seed", type=int, default=42)
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
