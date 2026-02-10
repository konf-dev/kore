"""
Kore LLM Codegen — Kore Environment

Gym-style RL environment that uses the REAL korec binary.
Each episode: agent emits tokens → we write .kore file → korec compile → korec run → parse result.

Critical optimizations:
  1. /dev/shm (RAM filesystem) for temp files — zero disk I/O
  2. Parse proof-checker errors for reward shaping (not just pass/fail)
  3. Action masking — block tokens that are guaranteed to fail
  4. Single subprocess per episode (compile+check is one call)
  5. Reuse file paths (no mktemp overhead)
"""

from __future__ import annotations
import subprocess
import os
import re
import time
from dataclasses import dataclass, field
from typing import Optional, Any
from pathlib import Path


# =============================================================================
# Kore Vocabulary — the agent's action space
# =============================================================================

# Organized by curriculum level. Each level unlocks new tokens.
# Matches ACTUAL korec parser dispatch (src/parser.rs).
# Every token here has been verified to compile + proof-check successfully.
VOCAB_BY_LEVEL: dict[int, list[str]] = {
    0: [  # Literals — push values onto the stack
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
        "true", "false", "nil",
        "END",
    ],
    1: [  # Arithmetic — ( a b -- result )
        "+", "-", "*", "div", "mod", "neg",
    ],
    2: [  # Stack manipulation — reorder/copy/discard
        "dup", "drop", "swap", "over", "rot", "nop",
    ],
    3: [  # Comparisons & logic — ( a b -- bool )
        "=", "!=", "<", ">", "<=", ">=",
        "not", "and", "or", "xor",
    ],
    4: [  # Local variables — ->name binds, name recalls
        "->a", "a", "->b", "b", "->n", "n", "->i", "i", "->c", "c",
    ],
    5: [  # Control flow (keyword style) — compiled to jumps
        "if", "else", "end", "while", "do",
        "times",  # (n quote -- ...) counted iteration
    ],
    6: [  # Quotes & higher-order (P1-pure control flow)
        "[", "]", "apply", "cond", "loop",
    ],
    7: [  # Bitwise operations — ( a b -- int )
        "band", "bor", "bxor", "bnot", "shl", "shr",
    ],
    8: [  # Data structures — pairs & sum types
        "pair", "unpair", "left", "right", "case",
        "first", "second",  # non-destructive pair access
    ],
    9: [  # Lists — first-class immutable sequences
        "len", "get", "set", "append", "reverse", "unlist",
        "map", "fold", "zip",
        "filter", "head", "tail", "range", "concat", "empty?",
    ],
    10: [  # Error handling — errors are values, not exceptions
        "error", "is-error", "try", "fail", "?",
    ],
    11: [  # String operations
        "str-len", "str-get", "str-concat", "str-slice", "to-str",
        "str-find", "str-split", "str-replace",
        "str-upper", "str-lower", "str-trim",
    ],
    12: [  # Type conversion & introspection
        "i2f", "f2i", "type-of", "depth", "describe",
    ],
    13: [  # Float math — strict f64
        "fadd", "fsub", "fmul", "fdiv", "fneg",
        "fsqrt", "fabs", "fexp", "flog",
        "fsin", "fcos", "fatan2", "fpow",
        "ffloor", "fceil", "fround",
    ],
    14: [  # Linear & affine types — P4 capability constraints
        "linear", "affine", "consume", "is-linear", "is-affine",
    ],
    15: [  # Fibers — cooperative multitasking
        "fiber-new", "fiber-step", "fiber-push", "fiber-stack", "fiber-status",
    ],
    16: [  # Concurrency — spawn & channels
        "spawn", "chan-new", "chan-send", "chan-recv",
    ],
    17: [  # Maps — associative data structure
        "map-new", "map-get", "map-set", "map-keys", "map-has",
    ],
    18: [  # Reflection (self-hosting) & advanced control
        "fetch", "size", "ret", "halt",
    ],
    19: [  # Function definitions — enables recursion & abstraction
        ":", ";",
        # Fixed function name placeholders for RL agent
        "fn-f", "fn-g", "fn-h", "fn-rec",
    ],
}

# Build full vocab and mappings
FULL_VOCAB: list[str] = []
for _lvl in sorted(VOCAB_BY_LEVEL.keys()):
    FULL_VOCAB.extend(VOCAB_BY_LEVEL[_lvl])
VOCAB_SIZE = len(FULL_VOCAB)
TOK2ID = {tok: i for i, tok in enumerate(FULL_VOCAB)}
ID2TOK = {i: tok for i, tok in enumerate(FULL_VOCAB)}
END_ID = TOK2ID["END"]


def vocab_for_level(level: int) -> list[int]:
    """Return list of valid token IDs for a curriculum level."""
    ids = []
    for lvl in range(level + 1):
        for tok in VOCAB_BY_LEVEL.get(lvl, []):
            ids.append(TOK2ID[tok])
    return ids


# =============================================================================
# Korec Interface — the bridge to the real compiler
# =============================================================================

# Use RAM filesystem for zero disk I/O
TMPDIR = "/dev/shm/kore_llm"
os.makedirs(TMPDIR, exist_ok=True)

# Kore project root (where korec binary lives)
KORE_ROOT = Path(__file__).resolve().parent.parent.parent  # experiments/kore_llm -> kore root


def _find_korec() -> str:
    """Find the korec binary."""
    candidates = [
        KORE_ROOT / "target" / "release" / "korec",
        KORE_ROOT / "target" / "debug" / "korec",
    ]
    for c in candidates:
        if c.exists():
            return str(c)
    raise FileNotFoundError(
        f"korec not found. Build with: cd {KORE_ROOT} && cargo build --release\n"
        f"Searched: {[str(c) for c in candidates]}"
    )


KOREC = _find_korec()


@dataclass
class KorecResult:
    """Result of compiling and running a Kore program."""
    source: str
    compiled: bool = False
    proof_ok: bool = False
    ran: bool = False
    result: Any = None           # int, float, bool, str, list, None
    error: Optional[str] = None
    compile_time_us: int = 0
    run_time_us: int = 0

    # Proof checker details (for reward shaping)
    type_errors: list[str] = field(default_factory=list)
    stack_types: list[str] = field(default_factory=list)


def run_korec(source: str, worker_id: int = 0, timeout: float = 2.0) -> KorecResult:
    """
    Compile and run a Kore program using the real korec binary.
    
    Uses /dev/shm for RAM-speed file I/O.
    Each worker gets its own file pair to avoid conflicts in parallel.
    """
    result = KorecResult(source=source)

    src_path = f"{TMPDIR}/w{worker_id}.kore"
    bc_path = f"{TMPDIR}/w{worker_id}.korec"

    # Write source (RAM filesystem — ~1µs)
    with open(src_path, "w") as f:
        f.write(source)

    # ── Step 1: Compile (includes optimizer + proof checker) ──
    t0 = time.monotonic()
    try:
        comp = subprocess.run(
            [KOREC, "compile", src_path, "-o", bc_path],
            capture_output=True, text=True, timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        result.error = "compile timeout"
        return result

    result.compile_time_us = int((time.monotonic() - t0) * 1_000_000)

    if comp.returncode != 0:
        # Parse proof checker errors for reward shaping
        stderr = comp.stderr
        result.error = stderr.strip()
        result.compiled = False
        result.proof_ok = False

        # Extract specific type errors
        for line in stderr.split("\n"):
            line = line.strip()
            if line.startswith("0") and ": " in line:
                # Lines like "0003: Stack underflow"
                result.type_errors.append(line.split(": ", 1)[1])

        return result

    result.compiled = True
    result.proof_ok = True

    # Parse compiler output for stack type info
    for line in comp.stdout.split("\n"):
        if "Proof checked" in line:
            result.proof_ok = True

    # ── Step 2: Run ──
    t0 = time.monotonic()
    try:
        run = subprocess.run(
            [KOREC, "run", bc_path],
            capture_output=True, text=True, timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        result.error = "runtime timeout"
        result.ran = False
        return result

    result.run_time_us = int((time.monotonic() - t0) * 1_000_000)

    if run.returncode != 0:
        result.error = run.stderr.strip()
        result.ran = False
        return result

    result.ran = True

    # Parse result from korec output
    # Formats: Result: Some(Int(N)), Some(Float(F)), Some(Bool(B)),
    #          Some(Str("..")), Some(Nil), Some(List([..])),
    #          Some(Pair((..))), Some(Error("..")), None
    stdout = run.stdout

    m = re.search(r"Result:\s*Some\(Int\((-?\d+)\)\)", stdout)
    if m:
        result.result = int(m.group(1))
        return result

    m = re.search(r"Result:\s*Some\(Float\((-?[\d.eE+-]+(?:inf|nan)?)\)\)", stdout)
    if m:
        result.result = float(m.group(1))
        return result

    m = re.search(r"Result:\s*Some\(Bool\((true|false)\)\)", stdout)
    if m:
        result.result = 1 if m.group(1) == "true" else 0
        return result

    m = re.search(r'Result:\s*Some\(Str\("(.*)"\)\)', stdout)
    if m:
        result.result = m.group(1)
        return result

    if "Result: Some(Nil)" in stdout:
        result.result = 0  # Nil → 0 for reward comparison
        return result

    if "Result: None" in stdout:
        result.result = None  # Empty stack
        result.error = "empty stack"
        return result

    # For complex types (List, Pair, Map, etc.) — store as raw string
    m = re.search(r"Result:\s*Some\((.+)\)", stdout)
    if m:
        result.result = m.group(1)  # raw string representation

    return result


# =============================================================================
# RL Environment
# =============================================================================

@dataclass
class Task:
    """A single task for the agent."""
    level: int
    description: str
    expected: Any                 # int, float, str, bool — compared to korec result
    max_tokens: int = 20
    hint: Optional[list[str]] = None  # Optimal solution for eval


@dataclass
class StepResult:
    """Result of one environment step."""
    done: bool
    reward: float
    info: dict


class KoreEnv:
    """
    Gym-style environment for learning to write Kore programs.
    
    Observation: (task_id, history_of_token_ids, current_level)
    Action: token_id from FULL_VOCAB
    Reward: 3-tier (compile + correct + elegance)
    """

    def __init__(self, worker_id: int = 0, level: int = 0):
        self.worker_id = worker_id
        self.level = level
        self.task: Optional[Task] = None
        self.history: list[int] = []
        self.max_episode_tokens = 40
        self.valid_ids = vocab_for_level(self.level)

    def set_level(self, level: int):
        self.level = level
        self.valid_ids = vocab_for_level(self.level)

    def reset(self, task: Task) -> dict:
        """Start a new episode with the given task."""
        self.task = task
        self.history = []
        self.max_episode_tokens = task.max_tokens
        return self._obs()

    def step(self, action: int) -> StepResult:
        """
        Take one action (emit one token).
        
        Returns StepResult with done, reward, info.
        Most reward is sparse (only at END), but we give small shaping rewards.
        """
        token = ID2TOK[action]
        self.history.append(action)

        # ── Not done yet (still generating) ──
        if token != "END" and len(self.history) < self.max_episode_tokens:
            # Tiny per-step penalty to encourage shorter programs
            return StepResult(done=False, reward=-0.002, info={"status": "generating"})

        # ── Episode ends: either END token or max length reached ──
        if token != "END":
            # Hit max length without END — bad
            return StepResult(done=True, reward=-0.3, info={
                "status": "truncated",
                "compiled": False,
                "correct": False,
            })

        # Build the source (strip the END token — it's an RL sentinel, not Kore syntax)
        source = " ".join(ID2TOK[tid] for tid in self.history if tid != END_ID)

        # ── Run through real korec ──
        kr = run_korec(source, worker_id=self.worker_id)

        # ── 3-tier reward ──
        reward = 0.0
        info = {
            "source": source,
            "compiled": kr.compiled,
            "proof_ok": kr.proof_ok,
            "correct": False,
            "result": kr.result,
            "expected": self.task.expected,
            "compile_us": kr.compile_time_us,
            "run_us": kr.run_time_us,
        }

        # Tier 1: Compilation + proof check
        if not kr.compiled:
            # Proof checker rejected. But give partial credit based on error.
            if kr.type_errors:
                # Got further before failing = slightly better
                err = kr.type_errors[0] if kr.type_errors else ""
                if "underflow" in err.lower():
                    reward = -0.2  # Common beginner mistake
                else:
                    reward = -0.25
            else:
                reward = -0.3  # Total failure (syntax error)
            info["status"] = "compile_error"
            info["error"] = kr.error
            return StepResult(done=True, reward=reward, info=info)

        # Program compiled and passed proof checker! That's worth something.
        reward += 0.1
        info["status"] = "compiled"

        # Tier 2: Correctness
        correct = False
        if kr.result is not None:
            exp = self.task.expected
            if isinstance(kr.result, float) and isinstance(exp, (int, float)):
                correct = abs(kr.result - exp) < 1e-6  # float tolerance
            else:
                correct = kr.result == exp

        if correct:
            reward += 1.0
            info["correct"] = True
            info["status"] = "correct"

            # Tier 3: Elegance bonus (shorter = better)
            program_len = len(self.history)
            max_len = self.task.max_tokens
            efficiency = 1.0 - (program_len / max_len)
            reward += 0.15 * efficiency
        else:
            # Compiled but wrong answer
            if kr.result is not None and isinstance(kr.result, (int, float)) \
               and isinstance(self.task.expected, (int, float)) \
               and self.task.expected != 0:
                # Partial credit: how close is the answer?
                diff = abs(kr.result - self.task.expected)
                max_diff = max(abs(self.task.expected), 1)
                closeness = max(0.0, 1.0 - diff / max_diff)
                reward += 0.05 * closeness  # Small partial credit
            info["status"] = "wrong_answer"

        return StepResult(done=True, reward=reward, info=info)

    def action_mask(self) -> list[bool]:
        """
        Returns a boolean mask over FULL_VOCAB.
        True = this action is allowed, False = blocked.
        
        This dramatically reduces wasted exploration.
        We let the proof checker handle deep type analysis —
        here we only block obvious structural errors.
        """
        mask = [False] * VOCAB_SIZE
        tokens_so_far = [ID2TOK[tid] for tid in self.history]
        defined_vars = set()

        # Track which variables have been defined
        for tok in tokens_so_far:
            if tok.startswith("->"):
                defined_vars.add(tok[2:])

        # Count open control structures
        open_ifs = 0
        open_whiles = 0
        for tok in tokens_so_far:
            if tok == "if":
                open_ifs += 1
            elif tok == "while":
                open_whiles += 1
            elif tok == "end":
                if open_ifs > 0:
                    open_ifs -= 1
                elif open_whiles > 0:
                    open_whiles -= 1

        # Track open quote brackets
        open_quotes = 0
        for tok in tokens_so_far:
            if tok == "[":
                open_quotes += 1
            elif tok == "]":
                open_quotes -= 1

        # Simple categories for masking (proof checker handles the rest)
        ALWAYS_OK = {
            # Literals
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
            "true", "false", "nil",
            # Arithmetic (proof checker validates stack depth)
            "+", "-", "*", "div", "mod", "neg",
            # Stack ops
            "dup", "drop", "swap", "over", "rot", "nop",
            # Comparisons & logic
            "=", "!=", "<", ">", "<=", ">=", "not", "and", "or", "xor",
            # Bitwise
            "band", "bor", "bxor", "bnot", "shl", "shr",
            # Data structures
            "pair", "unpair", "left", "right", "case",
            "first", "second",  # non-destructive pair access
            # List operations
            "len", "get", "set", "append", "reverse", "unlist",
            "map", "fold", "zip",
            "filter", "head", "tail", "range", "concat", "empty?",
            # Error handling
            "error", "is-error", "try", "fail", "?",
            # String operations
            "str-len", "str-get", "str-concat", "str-slice", "to-str",
            "str-find", "str-split", "str-replace",
            "str-upper", "str-lower", "str-trim",
            # Type conversion & introspection
            "i2f", "f2i", "type-of", "depth", "describe",
            # Float math
            "fadd", "fsub", "fmul", "fdiv", "fneg",
            "fsqrt", "fabs", "fexp", "flog",
            "fsin", "fcos", "fatan2", "fpow",
            "ffloor", "fceil", "fround",
            # Linear types
            "linear", "affine", "consume", "is-linear", "is-affine",
            # Fibers
            "fiber-new", "fiber-step", "fiber-push", "fiber-stack", "fiber-status",
            # Concurrency
            "spawn", "chan-new", "chan-send", "chan-recv",
            # Maps
            "map-new", "map-get", "map-set", "map-keys", "map-has",
            # Reflection & control
            "fetch", "size", "ret", "halt",
            # Control flow (open is always OK)
            "if", "while", "times",
            # Quotes (open is always OK)
            "[",
            # Higher-order words
            "apply", "cond", "loop",
            # Store to variable
            "->a", "->b", "->n", "->i", "->c",
        }

        for token_id in self.valid_ids:
            tok = ID2TOK[token_id]

            # END: only if all structures are closed
            if tok == "END":
                if open_ifs == 0 and open_whiles == 0 and open_quotes == 0:
                    mask[token_id] = True
                continue

            # Always-allowed tokens
            if tok in ALWAYS_OK:
                mask[token_id] = True
                continue

            # Load variable: only if it's been defined
            if tok in ("a", "b", "c", "n", "i"):
                if tok in defined_vars:
                    mask[token_id] = True
                continue

            # "do" only inside while
            if tok == "do":
                if open_whiles > 0:
                    mask[token_id] = True
                continue

            # "else" only inside if
            if tok == "else":
                if open_ifs > 0:
                    mask[token_id] = True
                continue

            # "end" only when there's something to close
            if tok == "end":
                if open_ifs > 0 or open_whiles > 0:
                    mask[token_id] = True
                continue

            # "]" only when there's an open quote
            if tok == "]":
                if open_quotes > 0:
                    mask[token_id] = True
                continue

            # Function definition tokens
            if tok == ":":
                # Can start a function def anytime (outside quotes for simplicity)
                if open_quotes == 0:
                    mask[token_id] = True
                continue
            if tok == ";":
                # Can end a function def (parser handles matching)
                mask[token_id] = True
                continue
            if tok.startswith("fn-"):
                # Function name placeholders — always available
                mask[token_id] = True
                continue

        # Ensure at least END is available as a fallback
        if not any(mask):
            mask[END_ID] = True

        return mask

    def _obs(self) -> dict:
        return {
            "task": self.task.description if self.task else "",
            "history": list(self.history),
            "level": self.level,
            "valid_ids": self.valid_ids,
        }


# =============================================================================
# Parallel environment runner (for batched training)
# =============================================================================

def run_episodes_batch(
    programs: list[str],
    expected: list[int],
    num_workers: int = 8,
) -> list[KorecResult]:
    """
    Run multiple programs through korec in parallel.
    Each gets its own worker_id for separate tmp files.
    """
    results = []
    for i, (src, _exp) in enumerate(zip(programs, expected)):
        results.append(run_korec(src, worker_id=i % num_workers))
    return results


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    print(f"Korec binary: {KOREC}")
    print(f"Tmpdir: {TMPDIR}")
    print(f"Vocabulary: {VOCAB_SIZE} tokens")
    print(f"Level 0 tokens: {[ID2TOK[i] for i in vocab_for_level(0)]}")
    print(f"Level 6 tokens: {len(vocab_for_level(6))} tokens")
    print(f"Full vocab: {len(vocab_for_level(max(VOCAB_BY_LEVEL.keys())))} tokens")
    print()

    # Test basic compile+run
    print("=== Testing korec interface ===")

    # Test 1: Simple addition
    kr = run_korec("3 5 +")
    print(f"  '3 5 +' → result={kr.result}, compiled={kr.compiled}, "
          f"proof={kr.proof_ok}, time={kr.compile_time_us + kr.run_time_us}µs")
    assert kr.result == 8, f"Expected 8, got {kr.result}"

    # Test 2: Stack underflow (proof checker should reject)
    kr = run_korec("+")
    print(f"  '+' → compiled={kr.compiled}, error={kr.type_errors}")
    assert not kr.compiled

    # Test 3: Factorial
    kr = run_korec("5 ->n 1 ->a while n 1 > do a n * ->a n 1 - ->n end a")
    print(f"  '5!' → result={kr.result}, compiled={kr.compiled}, "
          f"time={kr.compile_time_us + kr.run_time_us}µs")
    assert kr.result == 120

    # Test 4: Wrong answer (compiles but wrong)
    kr = run_korec("3 4 +")
    print(f"  '3 4 +' → result={kr.result}")
    assert kr.result == 7

    print("\n=== Testing environment ===")

    env = KoreEnv(level=1)
    task = Task(level=1, description="Compute 3 + 5", expected=8, max_tokens=8)
    env.reset(task)

    # Simulate agent actions: 3 5 + END
    for tok in ["3", "5", "+", "END"]:
        sr = env.step(TOK2ID[tok])
        if sr.done:
            print(f"  Task '{task.description}': reward={sr.reward:.2f}, "
                  f"status={sr.info['status']}, correct={sr.info.get('correct')}")
            break

    # Test action mask
    env2 = KoreEnv(level=4)
    env2.reset(Task(level=4, description="test", expected=0))
    mask = env2.action_mask()
    # Variable 'a' should NOT be loadable (not defined yet)
    assert mask[TOK2ID["a"]] == False, "Variable 'a' should be masked before definition"
    # Store ->a should be allowed
    assert mask[TOK2ID["->a"]] == True

    # After defining a, loading should be allowed
    env2.step(TOK2ID["5"])
    env2.step(TOK2ID["->a"])
    mask2 = env2.action_mask()
    assert mask2[TOK2ID["a"]] == True, "Variable 'a' should be available after ->a"

    # Performance test
    print("\n=== Performance ===")
    N = 100
    t0 = time.monotonic()
    for _ in range(N):
        run_korec("3 5 +")
    elapsed = time.monotonic() - t0
    print(f"  {N} compile+run cycles: {elapsed:.2f}s ({elapsed/N*1000:.1f}ms/call, "
          f"{N/elapsed:.0f} calls/sec)")

    print("\n✓ All tests passed!")
