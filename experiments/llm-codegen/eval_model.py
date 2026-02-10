#!/usr/bin/env python3
"""
Model Evaluation — Test a fine-tuned model on all curriculum levels.

Runs the model on every task in the curriculum, evaluates via korec serve,
and produces a detailed results report.

Usage:
  python eval_model.py --adapter checkpoints/sft-v1          # Eval SFT adapter
  python eval_model.py --adapter checkpoints/grpo-v1         # Eval GRPO adapter
  python eval_model.py --adapter checkpoints/grpo-v1 --n 5   # 5 samples per task
  python eval_model.py --model deepseek-ai/DeepSeek-R1-Distill-Qwen-14B  # Base model
"""

from __future__ import annotations
import argparse
import json
import os
import sys
import time
from pathlib import Path
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def main():
    parser = argparse.ArgumentParser(
        description="Evaluate a fine-tuned model on Kore curriculum tasks",
    )
    parser.add_argument("--model", type=str,
                        default="deepseek-ai/DeepSeek-R1-Distill-Qwen-14B",
                        help="Base model (ignored if --adapter provided)")
    parser.add_argument("--adapter", type=str, default=None,
                        help="Path to LoRA adapter checkpoint")
    parser.add_argument("--max-level", type=int, default=19,
                        help="Max curriculum level to evaluate (default: 19)")
    parser.add_argument("--n", type=int, default=1,
                        help="Samples per task for majority voting (default: 1)")
    parser.add_argument("--temperature", type=float, default=0.6)
    parser.add_argument("--max-new-tokens", type=int, default=512)
    parser.add_argument("--max-seq-len", type=int, default=2048)
    parser.add_argument("--output", type=str, default=None,
                        help="Output JSON path (default: <adapter>/eval_results.json)")
    parser.add_argument("--gpu", type=str,
                        default="0",
                        help="GPU UUID or index")
    parser.add_argument("--verbose", action="store_true")

    args = parser.parse_args()

    os.environ["CUDA_VISIBLE_DEVICES"] = args.gpu

    print("╔══════════════════════════════════════════════════╗")
    print("║  Kore LLM Codegen — Model Evaluation                  ║")
    print("╚══════════════════════════════════════════════════╝\n")

    # ── Load model ──
    print("Loading model...")

    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("  ⚠ Unsloth not installed. Install with: pip install unsloth")
        sys.exit(1)

    load_name = args.adapter if args.adapter else args.model
    print(f"  Loading: {load_name}")

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=load_name,
        max_seq_length=args.max_seq_len,
        dtype=None,
        load_in_4bit=True,
    )

    FastLanguageModel.for_inference(model)
    print(f"  Model ready ({model.config.num_hidden_layers} layers)")

    # ── Setup korec serve ──
    from experiment_runner import ServeRunner
    from scorer import score, ScoreBreakdown
    from curriculum import TASK_POOLS, MAX_LEVEL
    from data_prep import format_prompt, extract_kore_code

    runner = ServeRunner()
    test = runner.eval("3 5 +")
    assert test.ok and test.value == 8, f"korec serve broken: {test}"
    print(f"  korec serve OK ({test.elapsed_us}µs)\n")

    # ── Evaluate ──
    max_level = min(args.max_level, MAX_LEVEL)
    results_by_level: dict[int, list[dict]] = defaultdict(list)
    total_correct = 0
    total_tasks = 0
    t_start = time.monotonic()

    eval_levels = sorted(l for l in TASK_POOLS.keys() if l <= max_level)
    for level in eval_levels:
        pool = TASK_POOLS[level]
        if not pool:
            continue

        level_correct = 0
        level_total = 0

        for task in pool:
            best_score = -999.0
            best_kore = ""
            best_response = ""
            any_correct = False

            for sample_i in range(args.n):
                prompt = format_prompt(task)
                messages = [{"role": "user", "content": prompt}]

                input_ids = tokenizer.apply_chat_template(
                    messages,
                    tokenize=True,
                    add_generation_prompt=True,
                    return_tensors="pt",
                ).to(model.device)

                outputs = model.generate(
                    input_ids,
                    max_new_tokens=args.max_new_tokens,
                    temperature=args.temperature,
                    top_p=0.95,
                    do_sample=True,
                )

                response = tokenizer.decode(
                    outputs[0][input_ids.shape[1]:],
                    skip_special_tokens=True,
                )

                kore = extract_kore_code(response)
                result = runner.eval(kore, max_steps=10000)
                tokens = kore.strip().split()
                breakdown = score(result, task, tokens)

                if breakdown.total > best_score:
                    best_score = breakdown.total
                    best_kore = kore
                    best_response = response

                if breakdown.correct:
                    any_correct = True

            if any_correct:
                level_correct += 1
                total_correct += 1
            level_total += 1
            total_tasks += 1

            result_entry = {
                "description": task.description,
                "expected": str(task.expected),
                "expected_type": task.expected_type or "int",
                "hint": task.hint or "",
                "best_kore": best_kore,
                "best_score": best_score,
                "correct": any_correct,
            }
            results_by_level[level].append(result_entry)

            if args.verbose:
                marker = "✓" if any_correct else "✗"
                print(f"  {marker} L{level} | {task.description:30s} | "
                      f"score={best_score:+.2f} | {best_kore[:40]}")

        # Level summary
        rate = level_correct / max(1, level_total)
        bar = "█" * int(rate * 20) + "░" * (20 - int(rate * 20))
        print(f"  Level {level:2d}: {level_correct:3d}/{level_total:3d} "
              f"({rate:5.1%}) {bar}")

    elapsed = time.monotonic() - t_start
    overall_rate = total_correct / max(1, total_tasks)

    # ── Summary ──
    print(f"\n{'='*60}")
    print(f"  EVALUATION RESULTS")
    print(f"{'='*60}")
    print(f"  Model:       {load_name}")
    print(f"  Levels:      {eval_levels}")
    print(f"  Tasks:       {total_tasks}")
    print(f"  Correct:     {total_correct}/{total_tasks} ({overall_rate:.1%})")
    print(f"  Samples/task: {args.n}")
    print(f"  Time:        {elapsed:.1f}s ({total_tasks/elapsed:.1f} tasks/sec)")
    print(f"  korec evals: {runner.stats().get('eval_count', 0):,}")

    # Per-level table
    print(f"\n  {'Level':>5s} {'Correct':>8s} {'Total':>6s} {'Rate':>6s}")
    print(f"  {'─'*5} {'─'*8} {'─'*6} {'─'*6}")
    for level in eval_levels:
        entries = results_by_level.get(level, [])
        if not entries:
            continue
        correct = sum(1 for e in entries if e["correct"])
        total = len(entries)
        rate = correct / max(1, total)
        print(f"  {level:5d} {correct:8d} {total:6d} {rate:5.1%}")

    # ── Save results ──
    output_path = args.output
    if not output_path:
        base_dir = args.adapter if args.adapter else "eval_results"
        output_path = os.path.join(base_dir, "eval_results.json")

    Path(output_path).parent.mkdir(parents=True, exist_ok=True)

    results = {
        "model": args.model,
        "adapter": args.adapter,
        "max_level": max_level,
        "samples_per_task": args.n,
        "temperature": args.temperature,
        "total_correct": total_correct,
        "total_tasks": total_tasks,
        "overall_rate": overall_rate,
        "elapsed_seconds": elapsed,
        "korec_stats": runner.stats(),
        "per_level": {
            str(level): {
                "correct": sum(1 for e in entries if e["correct"]),
                "total": len(entries),
                "rate": sum(1 for e in entries if e["correct"]) / max(1, len(entries)),
                "tasks": entries,
            }
            for level, entries in sorted(results_by_level.items())
        },
    }

    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\n  Results saved to: {output_path}")

    runner.close()

    # ── Exit code based on results ──
    # 0 if > 50% correct, 1 otherwise (useful for CI)
    sys.exit(0 if overall_rate > 0.5 else 1)


if __name__ == "__main__":
    main()
