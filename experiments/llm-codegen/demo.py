#!/usr/bin/env python3
"""
Kore LLM Codegen — Interactive Demo

Load the trained model and watch it write Kore programs.
Runs a mix of tasks from easy to hard, showing the model's
reasoning and korec verification.
"""

import os
import sys
import re
import time
import functools

# Force unbuffered output so tee/log files see results immediately
print = functools.partial(print, flush=True)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# Demo tasks — (description, expected, level)
# Descriptions match training format closely for best results
DEMO_TASKS = [
    # L0: Push literals
    ("Push 42", "42", 0),
    ("Push true", "true", 0),

    # L1: Arithmetic
    ("Compute 7 + 8", "15", 1),
    ("Compute 13 * 7", "91", 1),
    ("Compute (4 + 5) * 3", "27", 1),
    ("Compute 100 - 37", "63", 1),

    # L2: Stack manipulation
    ("Square 9", "81", 2),
    ("Cube of 3", "27", 2),
    ("Compute 8 - 3 using swap", "5", 2),

    # L3: Comparisons & logic
    ("Is 10 > 3?", "true", 3),
    ("Is 2 = 7?", "false", 3),
    ("not false", "true", 3),
    ("true and true", "true", 3),

    # L4: Variables
    ("Store 6 in a, recall it", "6", 4),
    ("Store 5 in a, compute a + a", "10", 4),
    ("Store 3 in a, 4 in b, compute a * b", "12", 4),
    ("a=2, b=3, compute b-a", "1", 4),

    # L5: Control flow
    ("If 5 > 2 then 1 else 0", "1", 5),
    ("If 1 > 9 then 1 else 0", "0", 5),
    ("0 + 1 three times", "3", 5),

    # L6: Quotations
    ("Apply double to 7", "14", 6),
    ("Apply square to 6", "36", 6),

    # L7: Bitwise
    ("12 band 10", "8", 7),
    ("1 shl 4", "16", 7),
    ("5 bxor 3", "6", 7),

    # L8: Pairs
    ("Make a pair of 3 and 7, get first", "3", 8),
    ("Make a pair of 10 and 20, get second", "20", 8),

    # L9: Advanced
    ("Compute max of 4 and 9", "9", 9),
    ("Compute min of 7 and 2", "2", 9),
]


def main():
    adapter = sys.argv[1] if len(sys.argv) > 1 else "checkpoints/grpo-v2"

    print("╔══════════════════════════════════════════════════╗")
    print("║  Kore LLM Codegen — Interactive Demo                  ║")
    print("║  A model that learned to write Kore programs    ║")
    print("╚══════════════════════════════════════════════════╝")
    print()

    # ── Load model ──
    print("Loading model...")
    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("  ⚠ Unsloth not installed")
        sys.exit(1)

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=adapter,
        max_seq_length=1024,
        dtype=None,
        load_in_4bit=True,
    )
    FastLanguageModel.for_inference(model)
    print(f"  ✓ Model loaded from {adapter}\n")

    # ── Load korec ──
    from experiment_runner import ServeRunner
    runner = ServeRunner(max_steps=10000, timeout=2.0)
    test = runner.eval("1 1 +")
    assert test.ok and test.value == 2
    print(f"  ✓ korec serve ready\n")

    from data_prep import extract_kore_code, format_prompt

    # ── System prompt (same as training) ──
    print("=" * 60)
    print()

    correct = 0
    total = 0

    for desc, expected, level in DEMO_TASKS:
        total += 1

        # Build prompt same way as training
        from curriculum import Task
        task = Task(
            description=desc,
            expected=int(expected) if expected not in ("true", "false") else expected,
            expected_type="bool" if expected in ("true", "false") else "int",
            level=level,
            hint="",
        )
        prompt_text = format_prompt(task)
        messages = [{"role": "user", "content": prompt_text}]

        input_ids = tokenizer.apply_chat_template(
            messages,
            tokenize=True,
            add_generation_prompt=True,
            return_tensors="pt",
        ).to(model.device)

        t0 = time.monotonic()
        outputs = model.generate(
            input_ids,
            max_new_tokens=256,
            temperature=0.6,
            top_p=0.95,
            do_sample=True,
        )
        gen_time = time.monotonic() - t0

        response = tokenizer.decode(
            outputs[0][input_ids.shape[1]:],
            skip_special_tokens=True,
        )

        # Extract reasoning and code
        kore_code = extract_kore_code(response)

        # Extract think block if present
        think = ""
        think_match = re.search(r'<think>(.*?)</think>', response, re.DOTALL)
        if think_match:
            think = think_match.group(1).strip()
            # Truncate long reasoning
            if len(think) > 200:
                think = think[:200] + "..."

        # Verify with korec
        result = runner.eval(kore_code)

        # Check correctness
        if expected in ("true", "false"):
            is_correct = result.ok and str(result.value).lower() == expected
        else:
            is_correct = result.ok and result.value == int(expected)

        if is_correct:
            correct += 1
            mark = "✓"
        else:
            mark = "✗"

        # Display
        print(f"  {mark} Task: {desc}")
        if think:
            # Show first line of reasoning
            first_line = think.split('\n')[0].strip()
            if first_line:
                print(f"    💭 {first_line}")
        print(f"    📝 Kore: {kore_code}")
        print(f"    🔍 korec: ok={result.ok}, value={result.value}  (expected {expected})")
        print(f"    ⏱  {gen_time:.1f}s, {result.elapsed_us}µs verify")
        print()

    runner.close()

    print("=" * 60)
    print(f"  Results: {correct}/{total} correct ({100*correct/total:.1f}%)")
    print("=" * 60)

    # ── Interactive mode ──
    print("\n  Type a task (or 'q' to quit):\n")
    while True:
        try:
            user_input = input("  > ").strip()
        except (EOFError, KeyboardInterrupt):
            break

        if not user_input or user_input.lower() in ('q', 'quit', 'exit'):
            break

        task = Task(
            description=user_input,
            expected=0,
            expected_type="int",
            level=0,
            hint="",
        )
        prompt_text = format_prompt(task)
        messages = [{"role": "user", "content": prompt_text}]

        input_ids = tokenizer.apply_chat_template(
            messages,
            tokenize=True,
            add_generation_prompt=True,
            return_tensors="pt",
        ).to(model.device)

        t0 = time.monotonic()
        outputs = model.generate(
            input_ids,
            max_new_tokens=256,
            temperature=0.6,
            top_p=0.95,
            do_sample=True,
        )
        gen_time = time.monotonic() - t0

        response = tokenizer.decode(
            outputs[0][input_ids.shape[1]:],
            skip_special_tokens=True,
        )

        kore_code = extract_kore_code(response)

        # Show reasoning
        think_match = re.search(r'<think>(.*?)</think>', response, re.DOTALL)
        if think_match:
            think = think_match.group(1).strip()
            for line in think.split('\n')[:5]:
                if line.strip():
                    print(f"    💭 {line.strip()}")

        print(f"    📝 Kore: {kore_code}")

        result = runner.eval(kore_code)
        print(f"    🔍 korec: ok={result.ok}, value={result.value}")
        print(f"    ⏱  {gen_time:.1f}s\n")

    print("\n  👋 Done.")


if __name__ == "__main__":
    main()
