#!/usr/bin/env python3
"""
Data Preparation — Convert curriculum tasks into training datasets.

Produces:
  1. SFT dataset: (prompt, completion) pairs with reasoning chains
  2. GRPO prompt dataset: prompts only (model generates completions)

Output format: HuggingFace Dataset (saved as JSON or Arrow files).
"""

from __future__ import annotations
import json
import random
import sys
import os
from pathlib import Path
from typing import Optional

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from curriculum import (
    TASK_POOLS, RANDOM_GENERATORS, MAX_LEVEL, Task, Curriculum,
)
from kore_env import VOCAB_BY_LEVEL


# =============================================================================
# Kore token vocabulary per level (for prompt)
# =============================================================================

def tokens_for_level(level: int) -> str:
    """All tokens available at a given level, as a space-separated string."""
    tokens = []
    for lvl in range(level + 1):
        for tok in VOCAB_BY_LEVEL.get(lvl, []):
            if tok != "END":
                tokens.append(tok)
    return " ".join(tokens)


# =============================================================================
# Level-specific hints for the prompt
# =============================================================================

LEVEL_HINTS: dict[int, str] = {
    0: "- Just push the literal value onto the stack.",
    1: (
        "- Arithmetic is postfix: `3 5 +` means 3+5\n"
        "- `div` is integer division, `mod` is remainder, `neg` negates"
    ),
    2: (
        "- `dup` duplicates the top: `3 dup *` → 9\n"
        "- `swap` exchanges top two: `3 5 swap -` → 2\n"
        "- `over` copies second: `3 5 over` → 3 5 3\n"
        "- `drop` removes top"
    ),
    3: (
        "- Comparisons: `3 5 <` → true, `5 3 <` → false\n"
        "- `=` tests equality, `!=` inequality\n"
        "- `not`, `and`, `or`, `xor` for booleans"
    ),
    4: (
        "- `->a` stores top of stack in variable `a`\n"
        "- `a` recalls the value of variable `a`\n"
        "- Variables: a, b, n, i, c"
    ),
    5: (
        "- `if ... else ... end` for conditionals\n"
        "- `while ... do ... end` for loops\n"
        "- `N [ body ] times` repeats body N times\n"
        "- Example: `0 3 [ 1 + ] times` → 3"
    ),
    6: (
        "- `[ ... ]` creates a quote (anonymous function)\n"
        "- `apply` executes a quote\n"
        "- `cond` is conditional: `bool [then] [else] cond`\n"
        "- `loop` repeats a quote until it pushes false"
    ),
    7: (
        "- `band` bitwise AND, `bor` OR, `bxor` XOR\n"
        "- `bnot` bitwise NOT\n"
        "- `shl` shift left, `shr` shift right"
    ),
    8: (
        "- `pair` makes a pair: `3 4 pair` → Pair(3,4)\n"
        "- `unpair` destructures: `Pair(3,4) unpair` → 3 4\n"
        "- `first` / `second` access pair elements non-destructively"
    ),
    9: (
        "- `( 1 2 3 )` creates a list\n"
        "- `map`: `( 1 2 3 ) [ 2 * ] map` → (2 4 6)\n"
        "- `fold`: `( 1 2 3 ) 0 [ + ] fold` → 6\n"
        "- `filter`: `( 1 2 3 4 ) [ 2 > ] filter` → (3 4)\n"
        "- `head` / `tail` / `len` / `range` / `concat`"
    ),
    10: (
        "- `error` creates an error value: `1 error` → Error(1)\n"
        "- `is-error` tests if top is an error\n"
        "- `[ ... ] try` runs code, catching errors as values"
    ),
    11: (
        "- String literals: `\"hello\"`\n"
        "- `str-len` returns length, `str-concat` joins strings\n"
        "- `to-str` converts numbers to strings\n"
        "- `str-upper` / `str-lower` / `str-trim` / `str-slice`"
    ),
    12: (
        "- `i2f` converts int→float, `f2i` float→int (truncation)\n"
        "- `depth` returns current stack depth\n"
        "- Useful for bridging integer and float worlds"
    ),
    13: (
        "- Float ops: `fadd`, `fsub`, `fmul`, `fdiv`, `fneg`\n"
        "- `fsqrt`, `fabs`, `ffloor`, `fceil`, `fround`\n"
        "- Use float literals: `3.14`, `2.0`, `0.5`"
    ),
    17: (
        "- `map-new` creates empty map\n"
        "- `map-set`: `map \"key\" val map-set` → map with key=val\n"
        "- `map-get`: `map \"key\" map-get` → val\n"
        "- `map-has`: `map \"key\" map-has` → bool\n"
        "- Keys MUST be strings"
    ),
    19: (
        "- `: name ... ;` defines a function\n"
        "- Function names: `fn-f`, `fn-g`, `fn-h`, `fn-rec`\n"
        "- `: fn-f dup * ;` defines square, then `5 fn-f` → 25\n"
        "- Recursive: `: fn-rec dup 1 > if dup 1 - fn-rec * end ;`\n"
        "- Functions can call other functions for composition\n"
        "- Use \\n (newline) between definition and usage"
    ),
}


def get_level_hints(level: int) -> str:
    """Collect all hints from level 0 up to and including `level`."""
    hints = []
    for lvl in range(level + 1):
        if lvl in LEVEL_HINTS:
            hints.append(LEVEL_HINTS[lvl])
    return "\n".join(hints)


# =============================================================================
# Prompt template
# =============================================================================

PROMPT_TEMPLATE = """\
You are a Kore programming expert. Kore is a stack-based, proof-checked language.

Available tokens: {available_tokens}

## Task
{description}

## Expected
Input: (empty stack)
Output: {expected_value} (type: {expected_type})

## Rules
{level_hints}

Write a Kore program that solves this task. Output ONLY the Kore tokens, nothing else."""


def format_prompt(task: Task) -> str:
    """Format a task as a user prompt."""
    level = task.level
    expected_type = task.expected_type or _infer_type(task.expected)
    return PROMPT_TEMPLATE.format(
        available_tokens=tokens_for_level(level),
        description=task.description,
        expected_value=task.expected,
        expected_type=expected_type,
        level_hints=get_level_hints(level),
    )


def _infer_type(value) -> str:
    """Infer Kore type from Python value."""
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str) and value.startswith("List"):
        return "list"
    return "int"


# =============================================================================
# Reasoning chain synthesis (for SFT)
# =============================================================================

REASONING_TEMPLATES: dict[int, str] = {
    0: (
        "The task asks me to push {expected} onto the stack.\n"
        "In Kore, I just write the literal value.\n"
        "Program: {hint}"
    ),
    1: (
        "The task asks: {description}.\n"
        "In Kore (postfix), I push operands first, then the operator.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    2: (
        "The task asks: {description}.\n"
        "I need stack manipulation.\n"
        "Trace: {hint_trace}\n"
        "Program: {hint}"
    ),
    3: (
        "The task asks: {description}.\n"
        "I need a comparison or logic operation.\n"
        "Evaluation: {hint_eval}\n"
        "Program: {hint}"
    ),
    4: (
        "The task asks: {description}.\n"
        "I'll use local variables with -> to store and recall values.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    5: (
        "The task asks: {description}.\n"
        "I need control flow (if/while/times).\n"
        "Strategy: {hint_strategy}\n"
        "Program: {hint}"
    ),
    6: (
        "The task asks: {description}.\n"
        "I need quotations (anonymous functions) with [ ... ].\n"
        "A quote captures code to execute later with apply, or repeat with times.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    7: (
        "The task asks: {description}.\n"
        "I need bitwise operations.\n"
        "band=AND, bor=OR, bxor=XOR, shl=shift left, shr=shift right.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    8: (
        "The task asks: {description}.\n"
        "I need pair operations.\n"
        "pair creates a pair from top two stack values, unpair destructures it.\n"
        "first/second access elements non-destructively.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    9: (
        "The task asks: {description}.\n"
        "I need list operations.\n"
        "( ... ) creates a list, map/fold/filter transform it.\n"
        "range creates a list of numbers.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    10: (
        "The task asks: {description}.\n"
        "I need error handling.\n"
        "`error` creates an error value, `is-error` tests for it.\n"
        "`[ ... ] try` runs code safely, catching errors.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    11: (
        "The task asks: {description}.\n"
        "I need string operations.\n"
        "String literals use double quotes. `to-str` converts numbers.\n"
        "`str-len`, `str-concat`, `str-upper`, `str-lower`, `str-trim`, `str-slice` available.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    12: (
        "The task asks: {description}.\n"
        "I need type conversion.\n"
        "`i2f` converts int to float, `f2i` truncates float to int.\n"
        "`depth` gives the current stack depth.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    13: (
        "The task asks: {description}.\n"
        "I need float math.\n"
        "Float ops: fadd, fsub, fmul, fdiv, fneg, fsqrt, fabs.\n"
        "Float literals: 3.14, 2.0, 0.5.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    17: (
        "The task asks: {description}.\n"
        "I need map (dictionary) operations.\n"
        "`map-new` creates an empty map. Keys must be strings.\n"
        "`map-set`: `map \"key\" val map-set`. `map-get`: `map \"key\" map-get`.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
    19: (
        "The task asks: {description}.\n"
        "I need to define a function with `: name ... ;`.\n"
        "Function names: fn-f, fn-g, fn-h, fn-rec.\n"
        "Functions can be recursive (call themselves) or compose (call each other).\n"
        "The definition must come before usage, separated by newline.\n"
        "Step by step: {hint_steps}\n"
        "Program: {hint}"
    ),
}


def _describe_hint_steps(hint: str) -> str:
    """Generate a step-by-step description from a hint program."""
    tokens = hint.split()
    steps = []
    stack_desc = []

    for tok in tokens:
        if tok.lstrip("-").isdigit():
            stack_desc.append(tok)
            steps.append(f"Push {tok}")
        elif tok in ("+", "-", "*", "div", "mod"):
            op_names = {"+": "add", "-": "subtract", "*": "multiply",
                        "div": "divide", "mod": "modulo"}
            if len(stack_desc) >= 2:
                b = stack_desc.pop()
                a = stack_desc.pop()
                steps.append(f"{op_names.get(tok, tok)} {a} and {b}")
                stack_desc.append(f"({a}{tok}{b})")
            else:
                steps.append(f"Apply {tok}")
        elif tok == "dup":
            if stack_desc:
                stack_desc.append(stack_desc[-1])
            steps.append("Duplicate top")
        elif tok == "swap":
            if len(stack_desc) >= 2:
                stack_desc[-1], stack_desc[-2] = stack_desc[-2], stack_desc[-1]
            steps.append("Swap top two")
        elif tok == "neg":
            steps.append("Negate top")
        elif tok.startswith("->"):
            var = tok[2:]
            steps.append(f"Store top in variable {var}")
        elif tok in ("a", "b", "n", "i", "c"):
            steps.append(f"Recall variable {tok}")
        elif tok in ("<", ">", "=", "!=", "<=", ">="):
            steps.append(f"Compare with {tok}")
        elif tok in ("true", "false"):
            steps.append(f"Push {tok}")
        elif tok in ("not", "and", "or", "xor"):
            steps.append(f"Apply {tok}")
        elif tok == "if":
            steps.append("If top is true:")
        elif tok == "else":
            steps.append("Otherwise:")
        elif tok == "end":
            steps.append("End block")
        elif tok == "[":
            steps.append("Begin quote")
        elif tok == "]":
            steps.append("End quote")
        elif tok == "times":
            steps.append("Repeat N times")
        elif tok == "while":
            steps.append("While loop:")
        elif tok == "do":
            steps.append("Do:")
        elif tok == "apply":
            steps.append("Execute quote")
        elif tok in ("pair", "unpair", "first", "second"):
            steps.append(f"Pair operation: {tok}")
        elif tok in ("map", "fold", "filter", "head", "tail", "len",
                      "range", "concat", "empty?"):
            steps.append(f"List operation: {tok}")
        elif tok in ("(", ")"):
            steps.append("List literal" if tok == "(" else "End list")
        elif tok == "drop":
            steps.append("Discard top")
            if stack_desc:
                stack_desc.pop()
        elif tok == "over":
            if len(stack_desc) >= 2:
                stack_desc.append(stack_desc[-2])
            steps.append("Copy second element to top")
        elif tok == "rot":
            steps.append("Rotate top three")
        elif tok in ("error", "is-error"):
            steps.append(f"Error handling: {tok}")
        elif tok == "try":
            steps.append("Try (catch errors)")
        elif tok.startswith("str-") or tok == "to-str":
            steps.append(f"String operation: {tok}")
        elif tok.startswith('"') or tok.endswith('"'):
            steps.append(f"String literal {tok}")
        elif tok in ("i2f", "f2i", "depth", "type-of", "describe"):
            steps.append(f"Type conversion: {tok}")
        elif tok.startswith("f") and tok in ("fadd", "fsub", "fmul", "fdiv",
                "fneg", "fsqrt", "fabs", "ffloor", "fceil", "fround",
                "fexp", "flog", "fsin", "fcos", "fpow", "fatan2"):
            steps.append(f"Float math: {tok}")
        elif "." in tok and tok.replace(".", "").replace("-", "").isdigit():
            steps.append(f"Push float {tok}")
        elif tok.startswith("map-"):
            steps.append(f"Map operation: {tok}")
        elif tok == ":":
            steps.append("Begin function definition")
        elif tok == ";":
            steps.append("End function definition")
        elif tok.startswith("fn-"):
            steps.append(f"Call function {tok}")
        elif tok == "\\n" or tok == "\n":
            steps.append("Newline separator")
        else:
            steps.append(f"Token: {tok}")

    return " → ".join(steps)


def synthesize_reasoning(task: Task) -> str:
    """
    Create a reasoning chain for a task's hint.

    Returns the text to go inside <think>...</think>.
    """
    hint = task.hint or ""
    level = min(task.level, max(REASONING_TEMPLATES.keys()))

    hint_steps = _describe_hint_steps(hint)

    # Build template context
    ctx = {
        "description": task.description,
        "expected": task.expected,
        "hint": hint,
        "hint_steps": hint_steps,
        "hint_trace": hint_steps,
        "hint_eval": hint_steps,
        "hint_strategy": hint_steps,
    }

    template = REASONING_TEMPLATES.get(level, REASONING_TEMPLATES[1])
    return template.format(**ctx)


# =============================================================================
# Dataset generation
# =============================================================================

def generate_sft_dataset(
    max_level: int = 19,
    samples_per_random_level: int = 100,
    seed: int = 42,
    include_stdlib: bool = True,
) -> list[dict]:
    """
    Generate SFT training dataset from curriculum.

    Returns list of {"messages": [{"role": ..., "content": ...}, ...]} dicts.
    """
    rng = random.Random(seed)
    dataset = []

    # 1. All static tasks with hints (iterate over all available levels)
    for level in sorted(TASK_POOLS.keys()):
        if level > max_level:
            continue
        pool = TASK_POOLS[level]
        for task in pool:
            if not task.hint:
                continue
            messages = _task_to_messages(task)
            dataset.append({"messages": messages, "level": level})

    # 2. Random tasks for variety — weight harder levels more
    level_weights = {
        0: 0.3, 1: 0.5, 2: 0.7, 3: 0.7,
        4: 1.5, 5: 2.0, 6: 1.5, 7: 0.7,
        8: 1.2, 9: 1.5,
        10: 1.0, 11: 1.2, 12: 0.8, 13: 1.0,
        17: 1.0, 19: 2.5,  # weight fn defs heavily — key for stdlib growth
    }
    for level, gen_fn in RANDOM_GENERATORS.items():
        if level > max_level:
            continue
        weight = level_weights.get(level, 1.0)
        n_samples = int(samples_per_random_level * weight)
        for _ in range(n_samples):
            task = gen_fn(rng)
            if not task.hint:
                continue
            messages = _task_to_messages(task)
            dataset.append({"messages": messages, "level": level})

    # 3. Stdlib function definition tasks
    if include_stdlib and max_level >= 19:
        try:
            from stdlib_grower import generate_stdlib_sft_tasks
            stdlib_data = generate_stdlib_sft_tasks()
            dataset.extend(stdlib_data)
        except ImportError:
            pass  # stdlib_grower not available

    # Shuffle
    rng.shuffle(dataset)

    return dataset


def _task_to_messages(task: Task) -> list[dict]:
    """Convert a Task into a chat message list for SFT."""
    user_content = format_prompt(task)
    reasoning = synthesize_reasoning(task)
    assistant_content = f"<think>\n{reasoning}\n</think>\n\n{task.hint}"

    return [
        {"role": "user", "content": user_content},
        {"role": "assistant", "content": assistant_content},
    ]


def generate_grpo_prompts(
    n_prompts: int = 1000,
    max_level: int = 19,
    seed: int = 42,
) -> list[dict]:
    """
    Generate GRPO prompt dataset (prompts only, no completions).

    Returns list of {"prompt": [{"role": "user", "content": ...}]} dicts.
    """
    curriculum = Curriculum(start_level=0, seed=seed)
    curriculum.level = max_level  # Sample from all levels
    rng = random.Random(seed)

    # Available levels (may be non-contiguous: 0-13, 17, 19)
    available_levels = sorted([l for l in TASK_POOLS.keys() if l <= max_level])

    dataset = []
    for _ in range(n_prompts):
        # Sample from random available level (uniform)
        level = rng.choice(available_levels)
        task = curriculum._sample_from_level(level)
        prompt_text = format_prompt(task)

        # Embed task metadata in a way we can recover later
        dataset.append({
            "prompt": [{"role": "user", "content": prompt_text}],
            # Metadata for reward function
            "_task_expected": task.expected,
            "_task_expected_type": task.expected_type or _infer_type(task.expected),
            "_task_level": task.level,
            "_task_max_tokens": task.max_tokens,
            "_task_description": task.description,
        })

    return dataset


# =============================================================================
# Kore code extraction from model output
# =============================================================================

def extract_kore_code(completion: str) -> str:
    """
    Extract Kore program from model completion.

    The model outputs:
        <think>
        ...reasoning...
        </think>

        kore code here

    We extract everything after </think>.
    Falls back to the full completion if no </think> tag found.
    """
    import re

    # Try to find </think> block
    match = re.search(r'</think>\s*(.+)', completion, re.DOTALL)
    if match:
        code = match.group(1).strip()
        # Remove markdown code fences if present
        fence_match = re.search(r'```\w*\s*\n(.+?)```', code, re.DOTALL)
        if fence_match:
            code = fence_match.group(1).strip()
        # For multi-line code (function definitions), keep all lines
        # A function def starts with `:` and ends with `;` on possibly multiple lines
        lines = [l.strip() for l in code.split("\n") if l.strip() and not l.strip().startswith("```")]
        if lines:
            # Check if this looks like a multi-line function definition
            full = "\n".join(lines)
            if ":" in full and ";" in full:
                return full
            return lines[0]
        return code.split("\n")[0].strip()

    # Fallback: no </think> found
    text = completion.strip()
    # Try code fence extraction
    fence_match = re.search(r'```\w*\s*\n(.+?)```', text, re.DOTALL)
    if fence_match:
        code = fence_match.group(1).strip()
        lines = [l.strip() for l in code.split("\n") if l.strip()]
        full = "\n".join(lines)
        if ":" in full and ";" in full:
            return full
        return lines[0] if lines else code
    # Last non-empty line (or all lines for multi-line)
    lines = [l.strip() for l in text.split("\n") if l.strip()]
    if lines:
        full = "\n".join(lines)
        if ":" in full and ";" in full:
            return full
        return lines[-1]

    return text


def extract_task_from_prompt(prompt_text: str) -> Task:
    """
    Reconstruct a minimal Task from the prompt text for scoring.

    Extracts: expected value, expected type, max_tokens from the prompt.
    """
    import re

    # Extract expected value and type
    match = re.search(r'Output:\s*(.+?)\s*\(type:\s*(\w+)\)', prompt_text)
    expected_raw = ""
    expected_type = "int"
    if match:
        expected_raw = match.group(1).strip()
        expected_type = match.group(2).strip()

    # Parse expected value
    expected = _parse_expected(expected_raw, expected_type)

    # Extract description
    desc_match = re.search(r'## Task\n(.+?)(?:\n##|\Z)', prompt_text, re.DOTALL)
    description = desc_match.group(1).strip() if desc_match else "unknown"

    return Task(
        level=0,  # Not critical for scoring
        description=description,
        expected=expected,
        expected_type=expected_type,
        max_tokens=20,  # Default
    )


def _parse_expected(raw: str, expected_type: str):
    """Parse expected value from string."""
    if expected_type == "bool":
        return raw.lower() in ("true", "1", "yes")
    if expected_type == "float":
        try:
            return float(raw)
        except ValueError:
            return 0.0
    if expected_type == "list":
        return raw  # Keep as string for list comparison
    try:
        return int(raw)
    except ValueError:
        try:
            return float(raw)
        except ValueError:
            return raw


# =============================================================================
# Save / Load
# =============================================================================

def save_dataset(dataset: list[dict], path: str):
    """Save dataset as JSONL."""
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for item in dataset:
            f.write(json.dumps(item) + "\n")
    print(f"  Saved {len(dataset)} examples to {path}")


def load_dataset(path: str) -> list[dict]:
    """Load dataset from JSONL."""
    data = []
    with open(path) as f:
        for line in f:
            data.append(json.loads(line))
    return data


# =============================================================================
# CLI
# =============================================================================

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Generate training datasets for SFT+GRPO")
    parser.add_argument("--max-level", type=int, default=9,
                        help="Max curriculum level (default: 9)")
    parser.add_argument("--sft-random-per-level", type=int, default=50,
                        help="Random samples per level for SFT (default: 50)")
    parser.add_argument("--grpo-prompts", type=int, default=1000,
                        help="Number of GRPO prompts (default: 1000)")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--output-dir", type=str, default="data",
                        help="Output directory (default: data)")

    args = parser.parse_args()

    print("╔══════════════════════════════════════════════════╗")
    print("║  SFT+GRPO — Data Preparation                     ║")
    print("╚══════════════════════════════════════════════════╝\n")

    # Generate SFT dataset
    print("Generating SFT dataset...")
    sft_data = generate_sft_dataset(
        max_level=args.max_level,
        samples_per_random_level=args.sft_random_per_level,
        seed=args.seed,
    )
    sft_path = os.path.join(args.output_dir, "sft_train.jsonl")
    save_dataset(sft_data, sft_path)

    # Show examples
    print(f"\n  Example SFT entry (level {sft_data[0]['level']}):")
    msgs = sft_data[0]["messages"]
    print(f"  User:      {msgs[0]['content'][:100]}...")
    print(f"  Assistant: {msgs[1]['content'][:100]}...")

    # Count per level
    level_counts = {}
    for item in sft_data:
        lvl = item["level"]
        level_counts[lvl] = level_counts.get(lvl, 0) + 1
    print(f"\n  Per level: {dict(sorted(level_counts.items()))}")

    # Generate GRPO prompts
    print(f"\nGenerating GRPO prompt dataset...")
    grpo_data = generate_grpo_prompts(
        n_prompts=args.grpo_prompts,
        max_level=args.max_level,
        seed=args.seed,
    )
    grpo_path = os.path.join(args.output_dir, "grpo_prompts.jsonl")
    save_dataset(grpo_data, grpo_path)

    # Verify extract_kore_code
    print(f"\nTesting extract_kore_code:")
    test_cases = [
        ("<think>\nI need to add 3 and 5.\n</think>\n\n3 5 +", "3 5 +"),
        ("<think>\nCompute square.\n</think>\n4 dup *", "4 dup *"),
        ("3 5 +", "3 5 +"),  # no think block
        ("<think>\nreasoning\n</think>\n\n```\n3 5 +\n```", "3 5 +"),
    ]
    for completion, expected in test_cases:
        result = extract_kore_code(completion)
        status = "✓" if result == expected else f"✗ got '{result}'"
        print(f"  {status} | '{completion[:50]}...' → '{result}'")

    print(f"\n✓ Data preparation complete!")
    print(f"  SFT:  {len(sft_data)} examples → {sft_path}")
    print(f"  GRPO: {len(grpo_data)} prompts → {grpo_path}")
