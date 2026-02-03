"""
Kore Runtime Interface

Provides structured interaction with Kore for LLM training.
LLM sees: execution results, traces, effects, errors
LLM does NOT see: Rust source, container, filesystem
"""

import json
import subprocess
import hashlib
from dataclasses import dataclass, field
from typing import List, Dict, Tuple, Optional, Any
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
import threading


@dataclass
class ExecutionResult:
    """Result of executing a Kore program."""
    success: bool
    stack: List[Any] = field(default_factory=list)
    trace: List[Dict] = field(default_factory=list)
    error: Optional[str] = None
    effect: Optional[Dict] = None
    
    def to_prompt_string(self, include_trace: bool = True) -> str:
        """Format result for LLM prompt."""
        lines = []
        
        if self.success:
            lines.append(f"Result: {self.stack}")
            
            if self.effect:
                lines.append(f"Effect: ({self.effect.get('consumes', '?')} -- {self.effect.get('produces', '?')})")
                if self.effect.get('io'):
                    lines.append(f"IO Effects: {self.effect['io']}")
            
            if include_trace and self.trace:
                lines.append("\nExecution Trace:")
                for i, step in enumerate(self.trace[:20]):  # Limit trace length
                    op = step.get('op', '?')
                    stack_before = step.get('stack_before', [])
                    stack_after = step.get('stack_after', [])
                    lines.append(f"  {i}: {stack_before} → {op} → {stack_after}")
                
                if len(self.trace) > 20:
                    lines.append(f"  ... ({len(self.trace) - 20} more steps)")
        else:
            lines.append(f"Error: {self.error}")
        
        return "\n".join(lines)


@dataclass
class EffectSignature:
    """Kore program effect signature."""
    consumes: int
    produces: int
    io_effects: List[str] = field(default_factory=list)
    pure: bool = True
    
    def matches(self, other: 'EffectSignature') -> bool:
        return self.consumes == other.consumes and self.produces == other.produces
    
    def distance(self, other: 'EffectSignature') -> float:
        stack_dist = abs(self.consumes - other.consumes) + abs(self.produces - other.produces)
        io_dist = len(set(self.io_effects) ^ set(other.io_effects))
        return stack_dist + 0.5 * io_dist
    
    def to_string(self) -> str:
        io_str = f" [io: {', '.join(self.io_effects)}]" if self.io_effects else ""
        return f"({self.consumes} -- {self.produces}){io_str}"
    
    @classmethod
    def parse(cls, effect_dict: Dict) -> 'EffectSignature':
        return cls(
            consumes=effect_dict.get('consumes', 0),
            produces=effect_dict.get('produces', 0),
            io_effects=effect_dict.get('io', []),
            pure=effect_dict.get('pure', True)
        )


class KoreRuntime:
    """
    Interface to Kore runtime for LLM training.
    
    Provides:
    - Program execution with results
    - Step-by-step traces
    - Effect inference
    - Batch parallel execution
    """
    
    def __init__(
        self, 
        binary_path: str = "../target/release/kore-train",
        timeout: float = 5.0,
        max_trace_steps: int = 100,
        parallel_workers: int = 8
    ):
        self.binary = Path(binary_path).resolve()
        self.timeout = timeout
        self.max_trace_steps = max_trace_steps
        self.parallel_workers = parallel_workers
        self._lock = threading.Lock()
        
        # Verify binary exists
        if not self.binary.exists():
            raise FileNotFoundError(
                f"Kore binary not found at {self.binary}. "
                f"Run: cargo build --release --bin kore-train"
            )
    
    def execute(
        self, 
        program: str, 
        initial_stack: List[Any] = None,
        trace: bool = True
    ) -> ExecutionResult:
        """Execute a Kore program and return structured result."""
        
        try:
            input_data = json.dumps({
                "program": program,
                "stack": initial_stack or [],
                "trace": trace,
                "max_steps": self.max_trace_steps,
            })
            
            result = subprocess.run(
                [str(self.binary)],
                input=input_data,
                capture_output=True,
                text=True,
                timeout=self.timeout
            )
            
            if result.returncode == 0:
                output = json.loads(result.stdout)
                return ExecutionResult(
                    success=True,
                    stack=output.get("stack", []),
                    trace=output.get("trace", []),
                    effect=output.get("effect"),
                )
            else:
                return ExecutionResult(
                    success=False,
                    error=result.stderr.strip() or "Execution failed"
                )
                
        except subprocess.TimeoutExpired:
            return ExecutionResult(success=False, error="Timeout: program took too long")
        except json.JSONDecodeError as e:
            return ExecutionResult(success=False, error=f"Invalid output: {e}")
        except Exception as e:
            return ExecutionResult(success=False, error=str(e))
    
    def get_effect(self, program: str) -> Optional[EffectSignature]:
        """Get effect signature of program without full execution."""
        
        # Wrap in effect-infer
        analysis_program = f"[ {program} ] effect-infer"
        result = self.execute(analysis_program, trace=False)
        
        if result.success and result.stack:
            effect_dict = result.stack[0]
            if isinstance(effect_dict, dict) and 'effect' in effect_dict:
                inner = effect_dict['effect']
                return EffectSignature(
                    consumes=inner.get('consumes', 0),
                    produces=inner.get('produces', 0),
                    io_effects=effect_dict.get('io', []),
                    pure=effect_dict.get('pure', True)
                )
        return None
    
    def verify_equivalence(
        self, 
        p1: str, 
        p2: str, 
        n_tests: int = 50,
        value_range: Tuple[int, int] = (-100, 100)
    ) -> bool:
        """Test if two programs are equivalent on random inputs."""
        
        import random
        
        for _ in range(n_tests):
            # Random stack
            depth = random.randint(0, 5)
            stack = [random.randint(*value_range) for _ in range(depth)]
            
            r1 = self.execute(p1, stack.copy(), trace=False)
            r2 = self.execute(p2, stack.copy(), trace=False)
            
            # Both must succeed or both fail
            if r1.success != r2.success:
                return False
            
            # If both succeed, stacks must match
            if r1.success and r2.success:
                if r1.stack != r2.stack:
                    return False
        
        return True
    
    def execute_batch(
        self, 
        programs: List[str],
        initial_stacks: List[List[Any]] = None,
        trace: bool = False
    ) -> List[ExecutionResult]:
        """Execute multiple programs in parallel."""
        
        if initial_stacks is None:
            initial_stacks = [[] for _ in programs]
        
        results = [None] * len(programs)
        
        with ThreadPoolExecutor(max_workers=self.parallel_workers) as executor:
            futures = {
                executor.submit(self.execute, prog, stack, trace): i
                for i, (prog, stack) in enumerate(zip(programs, initial_stacks))
            }
            
            for future in as_completed(futures):
                idx = futures[future]
                try:
                    results[idx] = future.result()
                except Exception as e:
                    results[idx] = ExecutionResult(success=False, error=str(e))
        
        return results
    
    def get_trace_fingerprint(self, program: str) -> Optional[str]:
        """Get hash of execution trace for novelty detection."""
        
        result = self.execute(program, trace=True)
        if result.success and result.trace:
            trace_str = json.dumps(result.trace, sort_keys=True)
            return hashlib.sha256(trace_str.encode()).hexdigest()[:16]
        return None


# =============================================================================
# Prompt Templates
# =============================================================================

SYSTEM_PROMPT = """You are an expert Kore programmer. Kore is a stack-based language where:

1. Values are pushed onto a stack: `5 3` → stack is [5, 3]
2. Operations consume values and push results: `5 3 add` → [8]
3. Stack effect notation: (before -- after), e.g., `add` is (a b -- sum)

Core operations:
- Stack: dup (a -- a a), drop (a --), swap (a b -- b a), rot (a b c -- b c a), over (a b -- a b a)
- Math: add, sub, mul, div, mod, neg
- Compare: eq, lt (others composed: gt = swap lt, neq = eq not)
- Logic: and, or, not
- Control: [code] call, cond [then] [else] if, n [body] times, [cond] [body] while
- Define: [code] "name" def

When writing Kore:
1. Think about the stack effect you need
2. Trace through mentally: what's on stack after each op?
3. Keep it simple - shorter is usually better"""


def make_task_prompt(
    task_description: str,
    expected_effect: EffectSignature = None,
    examples: List[Tuple[List, Any]] = None,
    few_shot: List[Tuple[str, str]] = None
) -> str:
    """Create a task prompt for the LLM."""
    
    parts = [f"Task: {task_description}"]
    
    if expected_effect:
        parts.append(f"Required effect: {expected_effect.to_string()}")
    
    if examples:
        parts.append("\nExamples:")
        for inputs, output in examples[:5]:
            parts.append(f"  Input: {inputs} → Output: {output}")
    
    if few_shot:
        parts.append("\nSimilar solved problems:")
        for desc, solution in few_shot[:3]:
            parts.append(f"  {desc}")
            parts.append(f"  Solution: {solution}")
    
    parts.append("\nYour Kore program:")
    
    return "\n".join(parts)


def make_feedback_prompt(
    program: str,
    result: ExecutionResult,
    task_description: str,
    expected: Any = None
) -> str:
    """Create feedback prompt showing execution result."""
    
    parts = [
        f"Your program: {program}",
        "",
        result.to_prompt_string(include_trace=True),
    ]
    
    if expected is not None:
        if result.success and result.stack:
            if result.stack[-1] == expected:
                parts.append(f"\n✓ Correct! Expected {expected}, got {result.stack[-1]}")
            else:
                parts.append(f"\n✗ Incorrect. Expected {expected}, got {result.stack[-1]}")
        else:
            parts.append(f"\n✗ Expected {expected}, but execution failed")
    
    parts.append(f"\nOriginal task: {task_description}")
    parts.append("\nRevised program (or same if correct):")
    
    return "\n".join(parts)


# =============================================================================
# Test
# =============================================================================

if __name__ == "__main__":
    # Test the runtime
    runtime = KoreRuntime()
    
    print("Testing Kore Runtime...")
    
    # Test basic execution
    result = runtime.execute("5 3 add")
    print(f"\n5 3 add = {result.stack}")
    assert result.success and result.stack == [8]
    
    # Test with trace
    result = runtime.execute("5 dup mul", trace=True)
    print(f"\n5 dup mul = {result.stack}")
    print(result.to_prompt_string())
    assert result.success and result.stack == [25]
    
    # Test effect inference
    effect = runtime.get_effect("dup mul")
    print(f"\nEffect of 'dup mul': {effect.to_string()}")
    assert effect.consumes == 1 and effect.produces == 1
    
    # Test error handling
    result = runtime.execute("add")  # Not enough values
    print(f"\nError test: {result.error}")
    assert not result.success
    
    # Test equivalence
    equiv = runtime.verify_equivalence("dup mul", "dup dup mul drop")
    print(f"\n'dup mul' ≡ 'dup dup mul drop': {equiv}")
    
    # Test batch execution
    programs = ["1 2 add", "3 4 mul", "5 dup add"]
    results = runtime.execute_batch(programs)
    print(f"\nBatch results: {[r.stack for r in results]}")
    
    print("\n✓ All tests passed!")
