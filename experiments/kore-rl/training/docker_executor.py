"""
Kore Runtime Client - Docker CLI Execution
===========================================

Executes Kore programs directly via `docker exec` for lower latency.
No HTTP overhead - just subprocess calls to the container.
"""

import subprocess
import json
import os
from dataclasses import dataclass, field
from typing import List, Optional, Any, Dict, Tuple
from concurrent.futures import ThreadPoolExecutor, as_completed
import threading
import time


@dataclass
class TraceEntry:
    """Single execution step"""
    step: int
    op: str
    op_type: str  # "push" or "call"
    stack_before: List[Any]
    stack_after: List[Any]
    success: bool
    error: Optional[str] = None


@dataclass
class ExecuteResult:
    """Result of executing a Kore program"""
    success: bool
    final_stack: List[Any]
    trace: List[TraceEntry]
    error: Optional[str] = None
    error_at_step: Optional[int] = None
    steps_executed: int = 0
    execution_time_ms: int = 0


class KoreDockerExecutor:
    """
    Execute Kore programs via Docker CLI.
    
    Much faster than HTTP for high-throughput training:
    - No TCP/HTTP overhead
    - Direct subprocess execution
    - Parallel execution with thread pool
    
    Usage:
        executor = KoreDockerExecutor("kore-runtime")
        result = executor.execute("3 4 add")
        print(result.final_stack)  # [7]
    """
    
    def __init__(
        self,
        container_name: str = "kore-runtime",
        kore_binary: str = "/usr/local/bin/kore",
        timeout: float = 5.0,
        max_workers: int = 32,
    ):
        self.container_name = container_name
        self.kore_binary = kore_binary
        self.timeout = timeout
        self.executor = ThreadPoolExecutor(max_workers=max_workers)
        self._check_container()
    
    def _check_container(self):
        """Verify container is running"""
        try:
            result = subprocess.run(
                ["docker", "inspect", "-f", "{{.State.Running}}", self.container_name],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode != 0 or "true" not in result.stdout:
                raise RuntimeError(f"Container '{self.container_name}' is not running")
        except subprocess.TimeoutExpired:
            raise RuntimeError("Docker command timed out")
        except FileNotFoundError:
            raise RuntimeError("Docker not found in PATH")
    
    def execute(
        self,
        program: str,
        initial_stack: List[Any] = None,
        max_steps: int = 10000,
        trace: bool = True,
    ) -> ExecuteResult:
        """
        Execute a Kore program in the container.
        
        Uses `docker exec` with JSON output for structured results.
        """
        start = time.time()
        
        # Build command - pass program via stdin to avoid shell escaping issues
        cmd = [
            "docker", "exec", "-i",
            self.container_name,
            self.kore_binary,
            "--json",  # JSON output
            "--trace" if trace else "--no-trace",
            "--max-steps", str(max_steps),
            "-",  # Read from stdin
        ]
        
        try:
            result = subprocess.run(
                cmd,
                input=program,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
            
            elapsed_ms = int((time.time() - start) * 1000)
            
            if result.returncode != 0 and not result.stdout:
                # Complete failure
                return ExecuteResult(
                    success=False,
                    final_stack=[],
                    trace=[],
                    error=result.stderr.strip() or f"Exit code {result.returncode}",
                    execution_time_ms=elapsed_ms,
                )
            
            # Parse JSON output
            try:
                data = json.loads(result.stdout)
                return self._parse_json_result(data, elapsed_ms)
            except json.JSONDecodeError:
                # Fallback: parse simple output
                return self._parse_simple_output(result.stdout, result.stderr, elapsed_ms)
                
        except subprocess.TimeoutExpired:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error="Execution timeout",
                execution_time_ms=int(self.timeout * 1000),
            )
        except Exception as e:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error=str(e),
                execution_time_ms=int((time.time() - start) * 1000),
            )
    
    def execute_batch(
        self,
        programs: List[str],
        max_steps: int = 10000,
        trace: bool = False,  # Disable trace for batch (faster)
    ) -> List[ExecuteResult]:
        """
        Execute multiple programs in parallel.
        
        Uses thread pool for concurrent docker exec calls.
        """
        futures = []
        for program in programs:
            future = self.executor.submit(
                self.execute, program, None, max_steps, trace
            )
            futures.append(future)
        
        results = []
        for future in futures:
            try:
                results.append(future.result())
            except Exception as e:
                results.append(ExecuteResult(
                    success=False,
                    final_stack=[],
                    trace=[],
                    error=str(e),
                ))
        
        return results
    
    def _parse_json_result(self, data: Dict, elapsed_ms: int) -> ExecuteResult:
        """Parse JSON output from kore binary"""
        trace_entries = []
        for entry in data.get("trace", []):
            trace_entries.append(TraceEntry(
                step=entry.get("step", 0),
                op=entry.get("op", ""),
                op_type=entry.get("op_type", ""),
                stack_before=entry.get("stack_before", []),
                stack_after=entry.get("stack_after", []),
                success=entry.get("success", True),
                error=entry.get("error"),
            ))
        
        return ExecuteResult(
            success=data.get("success", False),
            final_stack=data.get("final_stack", []),
            trace=trace_entries,
            error=data.get("error"),
            error_at_step=data.get("error_at_step"),
            steps_executed=data.get("steps_executed", 0),
            execution_time_ms=elapsed_ms,
        )
    
    def _parse_simple_output(
        self, stdout: str, stderr: str, elapsed_ms: int
    ) -> ExecuteResult:
        """Parse simple text output (fallback)"""
        # Simple format: stack values one per line, or error
        lines = stdout.strip().split("\n") if stdout.strip() else []
        
        if stderr:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error=stderr.strip(),
                execution_time_ms=elapsed_ms,
            )
        
        # Try to parse stack values
        stack = []
        for line in lines:
            line = line.strip()
            if not line:
                continue
            try:
                # Try int
                stack.append(int(line))
            except ValueError:
                try:
                    # Try float
                    stack.append(float(line))
                except ValueError:
                    # String
                    if line.lower() == "true":
                        stack.append(True)
                    elif line.lower() == "false":
                        stack.append(False)
                    elif line.lower() == "null":
                        stack.append(None)
                    else:
                        stack.append(line)
        
        return ExecuteResult(
            success=True,
            final_stack=stack,
            trace=[],
            execution_time_ms=elapsed_ms,
            steps_executed=len(stack),  # Approximate
        )
    
    def health_check(self) -> bool:
        """Check if container is healthy"""
        try:
            result = subprocess.run(
                ["docker", "exec", self.container_name, "echo", "ok"],
                capture_output=True,
                text=True,
                timeout=2,
            )
            return result.returncode == 0
        except:
            return False
    
    def close(self):
        """Shutdown thread pool"""
        self.executor.shutdown(wait=False)
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()


class KoreLocalExecutor:
    """
    Execute Kore programs directly (no container).
    
    For maximum speed when training on same machine as runtime.
    Uses the local kore-train binary directly.
    """
    
    def __init__(
        self,
        kore_binary: str = "kore-train",
        timeout: float = 5.0,
        max_workers: int = 32,
    ):
        self.kore_binary = kore_binary
        self.timeout = timeout
        self.executor = ThreadPoolExecutor(max_workers=max_workers)
        self._check_binary()
    
    def _check_binary(self):
        """Verify kore binary exists"""
        try:
            result = subprocess.run(
                [self.kore_binary, "--help"],
                capture_output=True,
                text=True,
                timeout=2,
            )
            # kore-train outputs help to stderr, that's fine
        except FileNotFoundError:
            raise RuntimeError(f"Kore binary not found: {self.kore_binary}")
    
    def execute(
        self,
        program: str,
        initial_stack: List[Any] = None,
        max_steps: int = 10000,
        trace: bool = True,
    ) -> ExecuteResult:
        """Execute a Kore program locally"""
        start = time.time()
        
        cmd = [
            self.kore_binary,
            "--trace" if trace else "--no-trace",
            "--max-steps", str(max_steps),
        ]
        
        try:
            result = subprocess.run(
                cmd,
                input=program,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
            
            elapsed_ms = int((time.time() - start) * 1000)
            
            if result.returncode != 0 and not result.stdout:
                return ExecuteResult(
                    success=False,
                    final_stack=[],
                    trace=[],
                    error=result.stderr.strip() or f"Exit code {result.returncode}",
                    execution_time_ms=elapsed_ms,
                )
            
            try:
                data = json.loads(result.stdout)
                return self._parse_json_result(data, elapsed_ms)
            except json.JSONDecodeError:
                return ExecuteResult(
                    success=False,
                    final_stack=[],
                    trace=[],
                    error=f"Invalid JSON: {result.stdout[:100]}",
                    execution_time_ms=elapsed_ms,
                )
                
        except subprocess.TimeoutExpired:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error="Execution timeout",
                execution_time_ms=int(self.timeout * 1000),
            )
        except Exception as e:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error=str(e),
                execution_time_ms=int((time.time() - start) * 1000),
            )
    
    def _parse_json_result(self, data: Dict, elapsed_ms: int) -> ExecuteResult:
        """Parse JSON output from kore-train binary"""
        trace_entries = []
        for entry in data.get("trace", []):
            trace_entries.append(TraceEntry(
                step=entry.get("step", 0),
                op=entry.get("op", ""),
                op_type=entry.get("op_type", ""),
                stack_before=entry.get("stack_before", []),
                stack_after=entry.get("stack_after", []),
                success=entry.get("success", True),
                error=entry.get("error"),
            ))
        
        return ExecuteResult(
            success=data.get("success", False),
            final_stack=data.get("final_stack", []),
            trace=trace_entries,
            error=data.get("error"),
            error_at_step=data.get("error_at_step"),
            steps_executed=data.get("steps_executed", 0),
            execution_time_ms=elapsed_ms,
        )
    
    def execute_batch(
        self,
        programs: List[str],
        max_steps: int = 10000,
        trace: bool = False,
    ) -> List[ExecuteResult]:
        """Execute multiple programs in parallel"""
        futures = [
            self.executor.submit(self.execute, prog, None, max_steps, trace)
            for prog in programs
        ]
        return [f.result() for f in futures]
    
    def health_check(self) -> bool:
        try:
            result = subprocess.run(
                [self.kore_binary],
                input="1",
                capture_output=True,
                text=True,
                timeout=2,
            )
            return result.returncode == 0
        except:
            return False
    
    def close(self):
        self.executor.shutdown(wait=False)


class KoreInProcessExecutor:
    """
    Execute Kore programs in-process using Python simulation.
    
    Fastest option - no subprocess overhead at all.
    Uses the Python Kore simulator for training.
    """
    
    def __init__(self, max_steps: int = 10000):
        self.max_steps = max_steps
        # Import the Python simulator
        try:
            from kore_sim import KoreSimulator
            self.sim = KoreSimulator()
        except ImportError:
            self.sim = None
    
    def execute(
        self,
        program: str,
        initial_stack: List[Any] = None,
        max_steps: int = None,
        trace: bool = True,
    ) -> ExecuteResult:
        """Execute using Python simulator"""
        if self.sim is None:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error="Python simulator not available",
            )
        
        start = time.time()
        
        try:
            result = self.sim.execute(
                program,
                initial_stack=initial_stack or [],
                max_steps=max_steps or self.max_steps,
                trace=trace,
            )
            
            elapsed_ms = int((time.time() - start) * 1000)
            
            trace_entries = []
            if trace and hasattr(result, 'trace'):
                for entry in result.trace:
                    trace_entries.append(TraceEntry(
                        step=entry.get('step', 0),
                        op=entry.get('op', ''),
                        op_type=entry.get('op_type', ''),
                        stack_before=entry.get('stack_before', []),
                        stack_after=entry.get('stack_after', []),
                        success=entry.get('success', True),
                        error=entry.get('error'),
                    ))
            
            return ExecuteResult(
                success=result.success,
                final_stack=result.stack,
                trace=trace_entries,
                error=result.error,
                error_at_step=result.error_at_step if hasattr(result, 'error_at_step') else None,
                steps_executed=result.steps if hasattr(result, 'steps') else 0,
                execution_time_ms=elapsed_ms,
            )
        except Exception as e:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error=str(e),
                execution_time_ms=int((time.time() - start) * 1000),
            )
    
    def execute_batch(
        self,
        programs: List[str],
        max_steps: int = None,
        trace: bool = False,
    ) -> List[ExecuteResult]:
        """Execute multiple programs (sequential - simulator is fast)"""
        return [self.execute(prog, None, max_steps, trace) for prog in programs]
    
    def health_check(self) -> bool:
        return self.sim is not None
    
    def close(self):
        pass


def create_executor(
    mode: str = "docker",
    **kwargs
) -> KoreDockerExecutor | KoreLocalExecutor | KoreInProcessExecutor:
    """
    Create appropriate executor based on mode.
    
    Args:
        mode: "docker", "local", or "inprocess"
        **kwargs: Executor-specific arguments
    
    Returns:
        Configured executor instance
    """
    if mode == "docker":
        return KoreDockerExecutor(**kwargs)
    elif mode == "local":
        return KoreLocalExecutor(**kwargs)
    elif mode == "inprocess":
        return KoreInProcessExecutor(**kwargs)
    else:
        raise ValueError(f"Unknown mode: {mode}")


# Backwards compatibility
KoreRuntimeClient = KoreDockerExecutor


if __name__ == "__main__":
    # Quick benchmark
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["docker", "local", "inprocess"], default="docker")
    parser.add_argument("--container", default="kore-runtime")
    parser.add_argument("--binary", default="kore")
    parser.add_argument("--n", type=int, default=100)
    args = parser.parse_args()
    
    print(f"Benchmarking {args.mode} executor with {args.n} executions...")
    
    if args.mode == "docker":
        executor = KoreDockerExecutor(container_name=args.container)
    elif args.mode == "local":
        executor = KoreLocalExecutor(kore_binary=args.binary)
    else:
        executor = KoreInProcessExecutor()
    
    programs = [f"{i} {i+1} add" for i in range(args.n)]
    
    start = time.time()
    results = executor.execute_batch(programs, trace=False)
    elapsed = time.time() - start
    
    correct = sum(1 for i, r in enumerate(results) if r.success and r.final_stack == [2*i + 1])
    
    print(f"Time: {elapsed:.2f}s ({args.n / elapsed:.1f} programs/sec)")
    print(f"Correct: {correct}/{args.n}")
    
    executor.close()
