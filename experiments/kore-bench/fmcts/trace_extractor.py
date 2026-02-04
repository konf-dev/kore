"""
Trace extractor for FMCTS.

Converts Kore execution traces into supervised training data.
This is a key Kore advantage: traces are semantic, not debugging output.

Key insight: The interpreter is the oracle. Every executed program
gives us ground-truth labels for every step.
"""

from dataclasses import dataclass, field
from typing import List, Tuple, Any, Optional, Dict
import json


@dataclass
class SupervisionPair:
    """
    A single (state, action) pair extracted from a trace.
    
    This is the fundamental unit of supervised training data.
    The LLM learns: "Given this stack and goal, output this token."
    """
    # Input features
    stack: Tuple[Any, ...]           # Current stack state
    goal: Tuple[Any, ...]            # Target stack state
    trace_context: List[Dict]        # Recent trace entries (for history)
    program_so_far: str              # Tokens generated so far
    
    # Label
    action: str                      # The next token (ground truth)
    
    # Metadata
    step_idx: int                    # Position in program
    reward: float                    # How good was this action
    task_id: Optional[str] = None   # For tracking
    
    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "stack": list(self.stack),
            "goal": list(self.goal),
            "trace_context": self.trace_context,
            "program_so_far": self.program_so_far,
            "action": self.action,
            "step_idx": self.step_idx,
            "reward": self.reward,
            "task_id": self.task_id,
        }
    
    @classmethod
    def from_dict(cls, d: Dict) -> "SupervisionPair":
        """Create from dictionary."""
        return cls(
            stack=tuple(d["stack"]),
            goal=tuple(d["goal"]),
            trace_context=d.get("trace_context", []),
            program_so_far=d.get("program_so_far", ""),
            action=d["action"],
            step_idx=d.get("step_idx", 0),
            reward=d.get("reward", 0.0),
            task_id=d.get("task_id"),
        )
    
    def to_prompt(self, include_trace: bool = True) -> str:
        """
        Convert to LLM prompt format.
        
        This is what the model sees during inference.
        """
        parts = []
        
        parts.append(f"Goal: {list(self.goal)}")
        parts.append(f"Stack: {list(self.stack)}")
        
        if include_trace and self.trace_context:
            trace_str = "\n".join(
                f"  {t.get('op', '?')}: {t.get('stack_before', [])} → {t.get('stack_after', [])}"
                for t in self.trace_context[-3:]  # Last 3 steps
            )
            parts.append(f"Trace:\n{trace_str}")
        
        if self.program_so_far:
            parts.append(f"Code: {self.program_so_far}")
        
        parts.append("Next token:")
        
        return "\n".join(parts)


def extract_supervision(
    program: str,
    goal: List[Any],
    execute_fn,
    task_id: Optional[str] = None,
    trace_context_size: int = 3
) -> List[SupervisionPair]:
    """
    Extract supervised training data from a successful program.
    
    This is the CORE function for trace-supervised learning.
    Given a program that solves a task, we extract (state, action) pairs
    for every step.
    
    Args:
        program: Kore program (space-separated tokens)
        goal: Target stack state
        execute_fn: Function to execute a token on a stack
        task_id: Optional task identifier
        trace_context_size: How many recent trace entries to include
    
    Returns:
        List of SupervisionPair, one per token in program
    """
    tokens = program.strip().split()
    if not tokens:
        return []
    
    goal_tuple = tuple(goal)
    pairs = []
    
    # Track execution state
    current_stack = []
    trace_history = []
    program_so_far = []
    
    for step_idx, token in enumerate(tokens):
        # Record state BEFORE action
        stack_before = tuple(current_stack)
        
        # Create supervision pair
        pair = SupervisionPair(
            stack=stack_before,
            goal=goal_tuple,
            trace_context=trace_history[-trace_context_size:],
            program_so_far=" ".join(program_so_far),
            action=token,
            step_idx=step_idx,
            reward=0.0,  # Will be computed below
            task_id=task_id,
        )
        
        # Execute token
        new_stack, success, error = execute_fn(list(current_stack), token)
        
        if not success:
            # Program failed - still useful data (negative signal)
            pair.reward = -1.0
            pairs.append(pair)
            break
        
        # Compute reward (progress toward goal)
        from .rewards import dense_reward
        pair.reward = dense_reward(list(stack_before), new_stack, goal)
        
        # Bonus for final step if successful
        if step_idx == len(tokens) - 1 and new_stack == goal:
            pair.reward += 1.0
        
        pairs.append(pair)
        
        # Update state
        trace_entry = {
            "op": token,
            "stack_before": list(stack_before),
            "stack_after": new_stack.copy(),
        }
        trace_history.append(trace_entry)
        program_so_far.append(token)
        current_stack = new_stack
    
    return pairs


def extract_supervision_from_trace(
    trace: List[Dict],
    goal: List[Any],
    task_id: Optional[str] = None,
    trace_context_size: int = 3
) -> List[SupervisionPair]:
    """
    Extract supervision directly from a trace object.
    
    Use this when you already have a trace from execution,
    rather than re-executing the program.
    
    Args:
        trace: List of TraceEntry dictionaries
        goal: Target stack state
        task_id: Optional task identifier
        trace_context_size: How many recent trace entries to include
    
    Returns:
        List of SupervisionPair
    """
    goal_tuple = tuple(goal)
    pairs = []
    program_tokens = []
    
    for step_idx, entry in enumerate(trace):
        if not entry.get("success", True):
            break  # Stop at first error
        
        stack_before = tuple(entry.get("stack_before", []))
        stack_after = entry.get("stack_after", [])
        token = entry.get("op", "")
        
        if not token:
            continue
        
        # Compute reward
        from .rewards import dense_reward
        reward = dense_reward(list(stack_before), stack_after, goal)
        
        # Create supervision pair
        pair = SupervisionPair(
            stack=stack_before,
            goal=goal_tuple,
            trace_context=trace[max(0, step_idx - trace_context_size):step_idx],
            program_so_far=" ".join(program_tokens),
            action=token,
            step_idx=step_idx,
            reward=reward,
            task_id=task_id,
        )
        
        pairs.append(pair)
        program_tokens.append(token)
    
    # Bonus for successful completion
    if trace and trace[-1].get("success", True):
        final_stack = trace[-1].get("stack_after", [])
        if final_stack == goal and pairs:
            pairs[-1].reward += 1.0
    
    return pairs


@dataclass
class SupervisionDataset:
    """
    Dataset of supervision pairs for training.
    
    Supports:
    - Adding pairs from multiple programs
    - Serialization to/from JSON
    - Sampling for training batches
    """
    pairs: List[SupervisionPair] = field(default_factory=list)
    metadata: Dict = field(default_factory=dict)
    
    def add_program(
        self,
        program: str,
        goal: List[Any],
        execute_fn,
        task_id: Optional[str] = None
    ):
        """Add supervision pairs from a program."""
        new_pairs = extract_supervision(program, goal, execute_fn, task_id)
        self.pairs.extend(new_pairs)
    
    def add_trace(
        self,
        trace: List[Dict],
        goal: List[Any],
        task_id: Optional[str] = None
    ):
        """Add supervision pairs from a trace."""
        new_pairs = extract_supervision_from_trace(trace, goal, task_id)
        self.pairs.extend(new_pairs)
    
    def save(self, path: str):
        """Save dataset to JSON file."""
        data = {
            "pairs": [p.to_dict() for p in self.pairs],
            "metadata": self.metadata,
        }
        with open(path, "w") as f:
            json.dump(data, f, indent=2)
    
    @classmethod
    def load(cls, path: str) -> "SupervisionDataset":
        """Load dataset from JSON file."""
        with open(path, "r") as f:
            data = json.load(f)
        
        dataset = cls(
            pairs=[SupervisionPair.from_dict(p) for p in data["pairs"]],
            metadata=data.get("metadata", {}),
        )
        return dataset
    
    def sample(self, n: int) -> List[SupervisionPair]:
        """Random sample of pairs."""
        import random
        return random.sample(self.pairs, min(n, len(self.pairs)))
    
    def filter_by_reward(self, min_reward: float = 0.0) -> "SupervisionDataset":
        """Filter to pairs with positive reward."""
        filtered = [p for p in self.pairs if p.reward >= min_reward]
        return SupervisionDataset(pairs=filtered, metadata=self.metadata.copy())
    
    def statistics(self) -> Dict:
        """Get dataset statistics."""
        if not self.pairs:
            return {"count": 0}
        
        rewards = [p.reward for p in self.pairs]
        step_indices = [p.step_idx for p in self.pairs]
        stack_sizes = [len(p.stack) for p in self.pairs]
        
        return {
            "count": len(self.pairs),
            "unique_tasks": len(set(p.task_id for p in self.pairs if p.task_id)),
            "reward_mean": sum(rewards) / len(rewards),
            "reward_min": min(rewards),
            "reward_max": max(rewards),
            "avg_step_idx": sum(step_indices) / len(step_indices),
            "avg_stack_size": sum(stack_sizes) / len(stack_sizes),
        }
    
    def __len__(self) -> int:
        return len(self.pairs)
    
    def __getitem__(self, idx: int) -> SupervisionPair:
        return self.pairs[idx]


def generate_training_batch(
    pairs: List[SupervisionPair],
    batch_size: int,
    include_trace: bool = True
) -> List[Dict]:
    """
    Generate a training batch from supervision pairs.
    
    Returns list of dicts with 'prompt' and 'completion' keys,
    suitable for LLM fine-tuning.
    """
    import random
    
    batch = random.sample(pairs, min(batch_size, len(pairs)))
    
    return [
        {
            "prompt": pair.to_prompt(include_trace=include_trace),
            "completion": pair.action,
            "reward": pair.reward,
        }
        for pair in batch
    ]
