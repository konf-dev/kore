"""
Curriculum — Task generator for Kore LLM Codegen training.

19 levels of progressively harder tasks. Each level tests a specific
category of Kore capabilities, matching VOCAB_BY_LEVEL in kore_env.py.

Each task has:
  - source hint (optimal solution for reference)
  - expected result (for correctness checking)
  - level (determines which tokens are available)
  - max_tokens (budget for the program)

Tasks are randomly sampled from generators, so training sees variety.
"""

from __future__ import annotations
import random
from dataclasses import dataclass, field
from typing import Any, Optional, Callable


@dataclass
class Task:
    """A single training task."""
    level: int
    description: str
    expected: Any               # The expected top-of-stack result
    expected_type: str = "int"  # "int", "float", "bool", "str", "list", "pair"
    max_tokens: int = 20
    hint: Optional[str] = None  # Optimal Kore solution (for eval only)


# =============================================================================
# Level 0: Push literals
# =============================================================================

def _tasks_level0() -> list[Task]:
    """Push a single literal onto the stack."""
    tasks = []
    for n in range(11):  # 0..10
        tasks.append(Task(
            level=0,
            description=f"Push {n}",
            expected=n,
            max_tokens=4,
            hint=str(n),
        ))
    # Alternate phrasings for variety
    for n in [0, 1, 5, 10]:
        tasks.append(Task(
            level=0,
            description=f"Put the number {n} on the stack",
            expected=n,
            max_tokens=4,
            hint=str(n),
        ))
    tasks.append(Task(level=0, description="Push true", expected=True,
                       expected_type="bool", max_tokens=4, hint="true"))
    tasks.append(Task(level=0, description="Push false", expected=False,
                       expected_type="bool", max_tokens=4, hint="false"))
    tasks.append(Task(level=0, description="Push the boolean value true",
                       expected=True, expected_type="bool", max_tokens=4, hint="true"))
    tasks.append(Task(level=0, description="Push the boolean value false",
                       expected=False, expected_type="bool", max_tokens=4, hint="false"))
    return tasks


# =============================================================================
# Level 1: Arithmetic
# =============================================================================

def _tasks_level1() -> list[Task]:
    tasks = []
    # Addition
    for a, b in [(1, 2), (3, 5), (0, 7), (4, 6), (9, 1), (2, 8)]:
        tasks.append(Task(
            level=1, description=f"Compute {a} + {b}", expected=a + b,
            max_tokens=8, hint=f"{a} {b} +",
        ))
    # Subtraction
    for a, b in [(5, 3), (10, 4), (7, 7), (9, 2)]:
        tasks.append(Task(
            level=1, description=f"Compute {a} - {b}", expected=a - b,
            max_tokens=8, hint=f"{a} {b} -",
        ))
    # Multiplication
    for a, b in [(2, 3), (4, 5), (7, 8), (3, 3), (0, 9)]:
        tasks.append(Task(
            level=1, description=f"Compute {a} * {b}", expected=a * b,
            max_tokens=8, hint=f"{a} {b} *",
        ))
    # Division
    for a, b in [(6, 2), (10, 5), (9, 3), (8, 4)]:
        tasks.append(Task(
            level=1, description=f"Compute {a} / {b}", expected=a // b,
            max_tokens=8, hint=f"{a} {b} div",
        ))
    # Modulo
    for a, b in [(7, 3), (10, 4), (9, 2), (5, 5)]:
        tasks.append(Task(
            level=1, description=f"Compute {a} mod {b}", expected=a % b,
            max_tokens=8, hint=f"{a} {b} mod",
        ))
    # Negation
    for a in [3, 7, 0, 1]:
        tasks.append(Task(
            level=1, description=f"Negate {a}", expected=-a,
            max_tokens=6, hint=f"{a} neg",
        ))
    # Compound arithmetic
    tasks.append(Task(level=1, description="Compute (3+5)*2", expected=16,
                       max_tokens=10, hint="3 5 + 2 *"))
    tasks.append(Task(level=1, description="Compute 10-3-2", expected=5,
                       max_tokens=10, hint="10 3 - 2 -"))
    tasks.append(Task(level=1, description="Compute 2*3+4*5", expected=26,
                       max_tokens=12, hint="2 3 * 4 5 * +"))
    tasks.append(Task(level=1, description="Compute (4+6)*3", expected=30,
                       max_tokens=10, hint="4 6 + 3 *"))
    tasks.append(Task(level=1, description="Compute 5*5-5", expected=20,
                       max_tokens=10, hint="5 5 * 5 -"))
    tasks.append(Task(level=1, description="Compute (8-3)*(2+1)", expected=15,
                       max_tokens=12, hint="8 3 - 2 1 + *"))
    tasks.append(Task(level=1, description="Compute 1+2+3+4", expected=10,
                       max_tokens=12, hint="1 2 + 3 + 4 +"))
    tasks.append(Task(level=1, description="Compute 10/2+3", expected=8,
                       max_tokens=10, hint="10 2 div 3 +"))
    tasks.append(Task(level=1, description="Compute 7 mod 2 + 3 mod 2", expected=2,
                       max_tokens=12, hint="7 2 mod 3 2 mod +"))
    tasks.append(Task(level=1, description="Negate 5 then add 10", expected=5,
                       max_tokens=10, hint="5 neg 10 +"))
    tasks.append(Task(level=1, description="Compute 6*7", expected=42,
                       max_tokens=8, hint="6 7 *"))
    tasks.append(Task(level=1, description="Compute 9-9", expected=0,
                       max_tokens=8, hint="9 9 -"))
    tasks.append(Task(level=1, description="Compute 10 mod 3", expected=1,
                       max_tokens=8, hint="10 3 mod"))
    tasks.append(Task(level=1, description="Compute 8/2", expected=4,
                       max_tokens=8, hint="8 2 div"))
    return tasks


# =============================================================================
# Level 2: Stack manipulation
# =============================================================================

def _tasks_level2() -> list[Task]:
    tasks = []
    # dup: a → a a → a*a (squaring)
    for a in [2, 3, 5, 7, 4, 6, 8, 9, 10, 1, 0]:
        tasks.append(Task(
            level=2, description=f"Square {a}", expected=a * a,
            max_tokens=8, hint=f"{a} dup *",
        ))
    # dup: double a value
    for a in [3, 5, 7, 4, 1]:
        tasks.append(Task(
            level=2, description=f"Double {a} using dup", expected=a * 2,
            max_tokens=8, hint=f"{a} dup +",
        ))
    # swap: compute b - a (reversed subtraction)
    for a, b in [(3, 5), (1, 9), (4, 7), (2, 8), (5, 10), (0, 6), (3, 3)]:
        tasks.append(Task(
            level=2, description=f"Compute {b} - {a} using swap", expected=b - a,
            max_tokens=10, hint=f"{a} {b} swap -",
        ))
    # swap for division: compute b/a when pushed in wrong order
    for a, b in [(2, 6), (3, 9), (5, 10)]:
        tasks.append(Task(
            level=2, description=f"Compute {b} / {a} using swap", expected=b // a,
            max_tokens=10, hint=f"{a} {b} swap div",
        ))
    # over: a b → a b a
    for a, b in [(3, 4), (2, 5), (1, 8), (4, 7), (5, 6)]:
        tasks.append(Task(
            level=2, description=f"Push {a} and {b}, then add {a} again",
            expected=a + b + a, max_tokens=12,
            hint=f"{a} {b} over + +",
        ))
    # over for a*(a+b): a b over + *
    for a, b in [(2, 3), (3, 4), (1, 5)]:
        tasks.append(Task(
            level=2, description=f"Compute {a}*({a}+{b})",
            expected=a * (a + b), max_tokens=12,
            hint=f"{a} {b} over + *",
        ))
    # drop: push two, drop one, keep the other
    for a, b in [(3, 7), (5, 2), (1, 9)]:
        tasks.append(Task(
            level=2, description=f"Push {a} then {b}, drop {b}, result is {a}",
            expected=a, max_tokens=8,
            hint=f"{a} {b} drop",
        ))
    # Cube: dup dup * *
    for a in [2, 3, 4, 5]:
        tasks.append(Task(
            level=2, description=f"Cube of {a}",
            expected=a ** 3, max_tokens=10,
            hint=f"{a} dup dup * *",
        ))
    # dup + compound: a^2 + a = a*(a+1)
    for a in [3, 5, 7]:
        tasks.append(Task(
            level=2, description=f"Compute {a} squared plus {a}",
            expected=a * a + a, max_tokens=12,
            hint=f"{a} dup dup * +",
        ))
    return tasks


# =============================================================================
# Level 3: Comparison and logic
# =============================================================================

def _tasks_level3() -> list[Task]:
    tasks = []
    # Comparisons producing booleans — extensive coverage
    for a, b, op, py_op in [
        (3, 5, "<", lambda a, b: a < b),
        (5, 3, "<", lambda a, b: a < b),
        (0, 1, "<", lambda a, b: a < b),
        (10, 10, "<", lambda a, b: a < b),
        (7, 3, "<", lambda a, b: a < b),
        (3, 3, "=", lambda a, b: a == b),
        (3, 5, "=", lambda a, b: a == b),
        (0, 0, "=", lambda a, b: a == b),
        (7, 7, "=", lambda a, b: a == b),
        (1, 10, "=", lambda a, b: a == b),
        (7, 2, ">", lambda a, b: a > b),
        (2, 7, ">", lambda a, b: a > b),
        (5, 5, ">", lambda a, b: a > b),
        (10, 0, ">", lambda a, b: a > b),
        (0, 10, ">", lambda a, b: a > b),
        (4, 6, "<=", lambda a, b: a <= b),
        (6, 6, "<=", lambda a, b: a <= b),
        (8, 3, "<=", lambda a, b: a <= b),
        (3, 8, ">=", lambda a, b: a >= b),
        (5, 5, ">=", lambda a, b: a >= b),
        (9, 2, ">=", lambda a, b: a >= b),
        (3, 5, "!=", lambda a, b: a != b),
        (4, 4, "!=", lambda a, b: a != b),
    ]:
        expected = py_op(a, b)
        tasks.append(Task(
            level=3, description=f"Is {a} {op} {b}?",
            expected=expected, expected_type="bool",
            max_tokens=8, hint=f"{a} {b} {op}",
        ))
    # Logic — all combinations
    tasks.append(Task(level=3, description="not true", expected=False,
                       expected_type="bool", max_tokens=6, hint="true not"))
    tasks.append(Task(level=3, description="not false", expected=True,
                       expected_type="bool", max_tokens=6, hint="false not"))
    tasks.append(Task(level=3, description="true and false", expected=False,
                       expected_type="bool", max_tokens=8, hint="true false and"))
    tasks.append(Task(level=3, description="true and true", expected=True,
                       expected_type="bool", max_tokens=8, hint="true true and"))
    tasks.append(Task(level=3, description="false and false", expected=False,
                       expected_type="bool", max_tokens=8, hint="false false and"))
    tasks.append(Task(level=3, description="true or false", expected=True,
                       expected_type="bool", max_tokens=8, hint="true false or"))
    tasks.append(Task(level=3, description="false or false", expected=False,
                       expected_type="bool", max_tokens=8, hint="false false or"))
    tasks.append(Task(level=3, description="false or true", expected=True,
                       expected_type="bool", max_tokens=8, hint="false true or"))
    tasks.append(Task(level=3, description="true xor true", expected=False,
                       expected_type="bool", max_tokens=8, hint="true true xor"))
    tasks.append(Task(level=3, description="true xor false", expected=True,
                       expected_type="bool", max_tokens=8, hint="true false xor"))
    # Comparison + arithmetic combos
    for a in [0, 1, 3, 5, 8, 10]:
        tasks.append(Task(
            level=3, description=f"Is {a} > 0?",
            expected=a > 0, expected_type="bool",
            max_tokens=8, hint=f"{a} 0 >",
        ))
    for a in [0, 1, 5, 10]:
        tasks.append(Task(
            level=3, description=f"Is {a} = 0?",
            expected=a == 0, expected_type="bool",
            max_tokens=8, hint=f"{a} 0 =",
        ))
    return tasks


# =============================================================================
# Level 4: Local variables
# =============================================================================

def _tasks_level4() -> list[Task]:
    tasks = []
    # ── Store and recall (basic) ──
    for val in [0, 1, 3, 5, 7, 9, 10]:
        tasks.append(Task(
            level=4, description=f"Store {val} in a, recall it",
            expected=val, max_tokens=8, hint=f"{val} ->a a",
        ))
    # ── Store in different variables ──
    for val, var in [(3, "b"), (8, "n"), (2, "i"), (6, "c")]:
        tasks.append(Task(
            level=4, description=f"Store {val} in {var}, recall it",
            expected=val, max_tokens=8, hint=f"{val} ->{var} {var}",
        ))
    # ── Use variable twice: a + a ──
    for val in [1, 2, 3, 4, 5]:
        tasks.append(Task(
            level=4, description=f"Store {val} in a, compute a + a",
            expected=val * 2, max_tokens=10, hint=f"{val} ->a a a +",
        ))
    # ── Use variable twice: a * a ──
    for val in [2, 3, 4, 5, 6]:
        tasks.append(Task(
            level=4, description=f"Store {val} in a, compute a * a",
            expected=val * val, max_tokens=10, hint=f"{val} ->a a a *",
        ))
    # ── Two variables: a + b ──
    for a, b in [(1, 2), (3, 4), (5, 6), (0, 7), (8, 2)]:
        tasks.append(Task(
            level=4, description=f"Store {a} in a, {b} in b, compute a+b",
            expected=a + b, max_tokens=12, hint=f"{a} ->a {b} ->b a b +",
        ))
    # ── Two variables: a * b ──
    for a, b in [(2, 3), (3, 4), (5, 7), (4, 6)]:
        tasks.append(Task(
            level=4, description=f"Store {a} in a, {b} in b, compute a*b",
            expected=a * b, max_tokens=12, hint=f"{a} ->a {b} ->b a b *",
        ))
    # ── Two variables: b - a ──
    for a, b in [(2, 5), (3, 7), (1, 10), (4, 9), (0, 6)]:
        tasks.append(Task(
            level=4, description=f"a={a}, b={b}, compute b-a",
            expected=b - a, max_tokens=12, hint=f"{a} ->a {b} ->b b a -",
        ))
    # ── a² + b² (pythagorean) ──
    for a, b in [(3, 4), (1, 2), (2, 5), (5, 0)]:
        tasks.append(Task(
            level=4, description=f"a={a}, b={b}, compute a²+b²",
            expected=a*a + b*b, max_tokens=16, hint=f"{a} ->a {b} ->b a a * b b * +",
        ))
    # ── a*b + a (factor out a) ──
    for a, b in [(2, 3), (4, 5), (3, 7)]:
        tasks.append(Task(
            level=4, description=f"a={a}, b={b}, compute a*b+a",
            expected=a * b + a, max_tokens=14, hint=f"{a} ->a {b} ->b a b * a +",
        ))
    # ── Three variables ──
    tasks.append(Task(
        level=4, description="a=1, b=2, n=3, compute a+b+n",
        expected=6, max_tokens=16, hint="1 ->a 2 ->b 3 ->n a b + n +",
    ))
    tasks.append(Task(
        level=4, description="a=2, b=3, n=4, compute a*b*n",
        expected=24, max_tokens=16, hint="2 ->a 3 ->b 4 ->n a b * n *",
    ))
    # ── Variable as accumulator ──
    tasks.append(Task(
        level=4, description="Store 0, add 3, add 4, result",
        expected=7, max_tokens=16, hint="0 ->a a 3 + ->a a 4 + ->a a",
    ))
    tasks.append(Task(
        level=4, description="Store 10, subtract 3, subtract 2, result",
        expected=5, max_tokens=16, hint="10 ->a a 3 - ->a a 2 - ->a a",
    ))
    return tasks


# =============================================================================
# Level 5: Control flow (if/else/end, while/do/end, times)
# =============================================================================

def _tasks_level5() -> list[Task]:
    tasks = []
    # ── if/else: true branch ──
    for a, b in [(3, 2), (5, 1), (10, 0), (7, 3), (9, 8)]:
        tasks.append(Task(
            level=5, description=f"If {a} > {b} then 1 else 0",
            expected=1, max_tokens=16, hint=f"{a} {b} > if 1 else 0 end",
        ))
    # ── if/else: false branch ──
    for a, b in [(1, 5), (0, 3), (2, 7), (4, 9)]:
        tasks.append(Task(
            level=5, description=f"If {a} > {b} then 1 else 0",
            expected=0, max_tokens=16, hint=f"{a} {b} > if 1 else 0 end",
        ))
    # ── if/else with equality ──
    for a, b in [(3, 3), (5, 5), (0, 0)]:
        tasks.append(Task(
            level=5, description=f"If {a} = {b} then 1 else 0",
            expected=1, max_tokens=16, hint=f"{a} {b} = if 1 else 0 end",
        ))
    for a, b in [(3, 4), (1, 2)]:
        tasks.append(Task(
            level=5, description=f"If {a} = {b} then 1 else 0",
            expected=0, max_tokens=16, hint=f"{a} {b} = if 1 else 0 end",
        ))
    # ── if/else: compute different values ──
    tasks.append(Task(
        level=5, description="If 5 > 3 then 10 else 20",
        expected=10, max_tokens=16, hint="5 3 > if 10 else 20 end",
    ))
    tasks.append(Task(
        level=5, description="If 2 > 8 then 10 else 20",
        expected=20, max_tokens=16, hint="2 8 > if 10 else 20 end",
    ))
    # ── if without else (no-op false branch) ──
    tasks.append(Task(
        level=5, description="Push 5, if it's > 3 then add 1",
        expected=6, max_tokens=16, hint="5 dup 3 > if 1 + end",
    ))
    tasks.append(Task(
        level=5, description="Push 1, if it's > 3 keep it as is",
        expected=1, max_tokens=16, hint="1 dup 3 > if 1 + end",
    ))
    # ── max(a,b) via if/else ──
    for a, b in [(3, 7), (8, 2), (5, 5)]:
        tasks.append(Task(
            level=5, description=f"Maximum of {a} and {b}",
            expected=max(a, b), max_tokens=20,
            hint=f"{a} ->a {b} ->b a b > if a else b end",
        ))
    # ── min(a,b) via if/else ──
    for a, b in [(3, 7), (9, 1), (4, 4)]:
        tasks.append(Task(
            level=5, description=f"Minimum of {a} and {b}",
            expected=min(a, b), max_tokens=20,
            hint=f"{a} ->a {b} ->b a b < if a else b end",
        ))
    # ── times: counted iteration ──
    for n in [1, 2, 3, 4, 5]:
        tasks.append(Task(
            level=5, description=f"0 + 1 repeated {n} times",
            expected=n, max_tokens=12, hint=f"0 {n} [ 1 + ] times",
        ))
    # ── times: multiply ──
    tasks.append(Task(
        level=5, description="1 doubled 3 times",
        expected=8, max_tokens=12, hint="1 3 [ 2 * ] times",
    ))
    tasks.append(Task(
        level=5, description="2 doubled 4 times",
        expected=32, max_tokens=12, hint="2 4 [ 2 * ] times",
    ))
    tasks.append(Task(
        level=5, description="3 doubled 2 times",
        expected=12, max_tokens=12, hint="3 2 [ 2 * ] times",
    ))
    # ── times: add a constant ──
    for start, add, n in [(0, 2, 5), (10, 3, 3), (0, 5, 4)]:
        tasks.append(Task(
            level=5, description=f"Start at {start}, add {add} repeated {n} times",
            expected=start + add * n, max_tokens=14,
            hint=f"{start} {n} [ {add} + ] times",
        ))
    # ── times: subtract ──
    tasks.append(Task(
        level=5, description="Start at 10, subtract 2 three times",
        expected=4, max_tokens=12, hint="10 3 [ 2 - ] times",
    ))
    # ── simple while loop: count up ──
    for target in [3, 5, 7]:
        tasks.append(Task(
            level=5, description=f"Count from 0 to {target} using while",
            expected=target, max_tokens=20,
            hint=f"0 while dup {target} < do 1 + end",
        ))
    # ── while: count down ──
    tasks.append(Task(
        level=5, description="Count down from 5 to 0 using while",
        expected=0, max_tokens=20,
        hint="5 while dup 0 > do 1 - end",
    ))
    tasks.append(Task(
        level=5, description="Count down from 10 to 0 using while",
        expected=0, max_tokens=20,
        hint="10 while dup 0 > do 1 - end",
    ))
    # ── sum 1..n using while ──
    for n in [3, 4, 5]:
        expected_sum = n * (n + 1) // 2
        tasks.append(Task(
            level=5, description=f"Sum of 1 to {n} using while",
            expected=expected_sum, max_tokens=25,
            hint=f"{n} ->n 0 ->a while n 0 > do a n + ->a n 1 - ->n end a",
        ))
    # ── factorial using times (simpler version) ──
    # 4! = 24: start with 1, multiply by 2, 3, 4
    # Actually, times with a variable counter is tricky.
    # Simpler: use while
    # ── factorial via while ──
    for n, expected_fact in [(3, 6), (4, 24), (5, 120)]:
        tasks.append(Task(
            level=5, description=f"Compute {n}! using while",
            expected=expected_fact, max_tokens=25,
            hint=f"{n} ->n 1 ->a while n 1 > do a n * ->a n 1 - ->n end a",
        ))
    # ── power of 2 using while ──
    for n in [3, 4, 5]:
        tasks.append(Task(
            level=5, description=f"Compute 2^{n} using while",
            expected=2 ** n, max_tokens=25,
            hint=f"{n} ->n 1 ->a while n 0 > do a 2 * ->a n 1 - ->n end a",
        ))
    return tasks


# =============================================================================
# Level 6: Quotes & higher-order
# =============================================================================

def _tasks_level6() -> list[Task]:
    tasks = []
    # ── apply: basic operations ──
    for val in [3, 5, 7, 10]:
        tasks.append(Task(
            level=6, description=f"Apply double to {val}",
            expected=val * 2, max_tokens=12, hint=f"{val} [ 2 * ] apply",
        ))
    for val in [2, 3, 4, 5, 6]:
        tasks.append(Task(
            level=6, description=f"Apply square to {val}",
            expected=val * val, max_tokens=12, hint=f"{val} [ dup * ] apply",
        ))
    for val in [1, 3, 5, 8]:
        tasks.append(Task(
            level=6, description=f"Apply increment to {val}",
            expected=val + 1, max_tokens=12, hint=f"{val} [ 1 + ] apply",
        ))
    for val in [5, 10, 7]:
        tasks.append(Task(
            level=6, description=f"Apply negate to {val}",
            expected=-val, max_tokens=12, hint=f"{val} [ neg ] apply",
        ))
    # ── apply with multiple ops in quote ──
    tasks.append(Task(
        level=6, description="Apply add-3-then-double to 2",
        expected=10, max_tokens=14, hint="2 [ 3 + 2 * ] apply",
    ))
    tasks.append(Task(
        level=6, description="Apply square-then-add-1 to 3",
        expected=10, max_tokens=14, hint="3 [ dup * 1 + ] apply",
    ))
    # ── times with quote (counted iteration) ──
    for n in [2, 3, 4, 5]:
        tasks.append(Task(
            level=6, description=f"Apply add-1 {n} times to 0",
            expected=n, max_tokens=16, hint=f"0 {n} [ 1 + ] times",
        ))
    for n in [2, 3, 4]:
        tasks.append(Task(
            level=6, description=f"Apply double {n} times to 1",
            expected=2 ** n, max_tokens=16, hint=f"1 {n} [ 2 * ] times",
        ))
    # ── cond: conditional execution ──
    tasks.append(Task(
        level=6, description="If true, apply double to 5, else triple",
        expected=10, max_tokens=16, hint="5 true [ 2 * ] [ 3 * ] cond",
    ))
    tasks.append(Task(
        level=6, description="If false, apply double to 5, else triple",
        expected=15, max_tokens=16, hint="5 false [ 2 * ] [ 3 * ] cond",
    ))
    return tasks


# =============================================================================
# Level 7: Bitwise
# =============================================================================

def _tasks_level7() -> list[Task]:
    tasks = []
    # ── band (bitwise AND) ──
    for a, b in [(5, 3), (7, 6), (10, 3), (9, 5), (3, 3)]:
        tasks.append(Task(level=7, description=f"{a} band {b}", expected=a & b,
                           max_tokens=8, hint=f"{a} {b} band"))
    # ── bor (bitwise OR) ──
    for a, b in [(5, 3), (1, 2), (4, 8), (6, 3), (0, 7)]:
        tasks.append(Task(level=7, description=f"{a} bor {b}", expected=a | b,
                           max_tokens=8, hint=f"{a} {b} bor"))
    # ── bxor (bitwise XOR) ──
    for a, b in [(5, 3), (7, 7), (10, 5), (1, 0), (9, 6)]:
        tasks.append(Task(level=7, description=f"{a} bxor {b}", expected=a ^ b,
                           max_tokens=8, hint=f"{a} {b} bxor"))
    # ── shl (shift left) ──
    for a, n in [(1, 1), (1, 2), (1, 3), (1, 4), (2, 3), (3, 2)]:
        tasks.append(Task(level=7, description=f"{a} shl {n}", expected=a << n,
                           max_tokens=8, hint=f"{a} {n} shl"))
    # ── shr (shift right) ──
    for a, n in [(8, 1), (8, 2), (8, 3), (4, 1), (10, 1)]:
        tasks.append(Task(level=7, description=f"{a} shr {n}", expected=a >> n,
                           max_tokens=8, hint=f"{a} {n} shr"))
    # ── compound bitwise ──
    tasks.append(Task(level=7, description="(5 band 3) bor 8", expected=(5 & 3) | 8,
                       max_tokens=12, hint="5 3 band 8 bor"))
    tasks.append(Task(level=7, description="1 shl 3 band 10", expected=(1 << 3) & 10,
                       max_tokens=12, hint="1 3 shl 10 band"))
    return tasks


# =============================================================================
# Level 8: Pairs
# =============================================================================

def _tasks_level8() -> list[Task]:
    tasks = []
    # ── Make pair and unpair ──
    for a, b in [(3, 4), (1, 2), (5, 7), (0, 9), (8, 3)]:
        tasks.append(Task(
            level=8, description=f"Make pair ({a},{b}), sum both",
            expected=a + b, max_tokens=12, hint=f"{a} {b} pair unpair +",
        ))
    # ── Make pair, unpair, multiply ──
    for a, b in [(2, 3), (4, 5), (3, 7)]:
        tasks.append(Task(
            level=8, description=f"Make pair ({a},{b}), multiply",
            expected=a * b, max_tokens=12, hint=f"{a} {b} pair unpair *",
        ))
    # ── Make pair, unpair, subtract ──
    for a, b in [(5, 3), (9, 1), (7, 7)]:
        tasks.append(Task(
            level=8, description=f"Make pair ({a},{b}), compute first-second",
            expected=a - b, max_tokens=14, hint=f"{a} {b} pair unpair -",
        ))
    # ── first (non-destructive) ──
    for a, b in [(3, 4), (1, 9), (7, 2)]:
        tasks.append(Task(
            level=8, description=f"First of ({a},{b})",
            expected=a, max_tokens=12, hint=f"{a} {b} pair first swap drop",
        ))
    # ── second (non-destructive) ──
    for a, b in [(3, 4), (5, 8), (0, 6)]:
        tasks.append(Task(
            level=8, description=f"Second of ({a},{b})",
            expected=b, max_tokens=12, hint=f"{a} {b} pair second swap drop",
        ))
    return tasks


# =============================================================================
# Level 9: Lists
# =============================================================================

def _tasks_level9() -> list[Task]:
    tasks = []
    # ── map ──
    tasks.append(Task(
        level=9, description="Double each in (1 2 3)", expected="List([Int(2), Int(4), Int(6)])",
        expected_type="list", max_tokens=16, hint="( 1 2 3 ) [ 2 * ] map",
    ))
    tasks.append(Task(
        level=9, description="Add 1 to each in (0 1 2)",
        expected="List([Int(1), Int(2), Int(3)])",
        expected_type="list", max_tokens=16, hint="( 0 1 2 ) [ 1 + ] map",
    ))
    tasks.append(Task(
        level=9, description="Square each in (1 2 3)",
        expected="List([Int(1), Int(4), Int(9)])",
        expected_type="list", max_tokens=16, hint="( 1 2 3 ) [ dup * ] map",
    ))
    tasks.append(Task(
        level=9, description="Triple each in (2 3 4)",
        expected="List([Int(6), Int(9), Int(12)])",
        expected_type="list", max_tokens=16, hint="( 2 3 4 ) [ 3 * ] map",
    ))
    # ── fold (sum) ──
    tasks.append(Task(
        level=9, description="Sum of (1 2 3 4)", expected=10,
        max_tokens=16, hint="( 1 2 3 4 ) 0 [ + ] fold",
    ))
    tasks.append(Task(
        level=9, description="Sum of (1 2 3)", expected=6,
        max_tokens=16, hint="( 1 2 3 ) 0 [ + ] fold",
    ))
    tasks.append(Task(
        level=9, description="Product of (2 3 4)", expected=24,
        max_tokens=16, hint="( 2 3 4 ) 1 [ * ] fold",
    ))
    tasks.append(Task(
        level=9, description="Sum of (5 5 5 5)", expected=20,
        max_tokens=16, hint="( 5 5 5 5 ) 0 [ + ] fold",
    ))
    # ── filter ──
    tasks.append(Task(
        level=9, description="Filter >2 from (1 2 3 4 5)",
        expected="List([Int(3), Int(4), Int(5)])",
        expected_type="list", max_tokens=16, hint="( 1 2 3 4 5 ) [ 2 > ] filter",
    ))
    tasks.append(Task(
        level=9, description="Filter >5 from (3 5 7 9)",
        expected="List([Int(7), Int(9)])",
        expected_type="list", max_tokens=16, hint="( 3 5 7 9 ) [ 5 > ] filter",
    ))
    # ── len ──
    for elems, n in [("1 2 3", 3), ("1 2 3 4 5", 5), ("7", 1)]:
        tasks.append(Task(
            level=9, description=f"Length of ({elems})", expected=n,
            max_tokens=12, hint=f"( {elems} ) len swap drop",
        ))
    # ── head / tail ──
    tasks.append(Task(
        level=9, description="Head of (10 20 30)", expected=10,
        max_tokens=10, hint="( 10 20 30 ) head",
    ))
    tasks.append(Task(
        level=9, description="Head of (5 3 1)", expected=5,
        max_tokens=10, hint="( 5 3 1 ) head",
    ))
    # ── range + fold ──
    for n, expected_sum in [(3, 3), (5, 10), (6, 15)]:
        tasks.append(Task(
            level=9, description=f"Sum of 0..{n}", expected=expected_sum,
            max_tokens=16, hint=f"0 {n} range 0 [ + ] fold",
        ))
    # ── range + map ──
    tasks.append(Task(
        level=9, description="Squares of 1..4",
        expected="List([Int(1), Int(4), Int(9)])",
        expected_type="list", max_tokens=16, hint="1 4 range [ dup * ] map",
    ))
    tasks.append(Task(
        level=9, description="Double each in 1..4",
        expected="List([Int(2), Int(4), Int(6)])",
        expected_type="list", max_tokens=16, hint="1 4 range [ 2 * ] map",
    ))
    # ── concat ──
    tasks.append(Task(
        level=9, description="Concat (1 2) and (3 4)",
        expected="List([Int(1), Int(2), Int(3), Int(4)])",
        expected_type="list", max_tokens=16, hint="( 1 2 ) ( 3 4 ) concat",
    ))
    tasks.append(Task(
        level=9, description="Concat (0) and (1 2 3)",
        expected="List([Int(0), Int(1), Int(2), Int(3)])",
        expected_type="list", max_tokens=16, hint="( 0 ) ( 1 2 3 ) concat",
    ))
    return tasks


# =============================================================================
# Parametric task generators (for variety during training)
# =============================================================================

def _random_push_task(rng: random.Random) -> Task:
    """Generate a random push task (level 0)."""
    val = rng.randint(0, 10)
    return Task(
        level=0, description=f"Push {val}", expected=val,
        max_tokens=4, hint=str(val),
    )


def _random_arith_task(rng: random.Random) -> Task:
    """Generate a random arithmetic task (level 1)."""
    kind = rng.choice(["simple", "compound"])
    if kind == "simple":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        op = rng.choice(["+", "-", "*"])
        if op == "+":
            expected = a + b
        elif op == "-":
            expected = a - b
        else:
            expected = a * b
        return Task(
            level=1, description=f"Compute {a} {op} {b}", expected=expected,
            max_tokens=8, hint=f"{a} {b} {op}",
        )
    else:
        # compound: (a op1 b) op2 c
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        c = rng.randint(1, 10)
        ops = rng.choice([("+", "*"), ("-", "+"), ("+", "-"), ("*", "+")])
        op1, op2 = ops
        if op1 == "+": r1 = a + b
        elif op1 == "-": r1 = a - b
        else: r1 = a * b
        if op2 == "+": expected = r1 + c
        elif op2 == "-": expected = r1 - c
        else: expected = r1 * c
        return Task(
            level=1, description=f"Compute ({a}{op1}{b}){op2}{c}", expected=expected,
            max_tokens=12, hint=f"{a} {b} {op1} {c} {op2}",
        )


def _random_stack_task(rng: random.Random) -> Task:
    """Generate a random stack manipulation task (level 2)."""
    kind = rng.choice(["square", "double", "swap_sub", "cube"])
    a = rng.randint(0, 10)
    if kind == "square":
        return Task(
            level=2, description=f"Square {a}", expected=a * a,
            max_tokens=8, hint=f"{a} dup *",
        )
    elif kind == "double":
        return Task(
            level=2, description=f"Double {a} using dup", expected=a * 2,
            max_tokens=8, hint=f"{a} dup +",
        )
    elif kind == "swap_sub":
        b = rng.randint(0, 10)
        return Task(
            level=2, description=f"Compute {b} - {a} using swap",
            expected=b - a, max_tokens=10, hint=f"{a} {b} swap -",
        )
    else:  # cube
        a = rng.randint(1, 5)
        return Task(
            level=2, description=f"Cube of {a}",
            expected=a ** 3, max_tokens=10, hint=f"{a} dup dup * *",
        )


def _random_compare_task(rng: random.Random) -> Task:
    """Generate a random comparison task (level 3)."""
    kind = rng.choice(["compare", "logic"])
    if kind == "compare":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        op = rng.choice(["<", ">", "=", "!=", "<=", ">="])
        py_ops = {
            "<": lambda a, b: a < b, ">": lambda a, b: a > b,
            "=": lambda a, b: a == b, "!=": lambda a, b: a != b,
            "<=": lambda a, b: a <= b, ">=": lambda a, b: a >= b,
        }
        expected = py_ops[op](a, b)
        return Task(
            level=3, description=f"Is {a} {op} {b}?", expected=expected,
            expected_type="bool", max_tokens=8, hint=f"{a} {b} {op}",
        )
    else:
        # logic
        a_val = rng.choice([True, False])
        b_val = rng.choice([True, False])
        op = rng.choice(["and", "or", "xor"])
        if op == "and": expected = a_val and b_val
        elif op == "or": expected = a_val or b_val
        else: expected = a_val ^ b_val
        a_str = "true" if a_val else "false"
        b_str = "true" if b_val else "false"
        return Task(
            level=3, description=f"{a_str} {op} {b_str}", expected=expected,
            expected_type="bool", max_tokens=8, hint=f"{a_str} {b_str} {op}",
        )


def _random_variable_task(rng: random.Random) -> Task:
    """Generate a random variable task (level 4)."""
    kind = rng.choice([
        "store_recall", "var_double", "var_square",
        "two_var_add", "two_var_mul", "two_var_sub",
    ])
    if kind == "store_recall":
        val = rng.randint(0, 10)
        var = rng.choice(["a", "b", "n"])
        return Task(
            level=4, description=f"Store {val} in {var}, recall it",
            expected=val, max_tokens=8, hint=f"{val} ->{var} {var}",
        )
    elif kind == "var_double":
        val = rng.randint(0, 10)
        return Task(
            level=4, description=f"Store {val} in a, compute a + a",
            expected=val * 2, max_tokens=10, hint=f"{val} ->a a a +",
        )
    elif kind == "var_square":
        val = rng.randint(1, 10)
        return Task(
            level=4, description=f"Store {val} in a, compute a * a",
            expected=val * val, max_tokens=10, hint=f"{val} ->a a a *",
        )
    elif kind == "two_var_add":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        return Task(
            level=4, description=f"Store {a} in a, {b} in b, compute a+b",
            expected=a + b, max_tokens=12, hint=f"{a} ->a {b} ->b a b +",
        )
    elif kind == "two_var_mul":
        a = rng.randint(1, 10)
        b = rng.randint(1, 10)
        return Task(
            level=4, description=f"Store {a} in a, {b} in b, compute a*b",
            expected=a * b, max_tokens=12, hint=f"{a} ->a {b} ->b a b *",
        )
    else:  # two_var_sub
        a = rng.randint(0, 10)
        b = rng.randint(a, 10)
        return Task(
            level=4, description=f"a={a}, b={b}, compute b-a",
            expected=b - a, max_tokens=12, hint=f"{a} ->a {b} ->b b a -",
        )


def _random_control_flow_task(rng: random.Random) -> Task:
    """Generate a random control flow task (level 5)."""
    kind = rng.choice([
        "if_else_gt", "if_else_eq", "times_add", "times_mul",
        "while_count_up", "while_count_down", "max_ab", "min_ab",
    ])
    if kind == "if_else_gt":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        expected = 1 if a > b else 0
        return Task(
            level=5, description=f"If {a} > {b} then 1 else 0",
            expected=expected, max_tokens=16,
            hint=f"{a} {b} > if 1 else 0 end",
        )
    elif kind == "if_else_eq":
        a = rng.randint(0, 10)
        b = rng.choice([a, rng.randint(0, 10)])  # 50% equal
        expected = 1 if a == b else 0
        return Task(
            level=5, description=f"If {a} = {b} then 1 else 0",
            expected=expected, max_tokens=16,
            hint=f"{a} {b} = if 1 else 0 end",
        )
    elif kind == "times_add":
        start = rng.randint(0, 5)
        add = rng.randint(1, 5)
        n = rng.randint(1, 5)
        return Task(
            level=5, description=f"Start at {start}, add {add} repeated {n} times",
            expected=start + add * n, max_tokens=14,
            hint=f"{start} {n} [ {add} + ] times",
        )
    elif kind == "times_mul":
        n = rng.randint(1, 5)
        return Task(
            level=5, description=f"1 doubled {n} times",
            expected=2 ** n, max_tokens=12,
            hint=f"1 {n} [ 2 * ] times",
        )
    elif kind == "while_count_up":
        target = rng.randint(1, 10)
        return Task(
            level=5, description=f"Count from 0 to {target} using while",
            expected=target, max_tokens=20,
            hint=f"0 while dup {target} < do 1 + end",
        )
    elif kind == "while_count_down":
        start = rng.randint(1, 10)
        return Task(
            level=5, description=f"Count down from {start} to 0 using while",
            expected=0, max_tokens=20,
            hint=f"{start} while dup 0 > do 1 - end",
        )
    elif kind == "max_ab":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        return Task(
            level=5, description=f"Maximum of {a} and {b}",
            expected=max(a, b), max_tokens=20,
            hint=f"{a} ->a {b} ->b a b > if a else b end",
        )
    else:  # min_ab
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        return Task(
            level=5, description=f"Minimum of {a} and {b}",
            expected=min(a, b), max_tokens=20,
            hint=f"{a} ->a {b} ->b a b < if a else b end",
        )


def _random_quote_task(rng: random.Random) -> Task:
    """Generate a random quotation task (level 6)."""
    kind = rng.choice(["apply_double", "apply_square", "apply_inc", "times_add"])
    if kind == "apply_double":
        val = rng.randint(0, 10)
        return Task(
            level=6, description=f"Apply double to {val}",
            expected=val * 2, max_tokens=12, hint=f"{val} [ 2 * ] apply",
        )
    elif kind == "apply_square":
        val = rng.randint(0, 10)
        return Task(
            level=6, description=f"Apply square to {val}",
            expected=val * val, max_tokens=12, hint=f"{val} [ dup * ] apply",
        )
    elif kind == "apply_inc":
        val = rng.randint(0, 10)
        return Task(
            level=6, description=f"Apply increment to {val}",
            expected=val + 1, max_tokens=12, hint=f"{val} [ 1 + ] apply",
        )
    else:  # times_add
        n = rng.randint(1, 6)
        return Task(
            level=6, description=f"Apply add-1 {n} times to 0",
            expected=n, max_tokens=16, hint=f"0 {n} [ 1 + ] times",
        )


def _random_bitwise_task(rng: random.Random) -> Task:
    """Generate a random bitwise task (level 7)."""
    a = rng.randint(0, 10)
    b = rng.randint(0, 10)
    op = rng.choice(["band", "bor", "bxor"])
    if op == "band": expected = a & b
    elif op == "bor": expected = a | b
    else: expected = a ^ b
    return Task(
        level=7, description=f"{a} {op} {b}", expected=expected,
        max_tokens=8, hint=f"{a} {b} {op}",
    )


def _random_pair_task(rng: random.Random) -> Task:
    """Generate a random pair task (level 8)."""
    a = rng.randint(0, 10)
    b = rng.randint(0, 10)
    op = rng.choice(["sum", "mul", "first", "second"])
    if op == "sum":
        return Task(
            level=8, description=f"Make pair ({a},{b}), sum both",
            expected=a + b, max_tokens=12, hint=f"{a} {b} pair unpair +",
        )
    elif op == "mul":
        return Task(
            level=8, description=f"Make pair ({a},{b}), multiply",
            expected=a * b, max_tokens=12, hint=f"{a} {b} pair unpair *",
        )
    elif op == "first":
        return Task(
            level=8, description=f"First of ({a},{b})",
            expected=a, max_tokens=12, hint=f"{a} {b} pair first swap drop",
        )
    else:
        return Task(
            level=8, description=f"Second of ({a},{b})",
            expected=b, max_tokens=12, hint=f"{a} {b} pair second swap drop",
        )


def _random_sum_range_task(rng: random.Random) -> Task:
    """Sum of range(0, n) for a random n (level 9)."""
    n = rng.randint(2, 10)
    expected = sum(range(n))
    return Task(
        level=9, description=f"Sum of 0..{n}", expected=expected,
        max_tokens=16, hint=f"0 {n} range 0 [ + ] fold",
    )


# =============================================================================
# Level 10: Error handling
# =============================================================================

def _tasks_level10() -> list[Task]:
    tasks = []
    # ── is-error checks ──
    for val in [1, 0, 3, 5, 10]:
        tasks.append(Task(
            level=10, description=f"Is {val} error an error?",
            expected=True, expected_type="bool",
            max_tokens=10, hint=f"{val} error is-error",
        ))
    for val in [0, 1, 5, 10]:
        tasks.append(Task(
            level=10, description=f"Is {val} an error?",
            expected=False, expected_type="bool",
            max_tokens=8, hint=f"{val} is-error",
        ))
    # ── try: wrap safe code ──
    for a, b in [(3, 5), (1, 2), (4, 6)]:
        tasks.append(Task(
            level=10, description=f"Try computing {a}+{b}",
            expected=a + b, max_tokens=12,
            hint=f"[ {a} {b} + ] try",
        ))
    # ── try: catch error ──
    for val in [1, 2, 3]:
        tasks.append(Task(
            level=10, description=f"Try creating error from {val}, check is-error",
            expected=True, expected_type="bool",
            max_tokens=14, hint=f"[ {val} error ] try is-error",
        ))
    # ── error as value ──
    tasks.append(Task(
        level=10, description="Try 2*3, verify not error",
        expected=False, expected_type="bool",
        max_tokens=14, hint="[ 2 3 * ] try is-error",
    ))
    # ── nested try ──
    tasks.append(Task(
        level=10, description="Nested try: try(try(4+5))",
        expected=9, max_tokens=16,
        hint="[ [ 4 5 + ] try ] try",
    ))
    return tasks


# =============================================================================
# Level 11: String operations
# =============================================================================

def _tasks_level11() -> list[Task]:
    tasks = []
    # ── to-str (number to string) ──
    for val in [0, 1, 42, 7, 10]:
        tasks.append(Task(
            level=11, description=f"Convert {val} to string",
            expected=str(val), expected_type="str",
            max_tokens=8, hint=f"{val} to-str",
        ))
    # ── str-len ──
    for s, l in [("hello", 5), ("hi", 2), ("abc", 3), ("x", 1)]:
        tasks.append(Task(
            level=11, description=f'Length of "{s}"',
            expected=l, max_tokens=8,
            hint=f'"{s}" str-len',
        ))
    # ── str-concat ──
    for a, b in [("ab", "cd"), ("hello", " world"), ("x", "y")]:
        tasks.append(Task(
            level=11, description=f'Concat "{a}" and "{b}"',
            expected=a + b, expected_type="str",
            max_tokens=10,
            hint=f'"{a}" "{b}" str-concat',
        ))
    # ── str-upper ──
    for s in ["abc", "hello", "xyz"]:
        tasks.append(Task(
            level=11, description=f'Uppercase "{s}"',
            expected=s.upper(), expected_type="str",
            max_tokens=8, hint=f'"{s}" str-upper',
        ))
    # ── str-lower ──
    for s in ["ABC", "HELLO", "XYZ"]:
        tasks.append(Task(
            level=11, description=f'Lowercase "{s}"',
            expected=s.lower(), expected_type="str",
            max_tokens=8, hint=f'"{s}" str-lower',
        ))
    # ── str-trim ──
    tasks.append(Task(
        level=11, description='Trim "  hi  "',
        expected="hi", expected_type="str",
        max_tokens=8, hint='"  hi  " str-trim',
    ))
    # ── str-slice ──
    for s, start, end_, result in [("hello", 0, 2, "he"), ("hello", 1, 4, "ell"), ("abcde", 2, 5, "cde")]:
        tasks.append(Task(
            level=11, description=f'Slice "{s}"[{start}:{end_}]',
            expected=result, expected_type="str",
            max_tokens=14,
            hint=f'"{s}" {start} {end_} str-slice',
        ))
    # ── compound: concat + len ──
    tasks.append(Task(
        level=11, description='Length of "ab" + "cde"',
        expected=5, max_tokens=12,
        hint='"ab" "cde" str-concat str-len',
    ))
    # ── compound: to-str + concat ──
    tasks.append(Task(
        level=11, description='Concat "val=" and to-str(42)',
        expected="val=42", expected_type="str",
        max_tokens=12, hint='"val=" 42 to-str str-concat',
    ))
    return tasks


# =============================================================================
# Level 12: Type conversion & introspection
# =============================================================================

def _tasks_level12() -> list[Task]:
    tasks = []
    # ── i2f (int to float) ──
    for val in [0, 1, 3, 5, 10]:
        tasks.append(Task(
            level=12, description=f"Convert {val} to float",
            expected=float(val), expected_type="float",
            max_tokens=8, hint=f"{val} i2f",
        ))
    # ── f2i (float to int, truncation) ──
    for val, expected in [("3.7", 3), ("3.0", 3), ("5.9", 5), ("0.1", 0), ("9.99", 9)]:
        tasks.append(Task(
            level=12, description=f"Convert {val} to int",
            expected=expected, max_tokens=8,
            hint=f"{val} f2i",
        ))
    # ── depth ──
    tasks.append(Task(level=12, description="Depth of empty stack", expected=0,
                       max_tokens=6, hint="depth"))
    tasks.append(Task(level=12, description="Depth after pushing 1", expected=1,
                       max_tokens=8, hint="1 depth swap drop"))
    tasks.append(Task(level=12, description="Depth after pushing 1 2 3", expected=3,
                       max_tokens=10, hint="1 2 3 depth swap drop swap drop swap drop"))
    # ── compound: i2f + float math ──
    for a, b in [(3, 4), (2, 5)]:
        expected = float(a) + float(b)
        tasks.append(Task(
            level=12, description=f"Convert {a} and {b} to float, add",
            expected=expected, expected_type="float",
            max_tokens=12, hint=f"{a} i2f {b} i2f fadd",
        ))
    # ── round-trip: int → float → int ──
    for val in [3, 7, 0]:
        tasks.append(Task(
            level=12, description=f"Convert {val} to float then back to int",
            expected=val, max_tokens=10, hint=f"{val} i2f f2i",
        ))
    return tasks


# =============================================================================
# Level 13: Float math
# =============================================================================

def _tasks_level13() -> list[Task]:
    tasks = []
    # ── basic float ops ──
    for a, b in [("1.5", "2.5"), ("3.0", "4.0"), ("0.1", "0.9")]:
        fa, fb = float(a), float(b)
        tasks.append(Task(
            level=13, description=f"{a} + {b} (float)",
            expected=fa + fb, expected_type="float",
            max_tokens=8, hint=f"{a} {b} fadd",
        ))
    for a, b in [("5.0", "3.0"), ("10.0", "4.5"), ("7.0", "7.0")]:
        fa, fb = float(a), float(b)
        tasks.append(Task(
            level=13, description=f"{a} - {b} (float)",
            expected=fa - fb, expected_type="float",
            max_tokens=8, hint=f"{a} {b} fsub",
        ))
    for a, b in [("2.0", "3.0"), ("1.5", "4.0"), ("0.5", "0.5")]:
        fa, fb = float(a), float(b)
        tasks.append(Task(
            level=13, description=f"{a} * {b} (float)",
            expected=fa * fb, expected_type="float",
            max_tokens=8, hint=f"{a} {b} fmul",
        ))
    for a, b in [("6.0", "2.0"), ("10.0", "4.0"), ("9.0", "3.0")]:
        fa, fb = float(a), float(b)
        tasks.append(Task(
            level=13, description=f"{a} / {b} (float)",
            expected=fa / fb, expected_type="float",
            max_tokens=8, hint=f"{a} {b} fdiv",
        ))
    # ── unary float ops ──
    for val, expected in [("9.0", 3.0), ("4.0", 2.0), ("1.0", 1.0)]:
        tasks.append(Task(
            level=13, description=f"sqrt({val})",
            expected=expected, expected_type="float",
            max_tokens=8, hint=f"{val} fsqrt",
        ))
    for val in ["3.5", "5.0", "0.0"]:
        tasks.append(Task(
            level=13, description=f"negate {val} (float)",
            expected=-float(val), expected_type="float",
            max_tokens=8, hint=f"{val} fneg",
        ))
    for val in ["-5.0", "-3.0", "4.0"]:
        tasks.append(Task(
            level=13, description=f"|{val}| (float abs)",
            expected=abs(float(val)), expected_type="float",
            max_tokens=8, hint=f"{val} fabs",
        ))
    # ── floor/ceil/round ──
    for val, fl, ce, rd in [("3.7", 3.0, 4.0, 4.0), ("3.2", 3.0, 4.0, 3.0), ("5.5", 5.0, 6.0, 6.0)]:
        tasks.append(Task(
            level=13, description=f"floor({val})",
            expected=fl, expected_type="float",
            max_tokens=8, hint=f"{val} ffloor",
        ))
        tasks.append(Task(
            level=13, description=f"ceil({val})",
            expected=ce, expected_type="float",
            max_tokens=8, hint=f"{val} fceil",
        ))
    # ── compound float expressions ──
    tasks.append(Task(
        level=13, description="(1.5 + 2.5) * 3.0",
        expected=12.0, expected_type="float",
        max_tokens=12, hint="1.5 2.5 fadd 3.0 fmul",
    ))
    tasks.append(Task(
        level=13, description="sqrt(4.0) + sqrt(9.0)",
        expected=5.0, expected_type="float",
        max_tokens=12, hint="4.0 fsqrt 9.0 fsqrt fadd",
    ))
    return tasks


# =============================================================================
# Level 17: Maps
# =============================================================================

def _tasks_level17() -> list[Task]:
    tasks = []
    # ── create and get ──
    for key, val in [("x", 10), ("y", 20), ("z", 5), ("a", 0), ("b", 7)]:
        tasks.append(Task(
            level=17, description=f'Map: set "{key}"={val}, get "{key}"',
            expected=val, max_tokens=16,
            hint=f'map-new "{key}" {val} map-set "{key}" map-get',
        ))
    # ── has key ──
    for key in ["x", "a", "name"]:
        tasks.append(Task(
            level=17, description=f'Map: has "{key}" after setting it?',
            expected=True, expected_type="bool",
            max_tokens=16,
            hint=f'map-new "{key}" 1 map-set "{key}" map-has',
        ))
    for key in ["x", "a"]:
        tasks.append(Task(
            level=17, description=f'Map: has "{key}" in empty map?',
            expected=False, expected_type="bool",
            max_tokens=12,
            hint=f'map-new "{key}" map-has',
        ))
    # ── multi-key maps ──
    tasks.append(Task(
        level=17, description='Map: set x=1, y=2, get x',
        expected=1, max_tokens=20,
        hint='map-new "x" 1 map-set "y" 2 map-set "x" map-get',
    ))
    tasks.append(Task(
        level=17, description='Map: set x=1, y=2, get y',
        expected=2, max_tokens=20,
        hint='map-new "x" 1 map-set "y" 2 map-set "y" map-get',
    ))
    # ── overwrite ──
    tasks.append(Task(
        level=17, description='Map: set x=1, then x=9, get x',
        expected=9, max_tokens=20,
        hint='map-new "x" 1 map-set "x" 9 map-set "x" map-get',
    ))
    return tasks


# =============================================================================
# Level 19: Function definitions — the KEY level for stdlib growth
# =============================================================================

def _tasks_level19() -> list[Task]:
    tasks = []
    # ── Simple function definition and call ──
    for val, expected in [(5, 25), (3, 9), (4, 16), (7, 49), (2, 4)]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as square, apply to {val}",
            expected=expected, max_tokens=20,
            hint=f": fn-f dup * ;\n{val} fn-f",
        ))
    # ── Function: double ──
    for val in [3, 5, 7, 10, 0]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as double, apply to {val}",
            expected=val * 2, max_tokens=20,
            hint=f": fn-f dup + ;\n{val} fn-f",
        ))
    # ── Function: abs ──
    for val, expected in [(3, 3), (0, 0)]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as abs, apply to {val}",
            expected=expected, max_tokens=24,
            hint=f": fn-f dup 0 < if neg end ;\n{val} fn-f",
        ))
    # ── Function: increment ──
    for val in [0, 1, 4, 9]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as add-1, apply to {val}",
            expected=val + 1, max_tokens=20,
            hint=f": fn-f 1 + ;\n{val} fn-f",
        ))
    # ── Two functions (composition) ──
    for val in [2, 3, 4, 5]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as square, fn-g as fn-f+1. Apply fn-g to {val}",
            expected=val * val + 1, max_tokens=28,
            hint=f": fn-f dup * ;\n: fn-g fn-f 1 + ;\n{val} fn-g",
        ))
    # ── Recursive factorial ──
    for n, expected in [(3, 6), (4, 24), (5, 120)]:
        tasks.append(Task(
            level=19, description=f"Define recursive factorial, compute {n}!",
            expected=expected, max_tokens=30,
            hint=f": fn-rec dup 1 > if dup 1 - fn-rec * end ;\n{n} fn-rec",
        ))
    # ── Recursive sum 1..n ──
    for n, expected in [(3, 6), (4, 10), (5, 15)]:
        tasks.append(Task(
            level=19, description=f"Define recursive sum, compute sum(1..{n})",
            expected=expected, max_tokens=30,
            hint=f": fn-rec dup 0 > if dup 1 - fn-rec + end ;\n{n} fn-rec",
        ))
    # ── Function: cube ──
    for val in [2, 3, 4]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as cube, apply to {val}",
            expected=val ** 3, max_tokens=24,
            hint=f": fn-f dup dup * * ;\n{val} fn-f",
        ))
    # ── Function: max (two args) ──
    for a, b in [(3, 7), (8, 2), (5, 5)]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as max, compute max({a},{b})",
            expected=max(a, b), max_tokens=28,
            hint=f": fn-f over over < if swap end drop ;\n{a} {b} fn-f",
        ))
    # ── Function: min ──
    for a, b in [(3, 7), (9, 1), (4, 4)]:
        tasks.append(Task(
            level=19, description=f"Define fn-f as min, compute min({a},{b})",
            expected=min(a, b), max_tokens=28,
            hint=f": fn-f over over > if swap end drop ;\n{a} {b} fn-f",
        ))
    # ── Function that uses variables ──
    for a, b in [(2, 3), (4, 5)]:
        tasks.append(Task(
            level=19, description=f"Define fn-f(a,b)=a²+b², compute for {a},{b}",
            expected=a*a + b*b, max_tokens=30,
            hint=f": fn-f ->b ->a a a * b b * + ;\n{a} {b} fn-f",
        ))
    # ── Recursive power ──
    for base, exp, expected in [(2, 3, 8), (3, 2, 9), (2, 4, 16)]:
        tasks.append(Task(
            level=19, description=f"Define recursive power, compute {base}^{exp}",
            expected=expected, max_tokens=35,
            hint=f": fn-f ->b ->a a b 0 > if a b 1 - fn-f * else drop 1 end ;\n{base} {exp} fn-f",
        ))
    return tasks


# =============================================================================
# Random generators for L10-19
# =============================================================================

def _random_error_task(rng: random.Random) -> Task:
    """Random error handling task (level 10)."""
    kind = rng.choice(["is-error", "is-not-error", "try-ok", "try-catch"])
    if kind == "is-error":
        val = rng.randint(0, 10)
        return Task(level=10, description=f"Is {val} error an error?",
                     expected=True, expected_type="bool", max_tokens=10,
                     hint=f"{val} error is-error")
    elif kind == "is-not-error":
        val = rng.randint(0, 10)
        return Task(level=10, description=f"Is {val} an error?",
                     expected=False, expected_type="bool", max_tokens=8,
                     hint=f"{val} is-error")
    elif kind == "try-ok":
        a, b = rng.randint(0, 10), rng.randint(0, 10)
        return Task(level=10, description=f"Try computing {a}+{b}",
                     expected=a + b, max_tokens=12,
                     hint=f"[ {a} {b} + ] try")
    else:
        val = rng.randint(0, 10)
        return Task(level=10, description=f"Try {val} error, check is-error",
                     expected=True, expected_type="bool", max_tokens=14,
                     hint=f"[ {val} error ] try is-error")


def _random_string_task(rng: random.Random) -> Task:
    """Random string task (level 11)."""
    kind = rng.choice(["to-str", "str-len", "str-concat", "str-upper", "str-lower"])
    if kind == "to-str":
        val = rng.randint(0, 10)
        return Task(level=11, description=f"Convert {val} to string",
                     expected=str(val), expected_type="str", max_tokens=8,
                     hint=f"{val} to-str")
    elif kind == "str-len":
        words = ["hi", "abc", "hello", "x", "test", "kore"]
        s = rng.choice(words)
        return Task(level=11, description=f'Length of "{s}"',
                     expected=len(s), max_tokens=8, hint=f'"{s}" str-len')
    elif kind == "str-concat":
        a = rng.choice(["ab", "hi", "x", "foo"])
        b = rng.choice(["cd", "lo", "y", "bar"])
        return Task(level=11, description=f'Concat "{a}" and "{b}"',
                     expected=a + b, expected_type="str", max_tokens=10,
                     hint=f'"{a}" "{b}" str-concat')
    elif kind == "str-upper":
        s = rng.choice(["abc", "xyz", "hi", "ok"])
        return Task(level=11, description=f'Uppercase "{s}"',
                     expected=s.upper(), expected_type="str", max_tokens=8,
                     hint=f'"{s}" str-upper')
    else:
        s = rng.choice(["ABC", "XYZ", "HI", "OK"])
        return Task(level=11, description=f'Lowercase "{s}"',
                     expected=s.lower(), expected_type="str", max_tokens=8,
                     hint=f'"{s}" str-lower')


def _random_conversion_task(rng: random.Random) -> Task:
    """Random type conversion task (level 12)."""
    kind = rng.choice(["i2f", "f2i", "round_trip"])
    if kind == "i2f":
        val = rng.randint(0, 10)
        return Task(level=12, description=f"Convert {val} to float",
                     expected=float(val), expected_type="float", max_tokens=8,
                     hint=f"{val} i2f")
    elif kind == "f2i":
        val = rng.choice(["3.7", "5.9", "0.1", "8.3", "2.0", "9.99"])
        return Task(level=12, description=f"Convert {val} to int",
                     expected=int(float(val)), max_tokens=8,
                     hint=f"{val} f2i")
    else:
        val = rng.randint(0, 10)
        return Task(level=12, description=f"Convert {val} to float then back to int",
                     expected=val, max_tokens=10, hint=f"{val} i2f f2i")


def _random_float_task(rng: random.Random) -> Task:
    """Random float math task (level 13)."""
    kind = rng.choice(["add", "sub", "mul", "div", "sqrt", "neg", "compound"])
    if kind == "add":
        a = round(rng.uniform(0, 5), 1)
        b = round(rng.uniform(0, 5), 1)
        return Task(level=13, description=f"{a} + {b} (float)",
                     expected=a + b, expected_type="float", max_tokens=8,
                     hint=f"{a} {b} fadd")
    elif kind == "sub":
        a = round(rng.uniform(0, 10), 1)
        b = round(rng.uniform(0, a), 1)
        return Task(level=13, description=f"{a} - {b} (float)",
                     expected=a - b, expected_type="float", max_tokens=8,
                     hint=f"{a} {b} fsub")
    elif kind == "mul":
        a = round(rng.uniform(0, 5), 1)
        b = round(rng.uniform(0, 5), 1)
        return Task(level=13, description=f"{a} * {b} (float)",
                     expected=a * b, expected_type="float", max_tokens=8,
                     hint=f"{a} {b} fmul")
    elif kind == "div":
        b = rng.choice([2.0, 3.0, 4.0, 5.0])
        a = b * rng.randint(1, 5)
        return Task(level=13, description=f"{a} / {b} (float)",
                     expected=a / b, expected_type="float", max_tokens=8,
                     hint=f"{a} {b} fdiv")
    elif kind == "sqrt":
        val = rng.choice([1.0, 4.0, 9.0])
        import math
        return Task(level=13, description=f"sqrt({val})",
                     expected=math.sqrt(val), expected_type="float", max_tokens=8,
                     hint=f"{val} fsqrt")
    elif kind == "neg":
        val = round(rng.uniform(0, 10), 1)
        return Task(level=13, description=f"negate {val} (float)",
                     expected=-val, expected_type="float", max_tokens=8,
                     hint=f"{val} fneg")
    else:  # compound
        a = round(rng.uniform(0, 5), 1)
        b = round(rng.uniform(0, 5), 1)
        c = round(rng.uniform(1, 5), 1)
        return Task(level=13, description=f"({a} + {b}) * {c} (float)",
                     expected=(a + b) * c, expected_type="float", max_tokens=12,
                     hint=f"{a} {b} fadd {c} fmul")


def _random_map_task(rng: random.Random) -> Task:
    """Random map task (level 17)."""
    kind = rng.choice(["set-get", "has-yes", "has-no", "overwrite"])
    keys = ["x", "y", "z", "a", "b", "n"]
    if kind == "set-get":
        key = rng.choice(keys)
        val = rng.randint(0, 10)
        return Task(level=17, description=f'Map: set "{key}"={val}, get "{key}"',
                     expected=val, max_tokens=16,
                     hint=f'map-new "{key}" {val} map-set "{key}" map-get')
    elif kind == "has-yes":
        key = rng.choice(keys)
        return Task(level=17, description=f'Map: has "{key}" after setting it?',
                     expected=True, expected_type="bool", max_tokens=16,
                     hint=f'map-new "{key}" 1 map-set "{key}" map-has')
    elif kind == "has-no":
        key = rng.choice(keys)
        return Task(level=17, description=f'Map: has "{key}" in empty map?',
                     expected=False, expected_type="bool", max_tokens=12,
                     hint=f'map-new "{key}" map-has')
    else:
        key = rng.choice(keys)
        v1 = rng.randint(0, 5)
        v2 = rng.randint(6, 10)
        return Task(level=17, description=f'Map: set "{key}"={v1} then {v2}, get',
                     expected=v2, max_tokens=20,
                     hint=f'map-new "{key}" {v1} map-set "{key}" {v2} map-set "{key}" map-get')


def _random_function_task(rng: random.Random) -> Task:
    """Random function definition task (level 19)."""
    kind = rng.choice([
        "square", "double", "increment", "cube",
        "compose", "recursive_fact", "recursive_sum",
        "max_fn", "min_fn",
    ])
    if kind == "square":
        val = rng.randint(0, 10)
        return Task(level=19, description=f"Define fn-f as square, apply to {val}",
                     expected=val * val, max_tokens=20,
                     hint=f": fn-f dup * ;\n{val} fn-f")
    elif kind == "double":
        val = rng.randint(0, 10)
        return Task(level=19, description=f"Define fn-f as double, apply to {val}",
                     expected=val * 2, max_tokens=20,
                     hint=f": fn-f dup + ;\n{val} fn-f")
    elif kind == "increment":
        val = rng.randint(0, 10)
        return Task(level=19, description=f"Define fn-f as add-1, apply to {val}",
                     expected=val + 1, max_tokens=20,
                     hint=f": fn-f 1 + ;\n{val} fn-f")
    elif kind == "cube":
        val = rng.randint(1, 5)
        return Task(level=19, description=f"Define fn-f as cube, apply to {val}",
                     expected=val ** 3, max_tokens=24,
                     hint=f": fn-f dup dup * * ;\n{val} fn-f")
    elif kind == "compose":
        val = rng.randint(1, 7)
        return Task(level=19, description=f"Define fn-f=square, fn-g=fn-f+1. Apply fn-g to {val}",
                     expected=val * val + 1, max_tokens=28,
                     hint=f": fn-f dup * ;\n: fn-g fn-f 1 + ;\n{val} fn-g")
    elif kind == "recursive_fact":
        n = rng.choice([3, 4, 5])
        import math
        return Task(level=19, description=f"Recursive factorial of {n}",
                     expected=math.factorial(n), max_tokens=30,
                     hint=f": fn-rec dup 1 > if dup 1 - fn-rec * end ;\n{n} fn-rec")
    elif kind == "recursive_sum":
        n = rng.randint(2, 7)
        return Task(level=19, description=f"Recursive sum 1..{n}",
                     expected=n * (n + 1) // 2, max_tokens=30,
                     hint=f": fn-rec dup 0 > if dup 1 - fn-rec + end ;\n{n} fn-rec")
    elif kind == "max_fn":
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        return Task(level=19, description=f"Define fn-f as max, compute max({a},{b})",
                     expected=max(a, b), max_tokens=28,
                     hint=f": fn-f over over < if swap end drop ;\n{a} {b} fn-f")
    else:  # min
        a = rng.randint(0, 10)
        b = rng.randint(0, 10)
        return Task(level=19, description=f"Define fn-f as min, compute min({a},{b})",
                     expected=min(a, b), max_tokens=28,
                     hint=f": fn-f over over > if swap end drop ;\n{a} {b} fn-f")


# =============================================================================
# The Curriculum
# =============================================================================

# Static task pools per level (used for fixed evaluation)
TASK_POOLS: dict[int, list[Task]] = {
    0: _tasks_level0(),
    1: _tasks_level1(),
    2: _tasks_level2(),
    3: _tasks_level3(),
    4: _tasks_level4(),
    5: _tasks_level5(),
    6: _tasks_level6(),
    7: _tasks_level7(),
    8: _tasks_level8(),
    9: _tasks_level9(),
    10: _tasks_level10(),
    11: _tasks_level11(),
    12: _tasks_level12(),
    13: _tasks_level13(),
    17: _tasks_level17(),
    19: _tasks_level19(),
}

# Random generators for training variety
RANDOM_GENERATORS: dict[int, Callable] = {
    0: _random_push_task,
    1: _random_arith_task,
    2: _random_stack_task,
    3: _random_compare_task,
    4: _random_variable_task,
    5: _random_control_flow_task,
    6: _random_quote_task,
    7: _random_bitwise_task,
    8: _random_pair_task,
    9: _random_sum_range_task,
    10: _random_error_task,
    11: _random_string_task,
    12: _random_conversion_task,
    13: _random_float_task,
    17: _random_map_task,
    19: _random_function_task,
}

# How many levels we support for training
MAX_LEVEL = max(TASK_POOLS.keys())


class Curriculum:
    """
    Manages progressive task difficulty and level advancement.

    Levels up when success rate > threshold. Replays earlier levels
    to prevent catastrophic forgetting.
    """

    def __init__(
        self,
        start_level: int = 0,
        advance_threshold: float = 0.8,
        replay_fraction: float = 0.2,
        eval_window: int = 50,
        seed: int = 42,
    ):
        self.level = start_level
        self.advance_threshold = advance_threshold
        self.replay_fraction = replay_fraction
        self.eval_window = eval_window
        self.rng = random.Random(seed)

        # Track success per level
        self.history: dict[int, list[bool]] = {i: [] for i in TASK_POOLS.keys()}
        self.level_times: dict[int, float] = {}   # When each level was reached
        self.max_level_reached = start_level

    def sample_task(self) -> Task:
        """
        Sample a training task.

        With probability (1 - replay_fraction): sample from current level.
        With probability replay_fraction: sample from a previous level.
        """
        if self.level > 0 and self.rng.random() < self.replay_fraction:
            # Replay: sample from a random earlier level
            replay_level = self.rng.randint(0, self.level - 1)
            return self._sample_from_level(replay_level)

        return self._sample_from_level(self.level)

    def _sample_from_level(self, level: int) -> Task:
        """Sample a single task from a level (static or random)."""
        # Prefer random generators when available (more variety)
        if level in RANDOM_GENERATORS and self.rng.random() < 0.6:
            return RANDOM_GENERATORS[level](self.rng)

        pool = TASK_POOLS.get(level, TASK_POOLS.get(0, []))
        if not pool:
            return Task(level=0, description="Push 0", expected=0, max_tokens=4, hint="0")
        return self.rng.choice(pool)

    def sample_eval_batch(self, level: Optional[int] = None, n: int = 20) -> list[Task]:
        """Sample a batch of tasks for evaluation (from fixed pool only)."""
        lvl = level if level is not None else self.level
        pool = TASK_POOLS.get(lvl, TASK_POOLS.get(0, []))
        if len(pool) <= n:
            return list(pool)
        return self.rng.sample(pool, n)

    def record(self, task: Task, correct: bool):
        """Record whether a task was solved correctly."""
        self.history[task.level].append(correct)
        # Trim to window
        if len(self.history[task.level]) > self.eval_window * 2:
            self.history[task.level] = self.history[task.level][-self.eval_window:]

    def success_rate(self, level: Optional[int] = None) -> float:
        """Recent success rate for a level."""
        lvl = level if level is not None else self.level
        recent = self.history[lvl][-self.eval_window:]
        if not recent:
            return 0.0
        return sum(recent) / len(recent)

    def maybe_advance(self) -> bool:
        """
        Check if we should advance to the next level.
        Returns True if level was advanced.
        """
        if self.level >= MAX_LEVEL:
            return False

        recent = self.history[self.level][-self.eval_window:]
        if len(recent) < self.eval_window // 2:
            return False  # Not enough data

        rate = sum(recent) / len(recent)
        if rate >= self.advance_threshold:
            import time
            self.level_times[self.level] = time.monotonic()
            self.level += 1
            self.max_level_reached = max(self.max_level_reached, self.level)
            return True

        return False

    def status(self) -> dict:
        """Return curriculum status for logging."""
        rates = {}
        for lvl in range(self.max_level_reached + 1):
            rates[lvl] = self.success_rate(lvl)
        return {
            "level": self.level,
            "max_level": self.max_level_reached,
            "success_rates": rates,
            "total_tasks": sum(len(h) for h in self.history.values()),
        }


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    print("=== Curriculum Self-Test ===\n")

    # Count tasks per level
    total = 0
    for lvl in sorted(TASK_POOLS.keys()):
        pool = TASK_POOLS[lvl]
        total += len(pool)
        print(f"  Level {lvl:2d}: {len(pool):3d} tasks")
    print(f"  {'Total':>9s}: {total:3d} tasks")

    # Test curriculum sampling
    cur = Curriculum(start_level=0)
    for _ in range(10):
        task = cur.sample_task()
        print(f"\n  Sampled: level={task.level} '{task.description}' "
              f"expected={task.expected} hint='{task.hint}'")

    # Test level advancement
    print("\n--- Level advancement ---")
    for i in range(100):
        cur.record(Task(level=0, description="", expected=0), correct=True)
    advanced = cur.maybe_advance()
    print(f"  After 100 correct at level 0: advanced={advanced}, now level={cur.level}")

    # Test replay
    print("\n--- Replay sampling ---")
    cur.level = 3
    levels_seen = set()
    for _ in range(100):
        task = cur.sample_task()
        levels_seen.add(task.level)
    print(f"  At level 3, sampled from levels: {sorted(levels_seen)}")
    assert 3 in levels_seen, "Should sample from current level"
    assert any(l < 3 for l in levels_seen), "Should replay earlier levels"

    print(f"\n  Status: {cur.status()}")
    print("\n✓ All curriculum tests passed!")
