#!/usr/bin/env python3
"""
Experiment 4: Effect Signature Prediction

Hypothesis: LLMs can predict program effects (abstract interpretation)
without executing the program.

This is the MOST NOVEL experiment - tests if LLMs understand 
computational semantics at an abstract level.

Setup:
- 500 Kore programs of varying complexity
- LLM predicts effect signature (consumes, produces)
- Compare to ground truth from static analysis

Metrics:
- Exact accuracy
- Off-by-one accuracy
- Error analysis by operation type
"""

import argparse
import json
import random
from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Tuple, Optional
import re
import sys

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "training"))

from kore_runtime import KoreRuntime, EffectSignature


# =============================================================================
# Program Generation
# =============================================================================

# Basic operations with their effects
PRIMITIVES = {
    # Stack effects: (consumes, produces)
    "add": (2, 1),
    "sub": (2, 1),
    "mul": (2, 1),
    "div": (2, 1),
    "mod": (2, 1),
    "dup": (1, 2),
    "drop": (1, 0),
    "swap": (2, 2),
    "over": (2, 3),
    "rot": (3, 3),
    "eq": (2, 1),
    "lt": (2, 1),
    "gt": (2, 1),
    "not": (1, 1),
    "and": (2, 1),
    "or": (2, 1),
    "neg": (1, 1),
}


def compute_effect(program: str) -> Optional[EffectSignature]:
    """Compute effect of a program statically."""
    tokens = program.split()
    
    # Track net stack change
    # For sequence: effect is composition
    min_depth = 0  # Minimum stack depth needed
    current_depth = 0  # Current stack depth
    
    for token in tokens:
        if token in PRIMITIVES:
            consumes, produces = PRIMITIVES[token]
            current_depth -= consumes
            if current_depth < min_depth:
                min_depth = current_depth
            current_depth += produces
        elif token.lstrip('-').isdigit():
            # Literal pushes 1
            current_depth += 1
        else:
            # Unknown token
            return None
    
    # Final effect
    total_consumes = -min_depth  # How much we needed from stack
    total_produces = current_depth - min_depth  # How much we leave
    
    return EffectSignature(total_consumes, total_produces)


def generate_programs(n: int, max_length: int = 6) -> List[Tuple[str, EffectSignature]]:
    """Generate random programs with known effects."""
    
    programs = []
    
    # Single primitives
    for op, (c, p) in PRIMITIVES.items():
        programs.append((op, EffectSignature(c, p)))
    
    # Two-operation sequences
    ops = list(PRIMITIVES.keys())
    for _ in range(n // 3):
        op1, op2 = random.sample(ops, 2)
        prog = f"{op1} {op2}"
        effect = compute_effect(prog)
        if effect:
            programs.append((prog, effect))
    
    # Three+ operation sequences
    for length in range(3, max_length + 1):
        for _ in range(n // 4):
            seq = random.choices(ops, k=length)
            prog = " ".join(seq)
            effect = compute_effect(prog)
            if effect:
                programs.append((prog, effect))
    
    # With literals
    for _ in range(n // 4):
        literal = random.randint(1, 10)
        op = random.choice(ops)
        prog = f"{literal} {op}"
        effect = compute_effect(prog)
        if effect:
            programs.append((prog, effect))
    
    # Deduplicate and sample
    seen = set()
    unique = []
    for prog, effect in programs:
        if prog not in seen:
            seen.add(prog)
            unique.append((prog, effect))
    
    random.shuffle(unique)
    return unique[:n]


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
            {"role": "system", "content": "You are a stack-based programming expert. Answer precisely."},
            {"role": "user", "content": prompt}
        ]
        
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(text, return_tensors="pt").to(model_obj.device)
        
        outputs = model_obj.generate(
            **inputs,
            max_new_tokens=30,
            temperature=0.1,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
        
        response = tokenizer.decode(outputs[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True)
        return response.strip()
        
    except ImportError:
        # Mock for testing
        return f"({random.randint(0,3)} -- {random.randint(0,3)})"


def parse_effect(response: str) -> Optional[EffectSignature]:
    """Parse effect from LLM response."""
    
    # Match patterns like "(2 -- 1)" or "2 -- 1" or "consumes 2, produces 1"
    patterns = [
        r'\((\d+)\s*--\s*(\d+)\)',  # (2 -- 1)
        r'(\d+)\s*--\s*(\d+)',       # 2 -- 1
        r'consumes?\s*:?\s*(\d+).*produces?\s*:?\s*(\d+)',  # consumes 2, produces 1
        r'(\d+)\s*→\s*(\d+)',        # 2 → 1
    ]
    
    for pattern in patterns:
        match = re.search(pattern, response, re.IGNORECASE)
        if match:
            return EffectSignature(int(match.group(1)), int(match.group(2)))
    
    return None


# =============================================================================
# Experiment
# =============================================================================

def run_experiment(args):
    """Run the experiment."""
    
    print("="*60)
    print("Experiment 4: Effect Signature Prediction")
    print("="*60)
    
    runtime = KoreRuntime()
    
    # Verify runtime
    test = runtime.execute("1 2 add")
    if not test.success:
        print("ERROR: Kore runtime not working")
        return
    print("✓ Kore runtime verified")
    
    # Generate programs
    print(f"\nGenerating {args.n_programs} programs...")
    programs = generate_programs(args.n_programs)
    print(f"Generated {len(programs)} unique programs")
    print(f"Model: {args.model}\n")
    
    # Provide reference for primitives
    primitive_ref = "\n".join(f"  {op}: ({c} -- {p})" 
                               for op, (c, p) in list(PRIMITIVES.items())[:10])
    
    results = []
    
    for i, (program, true_effect) in enumerate(programs):
        if i % 50 == 0:
            print(f"Progress: {i}/{len(programs)}")
        
        prompt = f"""In a stack-based language, predict the effect of this program.
Effect format: (inputs_consumed -- outputs_produced)

Primitive effects:
{primitive_ref}

Program: {program}

What is the effect? Answer in format (N -- M):"""
        
        response = call_llm(prompt, args.model)
        predicted = parse_effect(response)
        
        if predicted:
            exact_match = predicted.matches(true_effect)
            consumes_correct = predicted.consumes == true_effect.consumes
            produces_correct = predicted.produces == true_effect.produces
            off_by_one = (
                abs(predicted.consumes - true_effect.consumes) <= 1 and
                abs(predicted.produces - true_effect.produces) <= 1
            )
        else:
            exact_match = False
            consumes_correct = False
            produces_correct = False
            off_by_one = False
        
        results.append({
            "program": program,
            "true_effect": true_effect.to_string(),
            "predicted_effect": predicted.to_string() if predicted else None,
            "response": response,
            "exact_match": exact_match,
            "consumes_correct": consumes_correct,
            "produces_correct": produces_correct,
            "off_by_one": off_by_one,
            "program_length": len(program.split()),
        })
    
    # Summary
    print("\n" + "="*60)
    print("RESULTS SUMMARY")
    print("="*60)
    
    n = len(results)
    exact = sum(1 for r in results if r["exact_match"])
    consumes_ok = sum(1 for r in results if r["consumes_correct"])
    produces_ok = sum(1 for r in results if r["produces_correct"])
    off_by_one = sum(1 for r in results if r["off_by_one"])
    parsed = sum(1 for r in results if r["predicted_effect"])
    
    print(f"\nParseable responses: {parsed}/{n} ({100*parsed/n:.1f}%)")
    print(f"Exact accuracy: {exact}/{n} ({100*exact/n:.1f}%)")
    print(f"Consumes correct: {consumes_ok}/{n} ({100*consumes_ok/n:.1f}%)")
    print(f"Produces correct: {produces_ok}/{n} ({100*produces_ok/n:.1f}%)")
    print(f"Off-by-one: {off_by_one}/{n} ({100*off_by_one/n:.1f}%)")
    
    # By program length
    print("\nAccuracy by program length:")
    for length in range(1, 7):
        subset = [r for r in results if r["program_length"] == length]
        if subset:
            acc = sum(1 for r in subset if r["exact_match"]) / len(subset)
            print(f"  Length {length}: {100*acc:.1f}% ({len(subset)} programs)")
    
    # Error analysis
    print("\nCommon errors:")
    errors = [r for r in results if not r["exact_match"] and r["predicted_effect"]]
    error_types = {}
    for r in errors:
        prog = r["program"]
        first_op = prog.split()[0] if prog.split() else "unknown"
        error_types[first_op] = error_types.get(first_op, 0) + 1
    
    for op, count in sorted(error_types.items(), key=lambda x: -x[1])[:5]:
        print(f"  {op}: {count} errors")
    
    # Save
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(results, f, indent=2, default=str)
    print(f"\nResults saved to {output_path}")


def main():
    parser = argparse.ArgumentParser(description="Effect Signature Prediction")
    parser.add_argument("--model", "-m", default="Qwen/Qwen2.5-Coder-3B-Instruct")
    parser.add_argument("--n-programs", "-n", type=int, default=200)
    parser.add_argument("--output", "-o", default="results/exp4_effect_prediction.json")
    parser.add_argument("--seed", type=int, default=42)
    
    args = parser.parse_args()
    random.seed(args.seed)
    
    run_experiment(args)


if __name__ == "__main__":
    main()
