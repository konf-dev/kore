#!/usr/bin/env python3
"""
Experiment 5: Trace-to-Program Inversion

Hypothesis: LLMs can reconstruct programs from execution traces,
demonstrating causal reasoning about computation.

This is a novel task - no existing benchmark tests this.

Setup:
- Generate 200 execution traces
- LLM reconstructs the program from trace only
- Verify reconstructed program has same behavior

Metrics:
- Exact match rate
- Semantic equivalence rate
- Difficulty analysis by trace length
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
class TraceExample:
    """An execution trace for reconstruction."""
    program: str  # Hidden from LLM (ground truth)
    trace: str    # What LLM sees
    input_stack: List[int]
    output_stack: List[Any]


# =============================================================================
# Trace Generation
# =============================================================================

# Programs to use
PROGRAMS = [
    # Simple
    "add",
    "sub",
    "mul",
    "dup",
    "swap",
    "drop",
    "over",
    
    # Two ops
    "dup add",
    "dup mul",
    "swap sub",
    "swap drop",
    "over add",
    "dup drop",
    
    # Three ops
    "dup dup add add",
    "dup dup mul mul",
    "swap dup add",
    "over over add",
    "rot add add",
    
    # Four+ ops
    "dup mul dup mul",
    "over over add swap sub",
    "dup dup dup add add add",
]


def format_trace(program: str, inputs: List[int], runtime: KoreRuntime) -> Optional[str]:
    """Generate formatted execution trace."""
    
    result = runtime.execute(program, inputs.copy(), trace=True)
    
    if not result.success:
        return None
    
    # Format step-by-step
    lines = [f"Initial stack: {inputs}"]
    
    if result.trace:
        for line in result.trace:
            lines.append(f"  → {line}")
    
    lines.append(f"Final stack: {result.stack}")
    
    return "\n".join(lines)


def generate_traces(n: int, runtime: KoreRuntime) -> List[TraceExample]:
    """Generate trace examples."""
    
    examples = []
    
    for program in PROGRAMS:
        # Determine input count from effect
        effect = runtime.get_effect(program)
        if effect is None:
            continue
        
        n_inputs = max(effect.consumes, 2)
        
        # Generate multiple traces per program
        for _ in range(3):
            inputs = [random.randint(1, 20) for _ in range(n_inputs)]
            trace = format_trace(program, inputs, runtime)
            
            if trace:
                result = runtime.execute(program, inputs.copy())
                examples.append(TraceExample(
                    program=program,
                    trace=trace,
                    input_stack=inputs,
                    output_stack=result.stack,
                ))
    
    random.shuffle(examples)
    return examples[:n]


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
            {"role": "system", "content": "You are a programming expert. Analyze traces carefully."},
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
        return random.choice(PROGRAMS)


def check_equivalence(prog_a: str, prog_b: str, runtime: KoreRuntime, n_tests: int = 5) -> bool:
    """Check if two programs are semantically equivalent."""
    
    if not prog_a.strip() or not prog_b.strip():
        return prog_a.strip() == prog_b.strip()
    
    effect_a = runtime.get_effect(prog_a)
    effect_b = runtime.get_effect(prog_b)
    
    if effect_a is None or effect_b is None:
        return False
    
    if not effect_a.matches(effect_b):
        return False
    
    n_inputs = max(effect_a.consumes, 2)
    
    for _ in range(n_tests):
        inputs = [random.randint(1, 20) for _ in range(n_inputs)]
        result_a = runtime.execute(prog_a, inputs.copy())
        result_b = runtime.execute(prog_b, inputs.copy())
        
        if result_a.success != result_b.success:
            return False
        if result_a.stack != result_b.stack:
            return False
    
    return True


# =============================================================================
# Experiment
# =============================================================================

def run_experiment(args):
    """Run the experiment."""
    
    print("="*60)
    print("Experiment 5: Trace-to-Program Inversion")
    print("="*60)
    
    runtime = KoreRuntime()
    
    # Verify
    test = runtime.execute("1 2 add")
    if not test.success:
        print("ERROR: Kore runtime not working")
        return
    print("✓ Kore runtime verified")
    
    # Generate traces
    print(f"\nGenerating {args.n_traces} traces...")
    examples = generate_traces(args.n_traces, runtime)
    print(f"Generated {len(examples)} trace examples")
    print(f"Model: {args.model}\n")
    
    results = []
    
    for i, example in enumerate(examples):
        if i % 20 == 0:
            print(f"Progress: {i}/{len(examples)}")
        
        prompt = f"""Given this execution trace of a stack-based program, 
reconstruct the original program.

Available operations: add, sub, mul, div, dup, drop, swap, over, rot

{example.trace}

What program produced this trace? Write only the program:"""
        
        response = call_llm(prompt, args.model)
        
        # Clean up response
        predicted = response.strip().strip('"').strip("'")
        
        # Check results
        exact_match = predicted == example.program
        
        # Check semantic equivalence
        try:
            semantic_match = check_equivalence(predicted, example.program, runtime)
        except:
            semantic_match = False
        
        # Check if it produces same output
        try:
            result = runtime.execute(predicted, example.input_stack.copy())
            output_match = result.success and result.stack == example.output_stack
        except:
            output_match = False
        
        results.append({
            "ground_truth": example.program,
            "predicted": predicted,
            "trace": example.trace,
            "input": example.input_stack,
            "expected_output": example.output_stack,
            "exact_match": exact_match,
            "semantic_match": semantic_match,
            "output_match": output_match,
            "trace_lines": len(example.trace.split('\n')),
        })
        
        if args.verbose and i < 10:
            print(f"\n--- Example {i+1} ---")
            print(f"True: {example.program}")
            print(f"Pred: {predicted}")
            print(f"Exact: {'✓' if exact_match else '✗'}, Semantic: {'✓' if semantic_match else '✗'}")
    
    # Summary
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    n = len(results)
    exact = sum(1 for r in results if r["exact_match"])
    semantic = sum(1 for r in results if r["semantic_match"])
    output = sum(1 for r in results if r["output_match"])
    
    print(f"\nExact match: {exact}/{n} ({100*exact/n:.1f}%)")
    print(f"Semantic equivalence: {semantic}/{n} ({100*semantic/n:.1f}%)")
    print(f"Correct output: {output}/{n} ({100*output/n:.1f}%)")
    
    # By program complexity
    print("\nBy original program length:")
    for length in range(1, 5):
        subset = [r for r in results if len(r["ground_truth"].split()) == length]
        if subset:
            sem_acc = sum(1 for r in subset if r["semantic_match"]) / len(subset)
            print(f"  {length} ops: {100*sem_acc:.1f}% semantic ({len(subset)} examples)")
    
    # By trace length
    print("\nBy trace length:")
    for min_lines, max_lines in [(0, 4), (4, 6), (6, 10), (10, 100)]:
        subset = [r for r in results if min_lines <= r["trace_lines"] < max_lines]
        if subset:
            sem_acc = sum(1 for r in subset if r["semantic_match"]) / len(subset)
            print(f"  {min_lines}-{max_lines} lines: {100*sem_acc:.1f}% ({len(subset)} examples)")
    
    # Common confusions
    print("\nCommon confusions:")
    confusions = {}
    for r in results:
        if not r["semantic_match"]:
            key = (r["ground_truth"], r["predicted"])
            confusions[key] = confusions.get(key, 0) + 1
    
    for (true, pred), count in sorted(confusions.items(), key=lambda x: -x[1])[:5]:
        print(f"  '{true}' → '{pred}': {count} times")
    
    # Save
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Trace-to-Program Inversion")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct")
    parser.add_argument("--n-traces", "-n", type=int, default=100)
    parser.add_argument("--output", "-o", default="results/exp5_trace_inversion.json")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--verbose", "-v", action="store_true")
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
