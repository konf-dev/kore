"""
Kore Executor for FMCTS.

Wraps the real Kore runtime (Docker or local binary).
Uses the same infrastructure as kore-rl for consistency.
"""

import subprocess
import json
import os
from dataclasses import dataclass, field
from typing import List, Optional, Any, Dict, Tuple
from concurrent.futures import ThreadPoolExecutor
import time
import logging

logger = logging.getLogger(__name__)


@dataclass
class TraceEntry:
    """Single execution step from Kore trace."""
    step: int
    op: str
    op_type: str  # "push" or "call"
    stack_before: List[Any]
    stack_after: List[Any]
    success: bool
    error: Optional[str] = None
    
    def to_dict(self) -> Dict:
        return {
            "step": self.step,
            "op": self.op,
            "op_type": self.op_type,
            "stack_before": self.stack_before,
            "stack_after": self.stack_after,
            "success": self.success,
            "error": self.error,
        }


@dataclass
class ExecuteResult:
    """Result of executing a Kore program."""
    success: bool
    final_stack: List[Any]
    trace: List[TraceEntry]
    error: Optional[str] = None
    error_at_step: Optional[int] = None
    steps_executed: int = 0
    execution_time_ms: int = 0
    
    def to_dict(self) -> Dict:
        return {
            "success": self.success,
            "final_stack": self.final_stack,
            "trace": [t.to_dict() for t in self.trace],
            "error": self.error,
            "error_at_step": self.error_at_step,
            "steps_executed": self.steps_executed,
            "execution_time_ms": self.execution_time_ms,
        }


class KoreExecutor:
    """
    Execute Kore programs using the real runtime.
    
    Default: Local binary (fastest, no Docker overhead)
    Alternative: Docker container (for sandboxing)
    
    Uses direct CLI execution via subprocess, NOT HTTP.
    """
    
    def __init__(
        self,
        use_docker: bool = False,
        container_name: str = "kore-runtime",
        kore_binary: str = None,  # Auto-detect
        timeout: float = 5.0,
        max_workers: int = 32,
    ):
        self.use_docker = use_docker
        self.container_name = container_name
        self.timeout = timeout
        self.executor = ThreadPoolExecutor(max_workers=max_workers)
        
        # Auto-detect local binary path
        if kore_binary is None:
            # Try to find kore-train in the project
            project_binary = os.path.join(
                os.path.dirname(__file__),
                "../../../target/release/kore-train"
            )
            if os.path.exists(project_binary):
                self.kore_binary = os.path.abspath(project_binary)
            else:
                self.kore_binary = "kore-train"
        else:
            self.kore_binary = kore_binary
        
        # Stats
        self.total_executions = 0
        self.total_time_ms = 0
        self.cache_hits = 0
        
        # Simple cache for repeated executions
        self._cache: Dict[str, ExecuteResult] = {}
        self._cache_max_size = 100_000
        
        # Verify binary exists
        if not self.use_docker:
            if not os.path.exists(self.kore_binary):
                logger.warning(f"Kore binary not found: {self.kore_binary}")
                logger.info("Build with: cargo build --release -p kore-train")
            else:
                logger.info(f"Using local Kore binary: {self.kore_binary}")
    
    def execute(
        self,
        program: str,
        max_steps: int = 10000,
        trace: bool = True,
        use_cache: bool = True,
    ) -> ExecuteResult:
        """
        Execute a Kore program and return the result.
        
        This is the core method for FMCTS - every MCTS simulation
        calls this to get the real execution result.
        """
        # Check cache
        cache_key = f"{program}:{max_steps}:{trace}"
        if use_cache and cache_key in self._cache:
            self.cache_hits += 1
            return self._cache[cache_key]
        
        start = time.time()
        self.total_executions += 1
        
        # Build command - direct CLI, no HTTP
        if self.use_docker:
            cmd = [
                "docker", "exec", "-i",
                self.container_name,
                "kore-train",
            ]
        else:
            cmd = [self.kore_binary]
        
        # Add trace flag
        if trace:
            cmd.append("--trace")
        else:
            cmd.append("--no-trace")
        
        cmd.extend(["--max-steps", str(max_steps)])
        
        try:
            result = subprocess.run(
                cmd,
                input=program,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
            
            elapsed_ms = int((time.time() - start) * 1000)
            self.total_time_ms += elapsed_ms
            
            if result.returncode != 0 and not result.stdout:
                exec_result = ExecuteResult(
                    success=False,
                    final_stack=[],
                    trace=[],
                    error=result.stderr.strip() or f"Exit code {result.returncode}",
                    execution_time_ms=elapsed_ms,
                )
            else:
                try:
                    data = json.loads(result.stdout)
                    exec_result = self._parse_json_result(data, elapsed_ms)
                except json.JSONDecodeError:
                    exec_result = self._parse_simple_output(result.stdout, result.stderr, elapsed_ms)
            
            # Cache result
            if use_cache and len(self._cache) < self._cache_max_size:
                self._cache[cache_key] = exec_result
            
            return exec_result
            
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
    
    def execute_step(
        self,
        program_so_far: str,
        next_token: str,
        max_steps: int = 10000,
    ) -> ExecuteResult:
        """
        Execute program + next token.
        
        Convenience method for MCTS expansion.
        """
        full_program = f"{program_so_far} {next_token}".strip()
        return self.execute(full_program, max_steps=max_steps, trace=True)
    
    def execute_batch(
        self,
        programs: List[str],
        max_steps: int = 10000,
        trace: bool = False,
    ) -> List[ExecuteResult]:
        """Execute multiple programs in parallel."""
        futures = []
        for program in programs:
            future = self.executor.submit(
                self.execute, program, max_steps, trace
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
        """Parse JSON output from kore binary."""
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
            steps_executed=data.get("steps_executed", len(trace_entries)),
            execution_time_ms=elapsed_ms,
        )
    
    def _parse_simple_output(
        self, stdout: str, stderr: str, elapsed_ms: int
    ) -> ExecuteResult:
        """Parse simple text output (fallback)."""
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
                stack.append(int(line))
            except ValueError:
                try:
                    stack.append(float(line))
                except ValueError:
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
            steps_executed=len(stack),
        )
    
    def health_check(self) -> bool:
        """Check if runtime is healthy."""
        try:
            result = self.execute("1 1 add", max_steps=100, trace=False, use_cache=False)
            return result.success and result.final_stack == [2]
        except:
            return False
    
    def stats(self) -> Dict:
        """Get execution statistics."""
        return {
            "total_executions": self.total_executions,
            "total_time_ms": self.total_time_ms,
            "avg_time_ms": self.total_time_ms / max(1, self.total_executions),
            "cache_size": len(self._cache),
            "cache_hits": self.cache_hits,
            "cache_hit_rate": self.cache_hits / max(1, self.total_executions + self.cache_hits),
        }
    
    def clear_cache(self):
        """Clear execution cache."""
        self._cache.clear()
        self.cache_hits = 0
    
    def close(self):
        """Shutdown thread pool."""
        self.executor.shutdown(wait=False)
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()
