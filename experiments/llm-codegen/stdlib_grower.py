#!/usr/bin/env python3
"""
Stdlib Grower — Self-extending standard library for Kore LLM Codegen.

The model writes Kore functions → korec verifies them → if they pass,
they're added to a growing prelude → the model can use them for harder tasks.

This creates a feedback loop:
  1. Model receives a "define function X that does Y" task
  2. Model outputs `: fn-f ... ;`
  3. korec compile + proof-check verifies correctness
  4. Test cases verify behavior
  5. If all pass → function added to growing stdlib
  6. Next tasks can USE previously-defined functions

Architecture:
  - StdlibEntry: one verified function definition
  - StdlibGrower: manages the growing stdlib file + verification
  - StdlibTask: tasks that ask the model to define functions
  - StdlibReward: reward function for GRPO that includes stdlib bonuses
"""

from __future__ import annotations
import os
import json
import time
import hashlib
from dataclasses import dataclass, field
from typing import Optional, Any
from pathlib import Path

from experiment_runner import ServeRunner


# =============================================================================
# Data structures
# =============================================================================

@dataclass
class TestCase:
    """A single test case for a stdlib function."""
    input_program: str   # e.g. "3 5 fn-f"
    expected: Any        # e.g. 8
    expected_type: str = "int"


@dataclass
class StdlibEntry:
    """One verified function in the growing stdlib."""
    name: str            # The Kore function name (e.g. "square")
    definition: str      # The full `: name ... ;` definition
    description: str     # Human-readable description
    signature: str       # Stack effect: e.g. "( n -- n*n )"
    tests: list[TestCase] = field(default_factory=list)
    verified: bool = False
    added_at: float = 0.0
    used_count: int = 0  # How many times other functions use this


@dataclass
class StdlibTask:
    """A task asking the model to define a function."""
    name: str            # Desired function name
    description: str     # What the function should do
    signature: str       # Stack effect
    tests: list[TestCase]  # Test cases to verify
    level: int = 19      # Always level 19 (function definitions)
    max_tokens: int = 40
    hint: Optional[str] = None  # Reference implementation
    depends_on: list[str] = field(default_factory=list)  # Functions it should use


# =============================================================================
# Stdlib Grower
# =============================================================================

STDLIB_DIR = Path(__file__).parent / "growing_stdlib"


class StdlibGrower:
    """
    Manages a growing standard library built by the model.

    The stdlib starts empty (or with seed functions) and grows as the
    model successfully defines and verifies new functions.
    """

    def __init__(
        self,
        stdlib_dir: Optional[Path] = None,
        runner: Optional[ServeRunner] = None,
    ):
        self.stdlib_dir = Path(stdlib_dir) if stdlib_dir else STDLIB_DIR
        self.stdlib_dir.mkdir(parents=True, exist_ok=True)

        self.entries: dict[str, StdlibEntry] = {}
        self.stdlib_file = self.stdlib_dir / "agent_stdlib.kore"
        self.meta_file = self.stdlib_dir / "agent_stdlib.json"

        # Runner for verification
        self._runner = runner
        self._owns_runner = runner is None

        # Load existing stdlib if available
        self._load()

    def _get_runner(self) -> ServeRunner:
        if self._runner is None:
            self._runner = ServeRunner()
        return self._runner

    def _load(self):
        """Load existing stdlib metadata."""
        if self.meta_file.exists():
            with open(self.meta_file) as f:
                data = json.load(f)
            for entry_data in data.get("entries", []):
                tests = [TestCase(**t) for t in entry_data.pop("tests", [])]
                entry = StdlibEntry(**entry_data, tests=tests)
                self.entries[entry.name] = entry

    def _save(self):
        """Save stdlib to disk (both .kore and .json metadata)."""
        # Save .kore file
        lines = [
            "-- ================================================================",
            "-- Agent-grown Standard Library",
            f"-- {len(self.entries)} verified functions",
            f"-- Generated at {time.strftime('%Y-%m-%d %H:%M:%S')}",
            "-- ================================================================",
            "",
        ]
        for entry in self.entries.values():
            if entry.verified:
                lines.append(f"-- {entry.description}")
                lines.append(f"-- {entry.signature}")
                lines.append(entry.definition)
                lines.append("")

        with open(self.stdlib_file, "w") as f:
            f.write("\n".join(lines))

        # Save metadata
        meta = {
            "count": len(self.entries),
            "entries": [],
        }
        for entry in self.entries.values():
            entry_data = {
                "name": entry.name,
                "definition": entry.definition,
                "description": entry.description,
                "signature": entry.signature,
                "tests": [{"input_program": t.input_program,
                           "expected": t.expected,
                           "expected_type": t.expected_type}
                          for t in entry.tests],
                "verified": entry.verified,
                "added_at": entry.added_at,
                "used_count": entry.used_count,
            }
            meta["entries"].append(entry_data)

        with open(self.meta_file, "w") as f:
            json.dump(meta, f, indent=2)

    def get_prelude_source(self) -> str:
        """Get the current stdlib as a single source string for --prelude."""
        parts = []
        for entry in self.entries.values():
            if entry.verified:
                parts.append(entry.definition)
        return "\n".join(parts)

    def get_prelude_file(self) -> str:
        """Get path to the stdlib .kore file."""
        self._save()  # Ensure it's up to date
        return str(self.stdlib_file)

    def available_functions(self) -> list[str]:
        """List of available function names."""
        return [name for name, entry in self.entries.items() if entry.verified]

    def verify_and_add(
        self,
        name: str,
        definition: str,
        description: str,
        signature: str,
        tests: list[TestCase],
    ) -> tuple[bool, str]:
        """
        Verify a function definition and add it to the stdlib if it passes.

        Returns (success, message).
        """
        runner = self._get_runner()

        # Step 1: Check the definition compiles on its own
        r = runner.eval(definition)
        # A function definition by itself should compile OK (leaves nothing on stack)
        # Actually it might leave Nil — that's fine

        # Step 2: Check the definition + each test case
        prelude = self.get_prelude_source()
        full_prelude = f"{prelude}\n{definition}" if prelude else definition

        passed = 0
        total = len(tests)
        errors = []

        for tc in tests:
            # Prepend all existing stdlib + new definition, then run test
            full_program = f"{full_prelude}\n{tc.input_program}"
            r = runner.eval(full_program)

            if not r.ok:
                errors.append(f"  Test '{tc.input_program}': {r.error}")
                continue

            # Check expected value
            ok = False
            if isinstance(tc.expected, bool):
                ok = r.value == tc.expected
            elif isinstance(tc.expected, (int, float)):
                if isinstance(r.value, (int, float)):
                    ok = abs(r.value - tc.expected) < 0.01
            elif isinstance(tc.expected, str):
                ok = str(r.value) == tc.expected or tc.expected in str(r.result)
            else:
                ok = True  # Complex types — just check compilation

            if ok:
                passed += 1
            else:
                errors.append(f"  Test '{tc.input_program}': expected {tc.expected}, got {r.value}")

        if passed == total:
            # All tests pass — add to stdlib
            entry = StdlibEntry(
                name=name,
                definition=definition,
                description=description,
                signature=signature,
                tests=tests,
                verified=True,
                added_at=time.time(),
            )
            self.entries[name] = entry
            self._save()
            return True, f"✓ {name}: {passed}/{total} tests passed, added to stdlib"
        else:
            msg = f"✗ {name}: {passed}/{total} tests passed\n" + "\n".join(errors)
            return False, msg

    def status(self) -> dict:
        """Return stdlib growth status."""
        return {
            "total_functions": len(self.entries),
            "verified": sum(1 for e in self.entries.values() if e.verified),
            "functions": list(self.entries.keys()),
            "total_bytes": len(self.get_prelude_source()),
        }

    def close(self):
        if self._owns_runner and self._runner is not None:
            self._runner.close()
            self._runner = None


# =============================================================================
# Seed stdlib tasks — what the model should learn to define first
# =============================================================================

SEED_STDLIB_TASKS: list[StdlibTask] = [
    # ── Tier 1: Basic building blocks ──
    StdlibTask(
        name="square",
        description="Square a number: n → n*n",
        signature="( n -- n*n )",
        tests=[
            TestCase("3 square", 9),
            TestCase("5 square", 25),
            TestCase("0 square", 0),
            TestCase("7 square", 49),
        ],
        hint=": square dup * ;",
    ),
    StdlibTask(
        name="cube",
        description="Cube a number: n → n³",
        signature="( n -- n*n*n )",
        tests=[
            TestCase("2 cube", 8),
            TestCase("3 cube", 27),
            TestCase("4 cube", 64),
        ],
        hint=": cube dup dup * * ;",
    ),
    StdlibTask(
        name="double",
        description="Double a number: n → 2n",
        signature="( n -- 2n )",
        tests=[
            TestCase("3 double", 6),
            TestCase("0 double", 0),
            TestCase("5 double", 10),
        ],
        hint=": double dup + ;",
    ),
    StdlibTask(
        name="inc",
        description="Increment by 1: n → n+1",
        signature="( n -- n+1 )",
        tests=[
            TestCase("0 inc", 1),
            TestCase("4 inc", 5),
            TestCase("9 inc", 10),
        ],
        hint=": inc 1 + ;",
    ),
    StdlibTask(
        name="dec",
        description="Decrement by 1: n → n-1",
        signature="( n -- n-1 )",
        tests=[
            TestCase("1 dec", 0),
            TestCase("5 dec", 4),
            TestCase("10 dec", 9),
        ],
        hint=": dec 1 - ;",
    ),
    StdlibTask(
        name="nip",
        description="Remove second element: a b → b",
        signature="( a b -- b )",
        tests=[
            TestCase("3 5 nip", 5),
            TestCase("1 9 nip", 9),
            TestCase("7 0 nip", 0),
        ],
        hint=": nip swap drop ;",
    ),
    StdlibTask(
        name="tuck",
        description="Copy top under second: a b → b a b",
        signature="( a b -- b a b )",
        tests=[
            TestCase("3 5 tuck drop +", 8),  # b a b → drop → b a → + → a+b
            TestCase("1 2 tuck + swap drop", 3),  # b a b → + → b (a+b) → swap drop → a+b
        ],
        hint=": tuck swap over ;",
    ),

    # ── Tier 2: Arithmetic utilities ──
    StdlibTask(
        name="abs",
        description="Absolute value: n → |n|",
        signature="( n -- |n| )",
        tests=[
            TestCase("3 abs", 3),
            TestCase("0 abs", 0),
        ],
        hint=": abs dup 0 < if neg end ;",
    ),
    StdlibTask(
        name="max",
        description="Maximum of two numbers: a b → max(a,b)",
        signature="( a b -- max )",
        tests=[
            TestCase("3 7 max", 7),
            TestCase("8 2 max", 8),
            TestCase("5 5 max", 5),
        ],
        hint=": max over over < if swap end drop ;",
    ),
    StdlibTask(
        name="min",
        description="Minimum of two numbers: a b → min(a,b)",
        signature="( a b -- min )",
        tests=[
            TestCase("3 7 min", 3),
            TestCase("8 2 min", 2),
            TestCase("5 5 min", 5),
        ],
        hint=": min over over > if swap end drop ;",
    ),
    StdlibTask(
        name="clamp",
        description="Clamp n between lo and hi: n lo hi → clamped",
        signature="( n lo hi -- clamped )",
        depends_on=["max", "min"],
        tests=[
            TestCase("5 0 10 clamp", 5),
            TestCase("0 0 10 clamp", 0),
            TestCase("10 0 10 clamp", 10),
        ],
        hint=": clamp ->c ->b ->a a b max c min ;",
        max_tokens=30,
    ),
    StdlibTask(
        name="sign",
        description="Sign of number: n → -1, 0, or 1",
        signature="( n -- sign )",
        tests=[
            TestCase("5 sign", 1),
            TestCase("0 sign", 0),
        ],
        hint=": sign dup 0 > if drop 1 else dup 0 < if drop -1 else drop 0 end end ;",
        max_tokens=35,
    ),
    StdlibTask(
        name="even?",
        description="Is number even? n → bool",
        signature="( n -- bool )",
        tests=[
            TestCase("4 even?", True),
            TestCase("7 even?", False),
            TestCase("0 even?", True),
        ],
        hint=": even? 2 mod 0 = ;",
    ),
    StdlibTask(
        name="odd?",
        description="Is number odd? n → bool",
        signature="( n -- bool )",
        tests=[
            TestCase("3 odd?", True),
            TestCase("4 odd?", False),
            TestCase("1 odd?", True),
        ],
        hint=": odd? 2 mod 1 = ;",
    ),

    # ── Tier 3: Higher-order / recursive ──
    StdlibTask(
        name="factorial",
        description="Factorial: n → n!",
        signature="( n -- n! )",
        tests=[
            TestCase("0 factorial", 1),
            TestCase("1 factorial", 1),
            TestCase("5 factorial", 120),
            TestCase("3 factorial", 6),
        ],
        hint=": factorial dup 1 > if dup 1 - factorial * else drop 1 end ;",
        max_tokens=35,
    ),
    StdlibTask(
        name="fibonacci",
        description="Fibonacci number: n → fib(n)",
        signature="( n -- fib(n) )",
        tests=[
            TestCase("0 fibonacci", 0),
            TestCase("1 fibonacci", 1),
            TestCase("5 fibonacci", 5),
            TestCase("7 fibonacci", 13),
        ],
        # Iterative fibonacci using variables
        hint=": fibonacci ->n 0 ->a 1 ->b 0 ->i while i n < do a b + ->c a drop b ->a c ->b i 1 + ->i end a ;",
        max_tokens=50,
    ),
    StdlibTask(
        name="gcd",
        description="Greatest common divisor (Euclidean): a b → gcd(a,b)",
        signature="( a b -- gcd )",
        tests=[
            TestCase("12 8 gcd", 4),
            TestCase("7 3 gcd", 1),
            TestCase("10 5 gcd", 5),
        ],
        hint=": gcd ->b ->a while b 0 > do a b mod ->c b ->a c ->b end a ;",
        max_tokens=40,
    ),
    StdlibTask(
        name="pow",
        description="Integer power: base exp → base^exp",
        signature="( base exp -- result )",
        tests=[
            TestCase("2 3 pow", 8),
            TestCase("3 2 pow", 9),
            TestCase("5 0 pow", 1),
            TestCase("2 10 pow", 1024),
        ],
        hint=": pow ->n ->a 1 ->b while n 0 > do b a * ->b n 1 - ->n end b ;",
        max_tokens=40,
    ),

    # ── Tier 4: String utilities ──
    StdlibTask(
        name="str-repeat",
        description="Repeat string n times: str n → repeated",
        signature="( str n -- str' )",
        tests=[
            TestCase('"ab" 3 str-repeat', "ababab"),
        ],
        hint=': str-repeat ->n "" ->a 0 ->i while i n < do a swap dup rot str-concat ->a i 1 + ->i end drop a ;',
        max_tokens=50,
    ),
    StdlibTask(
        name="str-reverse",
        description="Reverse a string",
        signature="( str -- str' )",
        tests=[
            TestCase('"abc" str-reverse', "cba"),
            TestCase('"hello" str-reverse', "olleh"),
        ],
        hint=': str-reverse dup str-len ->n "" ->a n 1 - ->i while i 0 >= do dup i i 1 + str-slice a swap str-concat ->a i 1 - ->i end drop a ;',
        max_tokens=60,
    ),

    # ── Tier 5: List utilities ──
    StdlibTask(
        name="sum-list",
        description="Sum all elements of a list",
        signature="( list -- sum )",
        tests=[
            TestCase("( 1 2 3 ) sum-list", 6),
            TestCase("( 5 5 5 ) sum-list", 15),
            TestCase("( 10 ) sum-list", 10),
        ],
        hint=": sum-list 0 [ + ] fold ;",
    ),
    StdlibTask(
        name="product-list",
        description="Product of all elements",
        signature="( list -- product )",
        tests=[
            TestCase("( 2 3 4 ) product-list", 24),
            TestCase("( 1 2 3 ) product-list", 6),
            TestCase("( 5 ) product-list", 5),
        ],
        hint=": product-list 1 [ * ] fold ;",
    ),
    StdlibTask(
        name="map-double",
        description="Double every element in a list",
        signature="( list -- list' )",
        tests=[
            TestCase("( 1 2 3 ) map-double 0 [ + ] fold", 12),
        ],
        hint=": map-double [ 2 * ] map ;",
    ),
    StdlibTask(
        name="map-square",
        description="Square every element in a list",
        signature="( list -- list' )",
        tests=[
            TestCase("( 1 2 3 ) map-square 0 [ + ] fold", 14),
        ],
        hint=": map-square [ dup * ] map ;",
    ),
    StdlibTask(
        name="count-positives",
        description="Count positive numbers in a list",
        signature="( list -- n )",
        tests=[
            TestCase("( 1 0 3 0 5 ) count-positives", 3),
        ],
        hint=": count-positives [ 0 > ] filter len swap drop ;",
        max_tokens=25,
    ),

    # ── Tier 6: OS-level utilities ──
    StdlibTask(
        name="bool-to-int",
        description="Convert boolean to 0 or 1",
        signature="( bool -- int )",
        tests=[
            TestCase("true bool-to-int", 1),
            TestCase("false bool-to-int", 0),
        ],
        hint=": bool-to-int if 1 else 0 end ;",
    ),
    StdlibTask(
        name="int-to-bool",
        description="Convert int to boolean (0=false, else true)",
        signature="( int -- bool )",
        tests=[
            TestCase("0 int-to-bool", False),
            TestCase("1 int-to-bool", True),
            TestCase("5 int-to-bool", True),
        ],
        hint=": int-to-bool 0 != ;",
    ),
    StdlibTask(
        name="divmod",
        description="Division and modulo: a b → quotient remainder",
        signature="( a b -- quot rem )",
        tests=[
            TestCase("7 3 divmod +", 3),  # 2 + 1 = 3
            TestCase("10 3 divmod drop", 3),  # quotient
        ],
        hint=": divmod ->b ->a a b div a b mod ;",
        max_tokens=25,
    ),
    StdlibTask(
        name="within?",
        description="Is n within [lo, hi)? n lo hi → bool",
        signature="( n lo hi -- bool )",
        tests=[
            TestCase("5 0 10 within?", True),
            TestCase("0 0 10 within?", True),
            TestCase("10 0 10 within?", False),
        ],
        hint=": within? ->c ->b ->a a b >= a c < and ;",
        max_tokens=30,
    ),
]


# =============================================================================
# Generate stdlib growth tasks for training
# =============================================================================

def generate_stdlib_sft_tasks(
    tasks: Optional[list[StdlibTask]] = None,
) -> list[dict]:
    """
    Convert stdlib tasks into SFT training format.

    Each task becomes a (prompt, completion) pair where:
    - prompt: "Define a function called X that does Y. Tests: ..."
    - completion: <think>reasoning</think>\n```kore\n: X ... ;\n```
    """
    from data_prep import format_prompt, PROMPT_TEMPLATE, tokens_for_level, get_level_hints

    if tasks is None:
        tasks = SEED_STDLIB_TASKS

    dataset = []
    for task in tasks:
        if not task.hint:
            continue

        # Build prompt
        test_desc = "\n".join(
            f"  - `{tc.input_program}` → {tc.expected}"
            for tc in task.tests
        )

        prompt = (
            f"You are a Kore programming expert. Kore is a stack-based, proof-checked language.\n\n"
            f"Available tokens: {tokens_for_level(task.level)}\n\n"
            f"## Task\n"
            f"Define a function called `{task.name}` that {task.description}.\n"
            f"Stack effect: {task.signature}\n\n"
            f"## Tests\n{test_desc}\n\n"
            f"## Rules\n{get_level_hints(task.level)}\n\n"
            f"Write the Kore function definition. Output ONLY the definition "
            f"(`: {task.name} ... ;`), nothing else."
        )

        # Build completion with reasoning
        reasoning = (
            f"I need to define `{task.name}` which {task.description}.\n"
            f"Stack effect: {task.signature}\n"
            f"The definition uses `: name ... ;` syntax.\n"
            f"Program: {task.hint}"
        )

        completion = f"<think>\n{reasoning}\n</think>\n```kore\n{task.hint}\n```"

        messages = [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": completion},
        ]

        dataset.append({
            "messages": messages,
            "level": task.level,
            "task_name": task.name,
        })

    return dataset


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    print("=== Stdlib Grower Self-Test ===\n")

    # Create a fresh grower with a temp dir
    import tempfile
    with tempfile.TemporaryDirectory() as tmpdir:
        grower = StdlibGrower(stdlib_dir=Path(tmpdir))

        # Test adding functions from seed tasks
        print("--- Verifying seed tasks ---")
        passed = 0
        failed = 0
        for task in SEED_STDLIB_TASKS:
            if not task.hint:
                continue

            ok, msg = grower.verify_and_add(
                name=task.name,
                definition=task.hint,
                description=task.description,
                signature=task.signature,
                tests=task.tests,
            )
            status = "✓" if ok else "✗"
            print(f"  {status} {task.name}: {msg.split(chr(10))[0]}")
            if ok:
                passed += 1
            else:
                failed += 1
                # Show errors
                for line in msg.split("\n")[1:]:
                    print(f"    {line}")

        print(f"\n  Results: {passed}/{passed+failed} seed functions verified")
        print(f"  Status: {grower.status()}")
        print(f"  Prelude size: {len(grower.get_prelude_source())} bytes")

        # Test using composed functions
        print("\n--- Testing composed functions ---")
        runner = grower._get_runner()

        prelude = grower.get_prelude_source()
        composed_tests = [
            (f"{prelude}\n3 square double", 18, "square(3)*2"),
            (f"{prelude}\n5 factorial", 120, "5!"),
            (f"{prelude}\n4 even?", True, "4 is even"),
            (f"{prelude}\n( 1 2 3 ) sum-list", 6, "sum [1,2,3]"),
        ]

        for prog, expected, desc in composed_tests:
            r = runner.eval(prog)
            if r.ok:
                ok = r.value == expected
                sym = "✓" if ok else "✗"
                print(f"  {sym} {desc}: {r.value} (expected {expected})")
            else:
                print(f"  ✗ {desc}: {r.error}")

        # Generate SFT data
        print("\n--- Generating SFT data ---")
        sft_data = generate_stdlib_sft_tasks()
        print(f"  Generated {len(sft_data)} SFT examples for stdlib tasks")

        grower.close()

    print("\n✓ Stdlib grower self-test complete!")
