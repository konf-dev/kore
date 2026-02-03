#!/usr/bin/env python3
"""
Fiber-Guided Program Search (FGPS) Implementation

This module implements the core FGPS algorithm that exploits Kore's unique features:
- Fibers: Pausable, forkable computation
- Checkpointing: Save/restore execution states  
- Effect analysis: Static verification before execution
- Traces: Deterministic replay

Usage:
    python fgps.py --goal "[14]" --beam-width 8 --max-steps 50
"""

import subprocess
import json
import hashlib
import heapq
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Tuple, Any
from pathlib import Path
import argparse
import math


# ============================================================================
# Kore Interface (subprocess-based, can be replaced with pyo3 bindings)
# ============================================================================

class KoreRuntime:
    """Interface to Kore runtime via subprocess."""
    
    def __init__(self, kore_binary: str = "./target/release/kore"):
        self.kore_binary = kore_binary
        self.fiber_counter = 0
        self._cache: Dict[str, Any] = {}
    
    def execute(self, program: str, initial_stack: List = None) -> Tuple[List, Optional[str]]:
        """Execute a Kore program and return (result_stack, error)."""
        cache_key = f"{program}:{initial_stack}"
        if cache_key in self._cache:
            return self._cache[cache_key]
        
        # Prepare initial stack as prefix
        if initial_stack:
            stack_prefix = " ".join(str(v) for v in initial_stack) + " "
        else:
            stack_prefix = ""
        
        full_program = stack_prefix + program
        
        try:
            result = subprocess.run(
                [self.kore_binary, "eval", full_program, "--format", "json"],
                capture_output=True,
                text=True,
                timeout=5.0
            )
            
            if result.returncode == 0:
                output = json.loads(result.stdout)
                stack = output.get("stack", [])
                self._cache[cache_key] = (stack, None)
                return stack, None
            else:
                error = result.stderr.strip() or "Unknown error"
                self._cache[cache_key] = ([], error)
                return [], error
                
        except subprocess.TimeoutExpired:
            return [], "Timeout"
        except Exception as e:
            return [], str(e)
    
    def effect_of(self, token: str) -> Tuple[int, int]:
        """Get stack effect (consumes, produces) for a token."""
        # Built-in effects (could be loaded from Kore metadata)
        effects = {
            # Literals (produce 1)
            "0": (0, 1), "1": (0, 1), "2": (0, 1), "3": (0, 1),
            "4": (0, 1), "5": (0, 1), "6": (0, 1), "7": (0, 1),
            "8": (0, 1), "9": (0, 1), "10": (0, 1),
            "true": (0, 1), "false": (0, 1), "null": (0, 1),
            
            # Stack ops
            "dup": (1, 2), "drop": (1, 0), "swap": (2, 2),
            "over": (2, 3), "rot": (3, 3), "nip": (2, 1),
            
            # Arithmetic
            "add": (2, 1), "sub": (2, 1), "mul": (2, 1),
            "div": (2, 1), "mod": (2, 1), "neg": (1, 1),
            
            # Comparison
            "eq": (2, 1), "lt": (2, 1), "gt": (2, 1),
            "le": (2, 1), "ge": (2, 1), "neq": (2, 1),
            
            # Logic
            "and": (2, 1), "or": (2, 1), "not": (1, 1),
        }
        
        # Check if it's a number
        try:
            float(token)
            return (0, 1)
        except ValueError:
            pass
        
        return effects.get(token, (0, 0))  # Unknown = no effect (conservative)
    
    def effect_valid(self, stack_depth: int, token: str) -> bool:
        """Check if token can be executed with current stack depth."""
        consumes, _ = self.effect_of(token)
        return stack_depth >= consumes


# ============================================================================
# Fiber Simulation (pure Python, mimics Kore fiber semantics)
# ============================================================================

@dataclass
class Fiber:
    """Simulated Kore fiber for search."""
    stack: List[Any] = field(default_factory=list)
    program: str = ""
    status: str = "running"  # running, paused, done, failed
    error: Optional[str] = None
    
    def fork(self) -> "Fiber":
        """Create a copy of this fiber (COW would be O(1) in Rust)."""
        return Fiber(
            stack=self.stack.copy(),
            program=self.program,
            status=self.status,
            error=self.error
        )
    
    def hash_stack(self) -> str:
        """Hash stack state for checkpoint lookup."""
        return hashlib.md5(str(self.stack).encode()).hexdigest()[:16]


# ============================================================================
# LLM Interface (placeholder - replace with actual model)
# ============================================================================

class SimpleLLM:
    """Simple rule-based LLM placeholder.
    
    In production, replace with actual LLM (Qwen, fine-tuned model, etc.)
    
    This version uses a smarter heuristic that reasons about how to
    construct the goal value through operations.
    """
    
    def __init__(self):
        # Token vocabulary
        self.vocab = [
            # Numbers
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
            # Stack ops
            "dup", "drop", "swap", "over", "rot",
            # Arithmetic
            "add", "sub", "mul", "div", "mod", "neg",
            # Comparison
            "eq", "lt", "gt", "le", "ge",
            # Logic
            "and", "or", "not",
            # Booleans
            "true", "false",
        ]
        
        # Precompute useful number decompositions
        self._decompositions = self._compute_decompositions(100)
    
    def _compute_decompositions(self, max_n: int) -> Dict[int, List[Tuple[str, ...]]]:
        """Precompute ways to make numbers from 0-10 using operations."""
        decomp = {}
        
        # Direct numbers
        for i in range(11):
            decomp[i] = [(str(i),)]
        
        # Two-number operations
        for a in range(11):
            for b in range(11):
                # a + b
                s = a + b
                if s <= max_n:
                    decomp.setdefault(s, []).append((str(a), str(b), "add"))
                # a * b
                p = a * b
                if p <= max_n and p > 0:
                    decomp.setdefault(p, []).append((str(a), str(b), "mul"))
                # a - b
                d = a - b
                if d >= 0 and d <= max_n:
                    decomp.setdefault(d, []).append((str(a), str(b), "sub"))
        
        # dup + op patterns (a a op)
        for a in range(11):
            decomp.setdefault(a + a, []).append((str(a), "dup", "add"))
            decomp.setdefault(a * a, []).append((str(a), "dup", "mul"))
        
        return decomp
    
    def propose(
        self,
        program: str,
        current_stack: List,
        goal_stack: List,
        top_k: int = 5
    ) -> List[Tuple[str, float]]:
        """Propose next tokens given current state.
        
        Returns: List of (token, probability) pairs.
        
        Uses decomposition table to suggest tokens that help reach goal.
        """
        proposals = []
        
        # Check if goal is a single integer we can decompose
        if len(goal_stack) == 1 and isinstance(goal_stack[0], int):
            goal_val = goal_stack[0]
            
            # Use decomposition table
            if goal_val in self._decompositions:
                for decomp in self._decompositions[goal_val][:3]:  # Top 3 decompositions
                    # Find which token from decomposition we should do next
                    current_len = len(current_stack)
                    tokens_so_far = program.split() if program.strip() else []
                    
                    # If we have the right values on stack, suggest the operation
                    if len(decomp) > 0:
                        next_token = decomp[min(len(tokens_so_far), len(decomp)-1)]
                        
                        # Verify this token is valid
                        score = self._score_token(next_token, current_stack, goal_stack)
                        if score > 0:
                            proposals.append((next_token, score + 5.0))  # Boost decomposition tokens
        
        # Also add regular scored proposals
        for token in self.vocab:
            score = self._score_token(token, current_stack, goal_stack)
            if score > 0:
                proposals.append((token, score))
        
        # Deduplicate by token, keeping highest score
        token_scores = {}
        for token, score in proposals:
            token_scores[token] = max(token_scores.get(token, 0), score)
        
        proposals = list(token_scores.items())
        
        # Normalize to probabilities
        total = sum(s for _, s in proposals)
        if total > 0:
            proposals = [(t, s/total) for t, s in proposals]
        
        # Return top-k
        proposals.sort(key=lambda x: -x[1])
        return proposals[:top_k]
    
    def _score_token(
        self,
        token: str,
        current_stack: List,
        goal_stack: List
    ) -> float:
        """Heuristic scoring for token selection."""
        score = 0.1  # Base score
        
        # Simulate what happens if we apply this token
        test_stack = current_stack.copy()
        
        try:
            # Handle literals
            if token.isdigit():
                test_stack.append(int(token))
            elif token == "true":
                test_stack.append(True)
            elif token == "false":
                test_stack.append(False)
            # Handle operations
            elif token == "add" and len(test_stack) >= 2:
                b, a = test_stack.pop(), test_stack.pop()
                test_stack.append(a + b)
            elif token == "sub" and len(test_stack) >= 2:
                b, a = test_stack.pop(), test_stack.pop()
                test_stack.append(a - b)
            elif token == "mul" and len(test_stack) >= 2:
                b, a = test_stack.pop(), test_stack.pop()
                test_stack.append(a * b)
            elif token == "div" and len(test_stack) >= 2:
                b, a = test_stack.pop(), test_stack.pop()
                test_stack.append(a // b if b != 0 else 0)
            elif token == "dup" and len(test_stack) >= 1:
                test_stack.append(test_stack[-1])
            elif token == "drop" and len(test_stack) >= 1:
                test_stack.pop()
            elif token == "swap" and len(test_stack) >= 2:
                test_stack[-1], test_stack[-2] = test_stack[-2], test_stack[-1]
            elif token == "neg" and len(test_stack) >= 1:
                test_stack[-1] = -test_stack[-1]
            else:
                return 0.0  # Can't apply this token
        except:
            return 0.0
        
        # Score based on distance to goal
        dist_before = self._stack_distance(current_stack, goal_stack)
        dist_after = self._stack_distance(test_stack, goal_stack)
        
        # Reward tokens that move closer to goal
        improvement = dist_before - dist_after
        if improvement > 0:
            score += improvement * 2.0
        elif improvement == 0:
            score += 0.1
        
        # Big bonus if we hit the goal exactly
        if test_stack == goal_stack:
            score += 100.0
        
        # Bonus for matching stack length
        if len(test_stack) == len(goal_stack):
            score += 0.5
        
        # Penalize going much further from goal
        if dist_after > dist_before + 5:
            score = 0.01
        
        return score
    
    def _stack_distance(self, s1: List, s2: List) -> float:
        """Distance metric between stacks."""
        # Length difference
        len_diff = abs(len(s1) - len(s2))
        
        # Value difference for overlapping positions
        val_diff = 0.0
        for i in range(min(len(s1), len(s2))):
            if isinstance(s1[i], (int, float)) and isinstance(s2[i], (int, float)):
                val_diff += abs(s1[i] - s2[i]) / (1 + abs(s2[i]))
            elif s1[i] != s2[i]:
                val_diff += 1.0
        
        return len_diff * 10 + val_diff


# ============================================================================
# FGPS Core Algorithm
# ============================================================================

@dataclass(order=True)
class SearchState:
    """State in the beam search."""
    priority: float  # Negative log prob (for min-heap)
    fiber: Fiber = field(compare=False)
    log_prob: float = field(compare=False)


class FiberGuidedSearch:
    """Main FGPS implementation."""
    
    def __init__(
        self,
        runtime: KoreRuntime,
        llm: SimpleLLM,
        beam_width: int = 8,
        max_steps: int = 50,
        checkpoint_dir: str = "./checkpoints"
    ):
        self.runtime = runtime
        self.llm = llm
        self.beam_width = beam_width
        self.max_steps = max_steps
        self.checkpoint_dir = Path(checkpoint_dir)
        self.checkpoint_dir.mkdir(exist_ok=True)
        
        # Checkpoints: stack_hash → (program, fiber)
        self.checkpoints: Dict[str, Tuple[str, Fiber]] = {}
        
        # Stats
        self.stats = {
            "fibers_created": 0,
            "fibers_pruned": 0,
            "checkpoints_saved": 0,
            "checkpoints_restored": 0,
        }
    
    def search(
        self,
        goal_stack: List,
        initial_stack: List = None
    ) -> Optional[str]:
        """Search for a program that produces goal_stack from initial_stack."""
        
        if initial_stack is None:
            initial_stack = []
        
        # Initialize beam with starting state
        initial_fiber = Fiber(stack=initial_stack.copy())
        beam = [SearchState(priority=0.0, fiber=initial_fiber, log_prob=0.0)]
        
        for step in range(self.max_steps):
            if not beam:
                # Try to restore from checkpoints
                beam = self._restore_from_checkpoints(goal_stack)
                if not beam:
                    break
            
            candidates = []
            
            for state in beam:
                fiber = state.fiber
                
                # Check if we've reached the goal
                if fiber.stack == goal_stack:
                    print(f"✓ Found solution at step {step}: {fiber.program}")
                    return fiber.program.strip()
                
                # Get LLM proposals
                proposals = self.llm.propose(
                    program=fiber.program,
                    current_stack=fiber.stack,
                    goal_stack=goal_stack,
                    top_k=self.beam_width
                )
                
                for token, prob in proposals:
                    # Effect check (static analysis!)
                    if not self.runtime.effect_valid(len(fiber.stack), token):
                        self.stats["fibers_pruned"] += 1
                        continue
                    
                    # Fork fiber
                    new_fiber = fiber.fork()
                    self.stats["fibers_created"] += 1
                    
                    # Apply token (simulated execution)
                    success = self._apply_token(new_fiber, token)
                    
                    if success:
                        new_fiber.program = fiber.program + " " + token
                        new_log_prob = state.log_prob + math.log(prob + 1e-10)
                        
                        # Checkpoint novel states
                        stack_hash = new_fiber.hash_stack()
                        if stack_hash not in self.checkpoints:
                            self.checkpoints[stack_hash] = (new_fiber.program, new_fiber.fork())
                            self.stats["checkpoints_saved"] += 1
                        
                        candidates.append(SearchState(
                            priority=-new_log_prob,  # Min-heap
                            fiber=new_fiber,
                            log_prob=new_log_prob
                        ))
                    else:
                        self.stats["fibers_pruned"] += 1
            
            # Select top-k candidates
            heapq.heapify(candidates)
            beam = [heapq.heappop(candidates) for _ in range(min(self.beam_width, len(candidates)))]
            
            if step % 10 == 0:
                self._print_progress(step, beam, goal_stack)
        
        return None
    
    def _apply_token(self, fiber: Fiber, token: str) -> bool:
        """Apply a single token to fiber's stack. Returns success."""
        try:
            stack = fiber.stack
            
            # Literals
            if token.lstrip('-').isdigit():
                stack.append(int(token))
            elif token == "true":
                stack.append(True)
            elif token == "false":
                stack.append(False)
            
            # Stack operations
            elif token == "dup":
                stack.append(stack[-1])
            elif token == "drop":
                stack.pop()
            elif token == "swap":
                stack[-1], stack[-2] = stack[-2], stack[-1]
            elif token == "over":
                stack.append(stack[-2])
            elif token == "rot":
                a, b, c = stack.pop(), stack.pop(), stack.pop()
                stack.extend([b, a, c])
            elif token == "nip":
                del stack[-2]
            
            # Arithmetic
            elif token == "add":
                b, a = stack.pop(), stack.pop()
                stack.append(a + b)
            elif token == "sub":
                b, a = stack.pop(), stack.pop()
                stack.append(a - b)
            elif token == "mul":
                b, a = stack.pop(), stack.pop()
                stack.append(a * b)
            elif token == "div":
                b, a = stack.pop(), stack.pop()
                if b == 0:
                    return False
                stack.append(a // b)
            elif token == "mod":
                b, a = stack.pop(), stack.pop()
                if b == 0:
                    return False
                stack.append(a % b)
            elif token == "neg":
                stack[-1] = -stack[-1]
            
            # Comparison
            elif token == "eq":
                b, a = stack.pop(), stack.pop()
                stack.append(a == b)
            elif token == "lt":
                b, a = stack.pop(), stack.pop()
                stack.append(a < b)
            elif token == "gt":
                b, a = stack.pop(), stack.pop()
                stack.append(a > b)
            elif token == "le":
                b, a = stack.pop(), stack.pop()
                stack.append(a <= b)
            elif token == "ge":
                b, a = stack.pop(), stack.pop()
                stack.append(a >= b)
            
            # Logic
            elif token == "and":
                b, a = stack.pop(), stack.pop()
                stack.append(a and b)
            elif token == "or":
                b, a = stack.pop(), stack.pop()
                stack.append(a or b)
            elif token == "not":
                stack[-1] = not stack[-1]
            
            else:
                # Unknown token
                return False
            
            return True
            
        except (IndexError, TypeError, ValueError):
            return False
    
    def _restore_from_checkpoints(
        self,
        goal_stack: List
    ) -> List[SearchState]:
        """Try to find checkpoints that are close to goal."""
        candidates = []
        
        for stack_hash, (program, fiber) in self.checkpoints.items():
            distance = self.llm._stack_distance(fiber.stack, goal_stack)
            if distance < 10:  # Threshold
                self.stats["checkpoints_restored"] += 1
                candidates.append(SearchState(
                    priority=distance,
                    fiber=fiber.fork(),
                    log_prob=0.0
                ))
        
        candidates.sort(key=lambda s: s.priority)
        return candidates[:self.beam_width]
    
    def _print_progress(
        self,
        step: int,
        beam: List[SearchState],
        goal_stack: List
    ):
        """Print search progress."""
        if beam:
            best = beam[0]
            distance = self.llm._stack_distance(best.fiber.stack, goal_stack)
            print(f"Step {step:3d}: beam={len(beam)}, "
                  f"best_stack={best.fiber.stack}, "
                  f"dist={distance:.2f}, "
                  f"prog='{best.fiber.program.strip()[:40]}'")
    
    def print_stats(self):
        """Print final statistics."""
        print("\n" + "="*50)
        print("FGPS Statistics:")
        for key, value in self.stats.items():
            print(f"  {key}: {value}")
        print("="*50)


# ============================================================================
# Main Entry Point
# ============================================================================

def main():
    parser = argparse.ArgumentParser(description="Fiber-Guided Program Search")
    parser.add_argument("--goal", type=str, required=True,
                        help="Goal stack as JSON list, e.g., '[14]'")
    parser.add_argument("--initial", type=str, default="[]",
                        help="Initial stack as JSON list")
    parser.add_argument("--beam-width", type=int, default=8,
                        help="Beam width for search")
    parser.add_argument("--max-steps", type=int, default=50,
                        help="Maximum search steps")
    args = parser.parse_args()
    
    # Parse stacks
    goal_stack = json.loads(args.goal)
    initial_stack = json.loads(args.initial)
    
    print(f"Goal: {goal_stack}")
    print(f"Initial: {initial_stack}")
    print(f"Beam width: {args.beam_width}")
    print(f"Max steps: {args.max_steps}")
    print()
    
    # Initialize components
    runtime = KoreRuntime()
    llm = SimpleLLM()
    
    searcher = FiberGuidedSearch(
        runtime=runtime,
        llm=llm,
        beam_width=args.beam_width,
        max_steps=args.max_steps
    )
    
    # Run search
    solution = searcher.search(goal_stack, initial_stack)
    
    if solution:
        print(f"\n✓ Solution found: {solution}")
        
        # Verify
        result_stack, error = runtime.execute(solution, initial_stack)
        if error:
            print(f"  Verification failed: {error}")
        elif result_stack == goal_stack:
            print(f"  ✓ Verified: {initial_stack} → {result_stack}")
        else:
            print(f"  ✗ Mismatch: expected {goal_stack}, got {result_stack}")
    else:
        print("\n✗ No solution found")
    
    searcher.print_stats()


if __name__ == "__main__":
    main()
