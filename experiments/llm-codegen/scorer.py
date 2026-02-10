"""
Scorer — Dense reward function for Kore program evaluation.

Multi-stage reward that gives gradient signal from EVERY attempt,
not just correct/incorrect. This is critical for evolutionary search
and later for policy gradient RL.

Reward breakdown:
  Stage 1 (compile):   +0.05 syntactically valid, +0.10 compiles + typechecks
  Stage 2 (runs):      +0.05 if runs without error
  Stage 3 (type):      +0.05 if output type matches expected
  Stage 4 (closeness): +0.00..0.30 based on numeric proximity
  Stage 5 (correct):   +1.00 if exactly correct
  Stage 6 (elegance):  +0.00..0.20 bonus for shorter programs
  Penalties:           -0.10 for not compiling, -0.05 per unused step budget

Total range: [-0.10 .. +1.70]
"""

from __future__ import annotations
from dataclasses import dataclass
from typing import Any, Optional

from experiment_runner import EvalResult
from curriculum import Task


@dataclass
class ScoreBreakdown:
    """Detailed reward breakdown for analysis and debugging."""
    compile_reward: float = 0.0
    typecheck_reward: float = 0.0
    runs_reward: float = 0.0
    type_match_reward: float = 0.0
    closeness_reward: float = 0.0
    correct_reward: float = 0.0
    elegance_reward: float = 0.0
    penalty: float = 0.0
    total: float = 0.0

    # Metadata
    correct: bool = False
    stage_reached: str = ""  # "compile", "typecheck", "run", "correct"

    def __repr__(self):
        parts = []
        if self.compile_reward: parts.append(f"comp={self.compile_reward:+.2f}")
        if self.typecheck_reward: parts.append(f"tc={self.typecheck_reward:+.2f}")
        if self.runs_reward: parts.append(f"run={self.runs_reward:+.2f}")
        if self.type_match_reward: parts.append(f"type={self.type_match_reward:+.2f}")
        if self.closeness_reward: parts.append(f"close={self.closeness_reward:+.2f}")
        if self.correct_reward: parts.append(f"correct={self.correct_reward:+.2f}")
        if self.elegance_reward: parts.append(f"eleg={self.elegance_reward:+.2f}")
        if self.penalty: parts.append(f"pen={self.penalty:+.2f}")
        return f"Score({self.total:+.3f} [{', '.join(parts)}])"


def _parse_type(result_str: Optional[str]) -> Optional[str]:
    """Extract type from result string like 'Int(8)' → 'int'."""
    if result_str is None:
        return None
    if result_str.startswith("Int("):
        return "int"
    if result_str.startswith("Float("):
        return "float"
    if result_str.startswith("Bool("):
        return "bool"
    if result_str.startswith("Str("):
        return "str"
    if result_str.startswith("List("):
        return "list"
    if result_str.startswith("Pair("):
        return "pair"
    if result_str == "Nil":
        return "nil"
    return "unknown"


def _check_correct(result_value: Any, expected: Any, expected_type: str) -> bool:
    """Check if result matches expected value."""
    if result_value is None or expected is None:
        return result_value == expected

    # For list/pair results stored as raw strings
    if expected_type in ("list", "pair") and isinstance(expected, str):
        if isinstance(result_value, str):
            return result_value == expected
        return False

    # Bool comparison
    if expected_type == "bool":
        if isinstance(expected, bool):
            return result_value is expected or result_value == expected
        return result_value == expected

    # Numeric comparison with float tolerance
    if isinstance(result_value, (int, float)) and isinstance(expected, (int, float)):
        if isinstance(result_value, float) or isinstance(expected, float):
            return abs(float(result_value) - float(expected)) < 1e-6
        return result_value == expected

    return result_value == expected


def _numeric_closeness(result_value: Any, expected: Any) -> float:
    """
    How close is the result to the expected value? [0.0 .. 1.0]
    Returns 0.0 for non-numeric or wildly wrong results.
    """
    if not isinstance(result_value, (int, float)) or not isinstance(expected, (int, float)):
        return 0.0

    if expected == 0:
        # Avoid division by zero — use absolute distance
        diff = abs(float(result_value))
        return max(0.0, 1.0 - diff / 10.0)

    diff = abs(float(result_value) - float(expected))
    scale = max(abs(float(expected)), 1.0)
    ratio = diff / scale

    if ratio > 2.0:
        return 0.0
    return max(0.0, 1.0 - ratio)


def score(eval_result: EvalResult, task: Task, program_tokens: list[str]) -> ScoreBreakdown:
    """
    Compute dense reward for a program evaluation against a task.

    Args:
        eval_result: Result from ServeRunner.eval()
        task: The task being attempted
        program_tokens: The token sequence (for elegance scoring)

    Returns:
        ScoreBreakdown with all components
    """
    s = ScoreBreakdown()
    program_len = len(program_tokens)

    # ── Stage 1: Did it compile? ──
    if not eval_result.compile_ok:
        s.penalty = -0.10
        s.stage_reached = "compile_error"
        s.total = s.penalty
        return s

    s.compile_reward = 0.05
    s.stage_reached = "compiled"

    # ── Stage 1b: Did it typecheck? ──
    if not eval_result.typecheck_ok:
        # Compiled but proof checker rejected — partial credit
        s.typecheck_reward = 0.0
        # Give more credit if fewer type errors
        n_errors = len(eval_result.type_errors)
        if n_errors == 1:
            s.typecheck_reward = 0.02
        s.stage_reached = "typecheck_error"
        s.total = s.compile_reward + s.typecheck_reward
        return s

    s.typecheck_reward = 0.10
    s.stage_reached = "typechecked"

    # ── Stage 2: Did it run without error? ──
    if not eval_result.ok:
        # Compiled + typechecked but runtime error (e.g., step limit, div by zero)
        s.runs_reward = 0.0
        if eval_result.error and "step limit" in eval_result.error:
            s.penalty = -0.05  # Infinite loop penalty
        s.stage_reached = "runtime_error"
        s.total = s.compile_reward + s.typecheck_reward + s.penalty
        return s

    s.runs_reward = 0.05
    s.stage_reached = "ran"

    # ── Stage 3: Does the output type match? ──
    result_type = _parse_type(eval_result.result)
    if result_type and result_type == task.expected_type:
        s.type_match_reward = 0.05
    elif result_type == "int" and task.expected_type in ("int", "float"):
        s.type_match_reward = 0.03  # Close enough

    # ── Stage 4: How close is the answer? ──
    closeness = _numeric_closeness(eval_result.value, task.expected)
    s.closeness_reward = 0.30 * closeness

    # ── Stage 5: Is it exactly correct? ──
    if _check_correct(eval_result.value, task.expected, task.expected_type):
        s.correct_reward = 1.00
        s.correct = True
        s.stage_reached = "correct"

        # ── Stage 6: Elegance bonus (shorter = better) ──
        if program_len > 0 and task.max_tokens > 0:
            efficiency = 1.0 - (program_len / task.max_tokens)
            s.elegance_reward = max(0.0, 0.20 * efficiency)

    # ── Total ──
    s.total = (s.compile_reward + s.typecheck_reward + s.runs_reward +
               s.type_match_reward + s.closeness_reward + s.correct_reward +
               s.elegance_reward + s.penalty)

    return s


def score_quick(eval_result: EvalResult, task: Task, program_tokens: list[str]) -> float:
    """Return just the total score (faster than full breakdown)."""
    return score(eval_result, task, program_tokens).total


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    from experiment_runner import ServeRunner

    runner = ServeRunner()
    print("=== Scorer Self-Test ===\n")

    # Test 1: Correct answer
    task = Task(level=1, description="3+5", expected=8, max_tokens=8, hint="3 5 +")
    r = runner.eval("3 5 +")
    s = score(r, task, ["3", "5", "+"])
    print(f"  Correct:     {s}")
    assert s.correct, f"Should be correct: {s}"
    assert s.total > 1.0, f"Correct should score > 1.0: {s.total}"

    # Test 2: Compile error
    r = runner.eval("bad")
    s = score(r, task, ["bad"])
    print(f"  Compile err: {s}")
    assert not s.correct
    assert s.total < 0, f"Compile error should be negative: {s.total}"

    # Test 3: Typecheck error
    r = runner.eval("+")
    s = score(r, task, ["+"])
    print(f"  Type err:    {s}")
    assert not s.correct
    assert s.total > -0.1, f"Typecheck error should give partial credit: {s.total}"

    # Test 4: Wrong answer but compiles
    r = runner.eval("3 4 +")
    s = score(r, task, ["3", "4", "+"])
    print(f"  Wrong (7):   {s}")
    assert not s.correct
    assert s.total > 0.1, f"Wrong answer that compiles should be positive: {s.total}"

    # Test 5: Close answer → high closeness
    task2 = Task(level=1, description="5*5", expected=25, max_tokens=8)
    r = runner.eval("5 5 + 3 *")  # = 30
    s = score(r, task2, ["5", "5", "+", "3", "*"])
    print(f"  Close (30):  {s}")
    assert s.closeness_reward > 0, f"Close answer should have closeness: {s}"

    # Test 6: Very wrong answer
    r = runner.eval("1")
    s = score(r, task2, ["1"])
    print(f"  Far (1):     {s}")
    assert s.closeness_reward < s.compile_reward, "Very wrong should have low closeness"

    # Test 7: Infinite loop
    r = runner.eval("[ true ] loop", max_steps=500)
    s = score(r, task, ["[", "true", "]", "loop"])
    print(f"  Inf loop:    {s}")
    assert s.penalty < 0, "Infinite loop should have penalty"

    # Test 8: Elegance — short correct answer gets bonus
    task3 = Task(level=1, description="Push 5", expected=5, max_tokens=10, hint="5")
    r = runner.eval("5")
    s = score(r, task3, ["5"])
    print(f"  Elegant:     {s}")
    assert s.elegance_reward > 0, "Short correct answer should get elegance bonus"

    # Test 9: Bool task
    task_bool = Task(level=3, description="3>2?", expected=True,
                      expected_type="bool", max_tokens=8, hint="3 2 >")
    r = runner.eval("3 2 >")
    s = score(r, task_bool, ["3", "2", ">"])
    print(f"  Bool:        {s}")
    assert s.correct, f"Bool task should be correct: {s}"

    # Test 10: List task
    task_list = Task(level=9, description="double list",
                      expected="List([Int(2), Int(4), Int(6)])",
                      expected_type="list", max_tokens=16,
                      hint="( 1 2 3 ) [ 2 * ] map")
    r = runner.eval("( 1 2 3 ) [ 2 * ] map")
    s = score(r, task_list, ["(", "1", "2", "3", ")", "[", "2", "*", "]", "map"])
    print(f"  List:        {s}")
    assert s.correct, f"List task should be correct: {s}"

    runner.close()
    print("\n✓ All scorer tests passed!")
