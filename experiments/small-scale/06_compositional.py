#!/usr/bin/env python3
"""
Experiment 6: Compositional Arithmetic

Hypothesis: LLMs can compose individually-learned operations
into novel combinations they haven't seen before.

This tests compositional generalization in a clean, minimal setting.

Setup:
- Train set: Simple programs (single ops, pairs)
- Test set: Compositions never seen together
- Zero-shot: No fine-tuning, just in-context examples

Metrics:
- Zero-shot success rate on novel compositions
- Accuracy by composition depth
"""

import argparse
import json
import random
from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Tuple, Set
import sys

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "training"))

from kore_runtime import KoreRuntime


# =============================================================================
# Task Definition
# =============================================================================

@dataclass
class CompositionTask:
    """A compositional task."""
    description: str
    target_program: str
    test_cases: List[Tuple[List[int], Any]]
    composition_depth: int
    seen_in_training: bool


# Atomic operations (training)
ATOMS = {
    "add": "Add two numbers",
    "sub": "Subtract (a - b)",
    "mul": "Multiply two numbers",
    "dup": "Duplicate top element",
    "swap": "Swap top two elements",
    "drop": "Remove top element",
}

# Simple pairs (training)
PAIRS = {
    "dup add": "Double a number (n * 2)",
    "dup mul": "Square a number (n²)",
    "swap sub": "Reverse subtract (b - a)",
    "dup drop": "No-op (identity)",
}

# Novel compositions (testing) - never seen these exact combinations
TEST_COMPOSITIONS = [
    # Depth 3
    ("dup dup add add", "Triple a number (n * 3)", 3),
    ("dup mul add", "n² + n", 3),
    ("over add mul", "(a + b) * b", 3),
    ("swap over add", "a + b, keeping b", 3),
    ("dup add mul", "2n * n = 2n²", 3),
    
    # Depth 4
    ("dup dup mul mul", "Fourth power (n⁴)", 4),
    ("dup mul dup add", "n² + n²  = 2n²", 4),
    ("over over add mul", "(a + b) * a", 4),
    ("dup add dup mul", "(2n)² = 4n²", 4),
    ("swap dup add swap", "2a, b", 4),
    
    # Depth 5
    ("dup dup dup add add add", "Quadruple (n * 4)", 5),
    ("dup mul dup mul add", "n⁴ + n²", 5),
    ("over dup mul add mul", "(a + b²) * b", 5),
    
    # Depth 6
    ("dup dup add add dup mul", "(3n)² = 9n²", 6),
    ("dup mul dup add swap drop", "n² + n² = 2n²", 6),
]


def generate_test_cases(program: str, runtime: KoreRuntime, n: int = 5) -> List[Tuple[List[int], Any]]:
    """Generate test cases for a program."""
    effect = runtime.get_effect(program)
    if effect is None:
        return []
    
    n_inputs = max(effect.consumes, 1)
    cases = []
    
    for _ in range(n):
        inputs = [random.randint(1, 10) for _ in range(n_inputs)]
        result = runtime.execute(program, inputs.copy())
        if result.success:
            output = result.stack[-1] if len(result.stack) == 1 else result.stack
            cases.append((inputs, output))
    
    return cases


# =============================================================================
# LLM Interface
# =============================================================================

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
            {"role": "system", "content": "You are a Kore expert. Compose operations precisely."},
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
        return "dup add"


def verify_program(program: str, task: CompositionTask, runtime: KoreRuntime) -> bool:
    """Verify program solves the task."""
    for inputs, expected in task.test_cases:
        result = runtime.execute(program, inputs.copy())
        if not result.success:
            return False
        
        actual = result.stack[-1] if len(result.stack) == 1 else result.stack
        
        if isinstance(expected, list):
            if actual != expected:
                return False
        else:
            if actual != expected:
                return False
    return True


# =============================================================================
# Experiment
# =============================================================================

def run_experiment(args):
    """Run the experiment."""
    
    print("="*60)
    print("Experiment 6: Compositional Arithmetic")
    print("="*60)
    
    runtime = KoreRuntime()
    
    # Verify
    test = runtime.execute("1 2 add")
    if not test.success:
        print("ERROR: Kore runtime not working")
        return
    print("✓ Kore runtime verified")
    
    # Build training examples string
    training_examples = "Known operations:\n"
    for prog, desc in list(ATOMS.items()) + list(PAIRS.items()):
        training_examples += f"  {prog}: {desc}\n"
    
    # Generate test tasks
    tasks = []
    for prog, desc, depth in TEST_COMPOSITIONS:
        cases = generate_test_cases(prog, runtime)
        if cases:
            tasks.append(CompositionTask(
                description=desc,
                target_program=prog,
                test_cases=cases,
                composition_depth=depth,
                seen_in_training=False,
            ))
    
    print(f"\nTest tasks: {len(tasks)} novel compositions")
    print(f"Model: {args.model}\n")
    
    results = []
    
    for i, task in enumerate(tasks):
        print(f"[{i+1}/{len(tasks)}] {task.description} (depth {task.composition_depth})")
        
        # Create prompt with training examples
        prompt = f"""{training_examples}

Your task: {task.description}

Example:
  Input: {task.test_cases[0][0]}
  Expected output: {task.test_cases[0][1]}

Compose the known operations to solve this. Write only the program:"""
        
        response = call_llm(prompt, args.model)
        predicted = response.strip().strip('"').strip("'")
        
        # Check
        exact_match = predicted == task.target_program
        correct = verify_program(predicted, task, runtime)
        
        print(f"  Target: {task.target_program}")
        print(f"  Predicted: {predicted}")
        print(f"  Result: {'✓' if correct else '✗'} (exact: {'✓' if exact_match else '✗'})")
        
        results.append({
            "description": task.description,
            "target": task.target_program,
            "predicted": predicted,
            "exact_match": exact_match,
            "correct": correct,
            "depth": task.composition_depth,
        })
    
    # Summary
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    n = len(results)
    exact = sum(1 for r in results if r["exact_match"])
    correct = sum(1 for r in results if r["correct"])
    
    print(f"\nOverall:")
    print(f"  Exact match: {exact}/{n} ({100*exact/n:.1f}%)")
    print(f"  Correct (any solution): {correct}/{n} ({100*correct/n:.1f}%)")
    
    # By depth
    print("\nBy composition depth:")
    for depth in sorted(set(r["depth"] for r in results)):
        subset = [r for r in results if r["depth"] == depth]
        correct_d = sum(1 for r in subset if r["correct"])
        print(f"  Depth {depth}: {correct_d}/{len(subset)} ({100*correct_d/len(subset):.1f}%)")
    
    # Generalization gap
    print("\nKey insight:")
    print("  These compositions were NEVER shown during prompting.")
    print("  Success requires genuine compositional generalization.")
    
    # Save
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Compositional Arithmetic")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct")
    parser.add_argument("--output", "-o", default="results/exp6_compositional.json")
    parser.add_argument("--seed", type=int, default=42)
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
