#!/usr/bin/env python3
"""
Experiment 2: Zero-Shot Equivalence Detection

Hypothesis: LLMs can detect semantic equivalence between programs
when given execution traces, without any fine-tuning.

Setup:
- 200 program pairs (100 equivalent, 100 not equivalent)
- Condition A: LLM sees only source code
- Condition B: LLM sees source code + execution traces
- Condition C: LLM sees only execution traces (no source)

Metrics:
- Accuracy, Precision, Recall, F1
- Confidence calibration
"""

import argparse
import json
import random
from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Tuple, Optional
import sys

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "training"))

from kore_runtime import KoreRuntime, EffectSignature


@dataclass
class ProgramPair:
    """A pair of programs to compare."""
    program_a: str
    program_b: str
    equivalent: bool  # Ground truth
    category: str  # e.g., "arithmetic", "stack", "semantic"


# =============================================================================
# Generate Program Pairs
# =============================================================================

EQUIVALENT_PAIRS = [
    # Algebraic equivalences
    ("dup add", "2 mul", "double"),
    ("dup mul", "dup dup mul mul drop", "square_verbose"),  # Both square
    ("dup dup mul mul", "dup mul dup mul", "fourth_power"),
    ("0 add", "", "identity"),
    ("1 mul", "", "identity_mul"),
    ("swap swap", "", "double_swap"),
    
    # Commutative
    ("add", "swap add", "commutative_add"),
    ("mul", "swap mul", "commutative_mul"),
    
    # Associative patterns
    ("add add", "rot add add", "associative"),
    
    # Stack manipulation
    ("dup drop", "", "dup_drop"),
    ("over drop", "swap drop swap", "over_equiv"),
    
    # Different implementations
    ("dup dup add add", "3 mul", "triple"),
    ("dup dup mul mul", "4 pow", "power4"),  # If pow exists
]

NON_EQUIVALENT_PAIRS = [
    # Similar looking but different
    ("add", "sub", "add_vs_sub"),
    ("add", "mul", "add_vs_mul"),
    ("dup add", "dup mul", "double_vs_square"),
    ("swap", "dup", "swap_vs_dup"),
    ("drop", "dup drop drop", "drop_one_vs_two"),
    
    # Off by one
    ("dup add", "dup add 1 add", "double_vs_double_plus_one"),
    ("dup mul", "dup mul 1 add", "square_vs_square_plus_one"),
    
    # Different effects
    ("add", "dup add drop", "effect_differs"),
    ("swap", "rot", "swap_vs_rot"),
    
    # Order matters
    ("sub", "swap sub", "sub_order"),
    ("div", "swap div", "div_order"),
]


def check_equivalence(prog_a: str, prog_b: str, runtime: KoreRuntime, n_tests: int = 10) -> bool:
    """Check if two programs are semantically equivalent."""
    
    # Get effects
    effect_a = runtime.get_effect(prog_a) if prog_a else EffectSignature(0, 0)
    effect_b = runtime.get_effect(prog_b) if prog_b else EffectSignature(0, 0)
    
    if effect_a is None or effect_b is None:
        return False
    
    # Empty program special case
    if not prog_a.strip() and not prog_b.strip():
        return True
    
    # Effect must match
    if not effect_a.matches(effect_b):
        return False
    
    # Test on random inputs
    n_inputs = max(effect_a.consumes, effect_b.consumes) if effect_a.consumes else 1
    
    for _ in range(n_tests):
        inputs = [random.randint(-50, 50) for _ in range(n_inputs)]
        
        result_a = runtime.execute(prog_a, inputs.copy()) if prog_a else type('R', (), {'success': True, 'stack': inputs})()
        result_b = runtime.execute(prog_b, inputs.copy()) if prog_b else type('R', (), {'success': True, 'stack': inputs})()
        
        if result_a.success != result_b.success:
            return False
        if result_a.success and result_a.stack != result_b.stack:
            return False
    
    return True


def generate_pairs(runtime: KoreRuntime, n_equivalent: int = 100, n_non_equivalent: int = 100) -> List[ProgramPair]:
    """Generate program pairs for the experiment."""
    pairs = []
    
    # Use known equivalences
    for prog_a, prog_b, category in EQUIVALENT_PAIRS:
        if check_equivalence(prog_a, prog_b, runtime):
            pairs.append(ProgramPair(prog_a, prog_b, True, f"equiv_{category}"))
    
    for prog_a, prog_b, category in NON_EQUIVALENT_PAIRS:
        pairs.append(ProgramPair(prog_a, prog_b, False, f"non_{category}"))
    
    # Generate more random pairs
    programs = [
        "add", "sub", "mul", "dup", "swap", "drop", "over", "rot",
        "dup add", "dup mul", "swap add", "swap sub",
        "dup dup add add", "dup dup mul mul",
        "over add", "rot add",
    ]
    
    while len([p for p in pairs if p.equivalent]) < n_equivalent:
        prog = random.choice(programs)
        # Create equivalent variant
        variants = [
            (prog, f"{prog} 0 add"),  # Add identity
            (prog, f"{prog} 1 mul"),  # Mul identity
            (prog, f"{prog} swap swap"),  # Double swap
        ]
        for a, b in variants:
            if check_equivalence(a, b, runtime):
                pairs.append(ProgramPair(a, b, True, "generated_equiv"))
                break
    
    while len([p for p in pairs if not p.equivalent]) < n_non_equivalent:
        a, b = random.sample(programs, 2)
        if not check_equivalence(a, b, runtime):
            pairs.append(ProgramPair(a, b, False, "generated_non"))
    
    # Balance
    equiv = [p for p in pairs if p.equivalent][:n_equivalent]
    non_equiv = [p for p in pairs if not p.equivalent][:n_non_equivalent]
    
    result = equiv + non_equiv
    random.shuffle(result)
    return result


# =============================================================================
# Get Execution Traces
# =============================================================================

def get_trace(program: str, inputs: List[int], runtime: KoreRuntime) -> str:
    """Get formatted execution trace."""
    if not program.strip():
        return f"Input: {inputs}\nNo operations\nOutput: {inputs}"
    
    result = runtime.execute(program, inputs.copy(), trace=True)
    
    if not result.success:
        return f"Input: {inputs}\nError: {result.error}"
    
    trace_lines = [f"Input: {inputs}"]
    if result.trace:
        for line in result.trace[:20]:  # Limit trace length
            trace_lines.append(f"  {line}")
    trace_lines.append(f"Output: {result.stack}")
    
    return "\n".join(trace_lines)


# =============================================================================
# LLM Prompts
# =============================================================================

def prompt_code_only(pair: ProgramPair) -> str:
    """Prompt with only source code."""
    return f"""Are these two Kore programs semantically equivalent?
(They produce the same output for all possible inputs)

Program A: {pair.program_a if pair.program_a else "(empty - identity)"}
Program B: {pair.program_b if pair.program_b else "(empty - identity)"}

Answer only "EQUIVALENT" or "NOT EQUIVALENT":"""


def prompt_code_and_trace(pair: ProgramPair, traces_a: List[str], traces_b: List[str]) -> str:
    """Prompt with code and execution traces."""
    return f"""Are these two Kore programs semantically equivalent?

Program A: {pair.program_a if pair.program_a else "(empty)"}
Program B: {pair.program_b if pair.program_b else "(empty)"}

Execution traces for Program A:
{chr(10).join(traces_a[:3])}

Execution traces for Program B:
{chr(10).join(traces_b[:3])}

Based on the traces, answer only "EQUIVALENT" or "NOT EQUIVALENT":"""


def prompt_trace_only(traces_a: List[str], traces_b: List[str]) -> str:
    """Prompt with only execution traces (no source code)."""
    return f"""Two unknown programs were executed on the same inputs.
Are they semantically equivalent (same outputs for all inputs)?

Program A traces:
{chr(10).join(traces_a[:3])}

Program B traces:
{chr(10).join(traces_b[:3])}

Answer only "EQUIVALENT" or "NOT EQUIVALENT":"""


# =============================================================================
# LLM Interface
# =============================================================================

def call_llm(prompt: str, model: str) -> str:
    """Call LLM and get response."""
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
            print("Model loaded!")
        
        tokenizer = call_llm._tokenizer
        model_obj = call_llm._model
        
        messages = [
            {"role": "system", "content": "You are a program analysis expert. Be precise and concise."},
            {"role": "user", "content": prompt}
        ]
        
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(text, return_tensors="pt").to(model_obj.device)
        
        outputs = model_obj.generate(
            **inputs,
            max_new_tokens=20,
            temperature=0.1,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
        
        response = tokenizer.decode(outputs[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True)
        return response.strip()
        
    except ImportError:
        # Mock for testing
        return random.choice(["EQUIVALENT", "NOT EQUIVALENT"])


def parse_response(response: str) -> Optional[bool]:
    """Parse LLM response to boolean."""
    response = response.upper().strip()
    if "NOT EQUIVALENT" in response or "NOT_EQUIVALENT" in response:
        return False
    if "EQUIVALENT" in response:
        return True
    return None


# =============================================================================
# Experiment
# =============================================================================

def run_experiment(args):
    """Run the full experiment."""
    
    print("="*60)
    print("Experiment 2: Zero-Shot Equivalence Detection")
    print("="*60)
    
    runtime = KoreRuntime()
    
    # Verify runtime
    test = runtime.execute("1 2 add")
    if not test.success:
        print(f"ERROR: Kore runtime not working")
        return
    print("✓ Kore runtime verified")
    
    # Generate pairs
    print(f"\nGenerating {args.n_pairs} program pairs...")
    pairs = generate_pairs(runtime, args.n_pairs // 2, args.n_pairs // 2)
    print(f"Generated {len(pairs)} pairs ({sum(1 for p in pairs if p.equivalent)} equivalent)")
    print(f"Model: {args.model}")
    
    results = {
        "condition_a": [],  # Code only
        "condition_b": [],  # Code + traces
        "condition_c": [],  # Traces only
    }
    
    for i, pair in enumerate(pairs):
        print(f"[{i+1}/{len(pairs)}] {pair.program_a} vs {pair.program_b} (GT: {'equiv' if pair.equivalent else 'diff'})")
        
        # Generate traces
        test_inputs = [[random.randint(1, 20) for _ in range(2)] for _ in range(3)]
        traces_a = [get_trace(pair.program_a, inp, runtime) for inp in test_inputs]
        traces_b = [get_trace(pair.program_b, inp, runtime) for inp in test_inputs]
        
        # Condition A: Code only
        prompt_a = prompt_code_only(pair)
        response_a = call_llm(prompt_a, args.model)
        pred_a = parse_response(response_a)
        
        # Condition B: Code + traces
        prompt_b = prompt_code_and_trace(pair, traces_a, traces_b)
        response_b = call_llm(prompt_b, args.model)
        pred_b = parse_response(response_b)
        
        # Condition C: Traces only
        prompt_c = prompt_trace_only(traces_a, traces_b)
        response_c = call_llm(prompt_c, args.model)
        pred_c = parse_response(response_c)
        
        results["condition_a"].append({
            "pair": (pair.program_a, pair.program_b),
            "ground_truth": pair.equivalent,
            "prediction": pred_a,
            "correct": pred_a == pair.equivalent,
            "response": response_a,
        })
        
        results["condition_b"].append({
            "pair": (pair.program_a, pair.program_b),
            "ground_truth": pair.equivalent,
            "prediction": pred_b,
            "correct": pred_b == pair.equivalent,
            "response": response_b,
        })
        
        results["condition_c"].append({
            "pair": (pair.program_a, pair.program_b),
            "ground_truth": pair.equivalent,
            "prediction": pred_c,
            "correct": pred_c == pair.equivalent,
            "response": response_c,
        })
        
        print(f"  Code only: {response_a[:30]} ({'✓' if pred_a == pair.equivalent else '✗'})")
        print(f"  Code+trace: {response_b[:30]} ({'✓' if pred_b == pair.equivalent else '✗'})")
        print(f"  Trace only: {response_c[:30]} ({'✓' if pred_c == pair.equivalent else '✗'})")
    
    # Compute metrics
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    for condition, name in [("condition_a", "Code Only"), ("condition_b", "Code + Traces"), ("condition_c", "Traces Only")]:
        correct = sum(1 for r in results[condition] if r["correct"])
        total = len(results[condition])
        
        # Precision/Recall for "equivalent" class
        tp = sum(1 for r in results[condition] if r["ground_truth"] and r["prediction"])
        fp = sum(1 for r in results[condition] if not r["ground_truth"] and r["prediction"])
        fn = sum(1 for r in results[condition] if r["ground_truth"] and not r["prediction"])
        
        precision = tp / max(1, tp + fp)
        recall = tp / max(1, tp + fn)
        f1 = 2 * precision * recall / max(0.001, precision + recall)
        
        print(f"\n{name}:")
        print(f"  Accuracy: {correct}/{total} ({100*correct/total:.1f}%)")
        print(f"  Precision: {precision:.3f}")
        print(f"  Recall: {recall:.3f}")
        print(f"  F1: {f1:.3f}")
    
    # Save results
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Zero-Shot Equivalence Detection")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct")
    parser.add_argument("--n-pairs", "-n", type=int, default=50)
    parser.add_argument("--output", "-o", default="results/exp2_equivalence.json")
    parser.add_argument("--seed", type=int, default=42)
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
