"""
Kore-RL Evaluation
==================

Evaluate trained models on:
1. KoreEval: Kore-specific benchmarks
2. HumanEval: Python coding (translated to Kore)
3. ARC-AGI: Abstract reasoning
"""

import os
import json
import time
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Any, Optional
import random

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from tqdm import tqdm

import sys
sys.path.append(str(Path(__file__).parent.parent / "training"))
from runtime_client import KoreRuntimeClient


@dataclass
class EvalResult:
    """Result of evaluating on a benchmark"""
    benchmark: str
    total: int
    correct: int
    accuracy: float
    avg_time_ms: float
    details: List[Dict[str, Any]]


class KoreEvaluator:
    """Evaluate models on Kore programming tasks"""
    
    def __init__(
        self,
        model_path: str,
        runtime_url: str = "http://localhost:8080",
        device: str = "cuda",
    ):
        self.tokenizer = AutoTokenizer.from_pretrained(model_path)
        self.model = AutoModelForCausalLM.from_pretrained(
            model_path,
            torch_dtype=torch.bfloat16,
            device_map="auto",
        )
        self.model.eval()
        self.runtime = KoreRuntimeClient(runtime_url)
        self.device = device
    
    def generate(
        self,
        prompt: str,
        max_tokens: int = 128,
        temperature: float = 0.1,
        n: int = 1,
    ) -> List[str]:
        """Generate completions"""
        inputs = self.tokenizer(prompt, return_tensors="pt").to(self.device)
        
        outputs = []
        for _ in range(n):
            with torch.no_grad():
                generated = self.model.generate(
                    **inputs,
                    max_new_tokens=max_tokens,
                    temperature=temperature,
                    do_sample=temperature > 0,
                    pad_token_id=self.tokenizer.pad_token_id,
                )
            
            text = self.tokenizer.decode(generated[0], skip_special_tokens=True)
            completion = text[len(prompt):]
            outputs.append(self._extract_program(completion))
        
        return outputs
    
    def _extract_program(self, text: str) -> str:
        """Extract Kore program from generated text"""
        if "```" in text:
            text = text.split("```")[0]
        
        lines = []
        for line in text.split("\n"):
            line = line.strip()
            if line and not line.startswith("#"):
                lines.append(line)
            elif lines:  # Stop at first comment after code
                break
        
        return " ".join(lines)
    
    def evaluate_task(
        self,
        prompt: str,
        target: Any,
        n_samples: int = 1,
    ) -> Dict[str, Any]:
        """Evaluate a single task"""
        programs = self.generate(prompt, n=n_samples)
        
        results = []
        for program in programs:
            start = time.time()
            result = self.runtime.execute(program)
            elapsed = (time.time() - start) * 1000
            
            correct = (
                result.success and
                len(result.final_stack) == 1 and
                result.final_stack[0] == target
            )
            
            results.append({
                "program": program,
                "success": result.success,
                "output": result.final_stack,
                "correct": correct,
                "time_ms": elapsed,
                "error": result.error,
            })
        
        # pass@k
        any_correct = any(r["correct"] for r in results)
        
        return {
            "prompt": prompt[:100],
            "target": target,
            "pass@1": results[0]["correct"] if results else False,
            f"pass@{n_samples}": any_correct,
            "samples": results,
        }


# ============================================================================
# KoreEval Benchmarks
# ============================================================================

def kore_eval_arithmetic(n: int = 100) -> List[Dict]:
    """Basic arithmetic benchmark"""
    tasks = []
    
    for _ in range(n):
        a = random.randint(1, 100)
        b = random.randint(1, 100)
        op = random.choice(["add", "sub", "mul"])
        
        if op == "add":
            result = a + b
            desc = f"{a} + {b}"
        elif op == "sub":
            result = a - b
            desc = f"{a} - {b}"
        else:
            result = a * b
            desc = f"{a} * {b}"
        
        tasks.append({
            "id": f"arith_{len(tasks)}",
            "prompt": f"Write a Kore program to compute {desc}.\n\n```kore\n",
            "target": result,
            "difficulty": 1,
        })
    
    return tasks


def kore_eval_stack(n: int = 50) -> List[Dict]:
    """Stack manipulation benchmark"""
    tasks = []
    
    # dup then add
    for _ in range(n // 2):
        a = random.randint(1, 50)
        tasks.append({
            "id": f"stack_{len(tasks)}",
            "prompt": f"Write a Kore program that puts {a} on stack, duplicates it, and adds (result: {2*a}).\n\n```kore\n",
            "target": 2 * a,
            "difficulty": 2,
        })
    
    # swap then sub
    for _ in range(n // 2):
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        tasks.append({
            "id": f"stack_{len(tasks)}",
            "prompt": f"Push {a} then {b}, swap them, subtract (result: {b - a}).\n\n```kore\n",
            "target": b - a,
            "difficulty": 2,
        })
    
    return tasks


def kore_eval_control(n: int = 50) -> List[Dict]:
    """Control flow benchmark"""
    tasks = []
    
    # max
    for _ in range(n // 3):
        a = random.randint(1, 100)
        b = random.randint(1, 100)
        tasks.append({
            "id": f"control_{len(tasks)}",
            "prompt": f"Write a Kore program that computes max({a}, {b}) using if.\n\n```kore\n",
            "target": max(a, b),
            "difficulty": 3,
        })
    
    # abs
    for _ in range(n // 3):
        a = random.randint(-50, 50)
        tasks.append({
            "id": f"control_{len(tasks)}",
            "prompt": f"Compute absolute value of {a}.\n\n```kore\n",
            "target": abs(a),
            "difficulty": 3,
        })
    
    # clamp
    for _ in range(n // 3):
        x = random.randint(-20, 120)
        lo, hi = 0, 100
        tasks.append({
            "id": f"control_{len(tasks)}",
            "prompt": f"Clamp {x} to range [{lo}, {hi}].\n\n```kore\n",
            "target": max(lo, min(hi, x)),
            "difficulty": 4,
        })
    
    return tasks


def kore_eval_loops(n: int = 30) -> List[Dict]:
    """Loop benchmark"""
    tasks = []
    
    # sum 1..n
    for _ in range(n // 3):
        k = random.randint(5, 20)
        tasks.append({
            "id": f"loop_{len(tasks)}",
            "prompt": f"Compute sum of 1 to {k} using times.\n\n```kore\n",
            "target": k * (k + 1) // 2,
            "difficulty": 4,
        })
    
    # factorial
    for _ in range(n // 3):
        k = random.randint(3, 7)
        fact = 1
        for i in range(1, k + 1):
            fact *= i
        tasks.append({
            "id": f"loop_{len(tasks)}",
            "prompt": f"Compute {k}! (factorial).\n\n```kore\n",
            "target": fact,
            "difficulty": 5,
        })
    
    # power
    for _ in range(n // 3):
        base = random.randint(2, 5)
        exp = random.randint(2, 5)
        tasks.append({
            "id": f"loop_{len(tasks)}",
            "prompt": f"Compute {base}^{exp} (power).\n\n```kore\n",
            "target": base ** exp,
            "difficulty": 5,
        })
    
    return tasks


def run_kore_eval(evaluator: KoreEvaluator, phase: int = 4) -> EvalResult:
    """Run full KoreEval benchmark"""
    tasks = []
    
    if phase >= 1:
        tasks.extend(kore_eval_arithmetic())
    if phase >= 2:
        tasks.extend(kore_eval_stack())
    if phase >= 3:
        tasks.extend(kore_eval_control())
    if phase >= 4:
        tasks.extend(kore_eval_loops())
    
    random.shuffle(tasks)
    
    correct = 0
    total_time = 0.0
    details = []
    
    for task in tqdm(tasks, desc="KoreEval"):
        result = evaluator.evaluate_task(task["prompt"], task["target"])
        
        if result["pass@1"]:
            correct += 1
        
        if result["samples"]:
            total_time += result["samples"][0]["time_ms"]
        
        details.append({
            "id": task["id"],
            "difficulty": task["difficulty"],
            **result,
        })
    
    return EvalResult(
        benchmark="KoreEval",
        total=len(tasks),
        correct=correct,
        accuracy=correct / len(tasks) if tasks else 0,
        avg_time_ms=total_time / len(tasks) if tasks else 0,
        details=details,
    )


# ============================================================================
# Main
# ============================================================================

def main():
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", type=str, required=True)
    parser.add_argument("--runtime-url", type=str, default="http://localhost:8080")
    parser.add_argument("--phase", type=int, default=4)
    parser.add_argument("--output", type=str, default="eval_results.json")
    args = parser.parse_args()
    
    print(f"Loading model: {args.model}")
    evaluator = KoreEvaluator(args.model, args.runtime_url)
    
    # Check runtime
    if not evaluator.runtime.health_check():
        print("ERROR: Kore runtime not available!")
        return
    
    print(f"\nRunning KoreEval (phase {args.phase})...")
    result = run_kore_eval(evaluator, args.phase)
    
    print(f"\n{'='*50}")
    print(f"KoreEval Results")
    print(f"{'='*50}")
    print(f"Total:    {result.total}")
    print(f"Correct:  {result.correct}")
    print(f"Accuracy: {result.accuracy:.2%}")
    print(f"Avg Time: {result.avg_time_ms:.1f}ms")
    
    # Breakdown by difficulty
    by_diff = {}
    for d in result.details:
        diff = d["difficulty"]
        if diff not in by_diff:
            by_diff[diff] = {"correct": 0, "total": 0}
        by_diff[diff]["total"] += 1
        if d["pass@1"]:
            by_diff[diff]["correct"] += 1
    
    print(f"\nBy Difficulty:")
    for diff in sorted(by_diff.keys()):
        stats = by_diff[diff]
        acc = stats["correct"] / stats["total"] if stats["total"] else 0
        print(f"  Level {diff}: {stats['correct']}/{stats['total']} = {acc:.2%}")
    
    # Save results
    with open(args.output, "w") as f:
        json.dump({
            "benchmark": result.benchmark,
            "total": result.total,
            "correct": result.correct,
            "accuracy": result.accuracy,
            "avg_time_ms": result.avg_time_ms,
            "details": result.details,
        }, f, indent=2)
    
    print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
