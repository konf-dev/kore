"""
Experiment Runner — korec serve pipe interface.

Manages a persistent `korec serve` subprocess. Sends Kore source programs,
receives JSON results. Zero OS process overhead per eval.

Measured: ~50,000 programs/sec on single core.
"""

from __future__ import annotations
import subprocess
import json
import os
import time
import select
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional, Any


KORE_ROOT = Path(__file__).resolve().parent.parent.parent


def _find_korec() -> str:
    for p in [KORE_ROOT / "target/release/korec", KORE_ROOT / "target/debug/korec"]:
        if p.exists():
            return str(p)
    raise FileNotFoundError(f"korec not found. Run: cd {KORE_ROOT} && cargo build --release")


KOREC = _find_korec()


@dataclass
class EvalResult:
    """Result from evaluating one program via korec serve."""
    source: str
    ok: bool = False
    stage: str = ""              # "compile", "typecheck", "run"
    result: Optional[str] = None # Raw result string e.g. "Int(8)"
    error: Optional[str] = None
    steps: int = 0
    compile_ok: bool = False
    typecheck_ok: bool = False
    type_errors: list[str] = field(default_factory=list)
    stack: list[str] = field(default_factory=list)
    elapsed_us: int = 0          # Wall-clock microseconds for this eval

    # Parsed value (for reward computation)
    value: Any = None            # int, float, bool, str, list, None

    def parse_value(self):
        """Parse the result string into a Python value."""
        if self.result is None:
            self.value = None
            return

        r = self.result
        # Int(N)
        if r.startswith("Int(") and r.endswith(")"):
            try:
                self.value = int(r[4:-1])
            except ValueError:
                self.value = r
            return

        # Float(F)
        if r.startswith("Float(") and r.endswith(")"):
            try:
                self.value = float(r[6:-1])
            except ValueError:
                self.value = r
            return

        # Bool(true/false)
        if r == "Bool(true)":
            self.value = True
            return
        if r == "Bool(false)":
            self.value = False
            return

        # Str("...")
        if r.startswith('Str("') and r.endswith('")'):
            self.value = r[5:-2]  # Extract string content
            return

        # Nil
        if r == "Nil":
            self.value = None
            return

        # List, Pair, etc. — keep as raw string
        self.value = r


class ServeRunner:
    """
    Manages a persistent `korec serve` process.

    Usage:
        runner = ServeRunner()
        result = runner.eval("3 5 +")
        assert result.value == 8
        results = runner.eval_batch(["3 5 +", "4 2 *", "bad"])
        runner.close()
    """

    def __init__(self, max_steps: int = 100_000, timeout: float = 2.0, prelude: Optional[str] = None):
        self.max_steps = max_steps
        self.timeout = timeout
        self.prelude = prelude  # path to a prelude .kore file
        self.proc: Optional[subprocess.Popen] = None
        self._start()
        self._eval_count = 0
        self._total_us = 0

    def _start(self):
        """Start (or restart) the korec serve process."""
        if self.proc is not None:
            try:
                self.proc.kill()
                self.proc.wait(timeout=1)
            except Exception:
                pass

        cmd = [KOREC, "serve"]
        if self.prelude:
            cmd.extend(["--prelude", self.prelude])

        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=0,  # unbuffered for low latency
        )

    def _ensure_alive(self):
        if self.proc is None or self.proc.poll() is not None:
            self._start()

    def eval(self, source: str, max_steps: Optional[int] = None) -> EvalResult:
        """
        Evaluate one Kore program. Returns EvalResult with parsed value.

        If max_steps is specified, sends JSON input; otherwise plain text.
        """
        self._ensure_alive()
        steps = max_steps if max_steps is not None else self.max_steps
        result = EvalResult(source=source)

        # Build input line — use JSON when custom max_steps or multi-line source
        if steps != 100_000 or "\n" in source:
            # Use JSON protocol (handles newlines in source via \n escape)
            line = json.dumps({"source": source, "max_steps": steps}) + "\n"
        else:
            line = source + "\n"

        t0 = time.monotonic()
        try:
            self.proc.stdin.write(line.encode())
            self.proc.stdin.flush()

            # Read with timeout (safety net for edge cases)
            ready, _, _ = select.select([self.proc.stdout], [], [], self.timeout)
            if not ready:
                # Timeout — kill and restart
                result.error = "timeout"
                result.stage = "timeout"
                self._start()
                return result

            response_line = self.proc.stdout.readline()
            if not response_line:
                # Process died
                result.error = "serve process died"
                self._start()
                return result

            elapsed_us = int((time.monotonic() - t0) * 1_000_000)
            result.elapsed_us = elapsed_us
            self._eval_count += 1
            self._total_us += elapsed_us

            # Parse JSON response
            data = json.loads(response_line)
            result.ok = data.get("ok", False)
            result.stage = data.get("stage", "")
            result.result = data.get("result")
            result.error = data.get("error")
            result.steps = data.get("steps", 0)
            result.compile_ok = data.get("compile_ok", False)
            result.typecheck_ok = data.get("typecheck_ok", False)
            result.type_errors = data.get("type_errors", [])
            result.stack = data.get("stack", data.get("partial_stack", []))
            result.parse_value()

        except Exception as e:
            result.error = str(e)
            self._start()

        return result

    def eval_batch(self, programs: list[str], max_steps: Optional[int] = None) -> list[EvalResult]:
        """Evaluate multiple programs sequentially through the same serve process."""
        return [self.eval(src, max_steps) for src in programs]

    def stats(self) -> dict:
        return {
            "eval_count": self._eval_count,
            "total_us": self._total_us,
            "avg_us": self._total_us / max(1, self._eval_count),
            "throughput": self._eval_count / max(1, self._total_us / 1_000_000),
        }

    def close(self):
        if self.proc is not None:
            try:
                self.proc.stdin.close()
                self.proc.wait(timeout=2)
            except Exception:
                self.proc.kill()
            self.proc = None

    def __del__(self):
        self.close()


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    runner = ServeRunner()

    print(f"korec binary: {KOREC}")
    print()

    # Basic tests
    print("=== Correctness Tests ===")

    r = runner.eval("3 5 +")
    assert r.ok and r.value == 8, f"Expected 8, got {r}"
    print(f"  '3 5 +'          → {r.value} ✓  ({r.elapsed_us}µs)")

    r = runner.eval("bad")
    assert not r.ok and r.stage == "compile", f"Expected compile error: {r}"
    print(f"  'bad'             → compile error ✓")

    r = runner.eval("+")
    assert not r.ok and r.stage == "typecheck", f"Expected typecheck error: {r}"
    print(f"  '+'               → typecheck error ✓")

    r = runner.eval("[ true ] loop", max_steps=500)
    assert not r.ok and "step limit" in r.error, f"Expected step limit: {r}"
    print(f"  infinite loop     → step limit ✓  (steps={r.steps})")

    r = runner.eval("true")
    assert r.ok and r.value is True, f"Expected True, got {r.value}"
    print(f"  'true'            → {r.value} ✓")

    r = runner.eval("3.14")
    assert r.ok and abs(r.value - 3.14) < 0.001, f"Expected 3.14, got {r.value}"
    print(f"  '3.14'            → {r.value} ✓")

    r = runner.eval("( 1 2 3 )")
    assert r.ok and "List" in r.result, f"Expected List, got {r.result}"
    print(f"  '( 1 2 3 )'       → {r.result} ✓")

    # Batch test
    print("\n=== Throughput Test ===")
    N = 1000
    programs = ["3 5 +"] * N
    t0 = time.monotonic()
    results = runner.eval_batch(programs)
    elapsed = time.monotonic() - t0
    ok_count = sum(1 for r in results if r.ok)
    print(f"  {N} programs in {elapsed*1000:.1f}ms → {N/elapsed:.0f} prog/sec")
    print(f"  {ok_count}/{N} succeeded")

    s = runner.stats()
    print(f"  Avg latency: {s['avg_us']:.0f}µs")

    runner.close()
    print("\n✓ All tests passed!")
