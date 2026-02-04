"""
Trace-based reward functions for FMCTS.

Kore Advantage: Traces are semantic (part of the language),
so we can extract rich supervision from every execution.
"""

from dataclasses import dataclass
from typing import List, Any, Dict
import math

from .stack_distance import stack_distance, stack_similarity


def dense_reward(
    stack_before: List[Any],
    stack_after: List[Any],
    goal: List[Any]
) -> float:
    """
    Compute reward for a single step.
    
    Positive if we moved closer to goal, negative if we moved away.
    This is the PER-STEP reward used during MCTS simulation.
    
    Returns:
        Reward in range approximately [-1, 1]
    """
    dist_before = stack_distance(stack_before, goal)
    dist_after = stack_distance(stack_after, goal)
    
    # Progress = reduction in distance
    progress = dist_before - dist_after
    
    # Normalize to reasonable range
    reward = math.tanh(progress)
    
    # Bonus for reaching goal
    if stack_after == goal:
        reward += 1.0
    
    return reward


def trace_reward(
    trace: List[Dict],
    goal: List[Any],
    terminal_bonus: float = 10.0,
    step_cost: float = 0.01,
    error_penalty: float = 2.0,
) -> float:
    """
    Compute total reward from an execution trace.
    
    Uses trace information for rich reward signal:
    - Progress toward goal at each step
    - Penalty for unnecessary steps (encourages shorter programs)
    - Large bonus for reaching the goal
    - Penalty for errors
    
    Args:
        trace: List of trace entries from Kore execution
        goal: Target stack state
        terminal_bonus: Bonus for reaching exact goal
        step_cost: Small penalty per step
        error_penalty: Penalty for execution errors
    
    Returns:
        Total reward for the trace
    """
    if not trace:
        return -1.0  # No execution = bad
    
    total_reward = 0.0
    
    for i, entry in enumerate(trace):
        # Get stack states (handle both dict and TraceEntry)
        if hasattr(entry, 'stack_before'):
            stack_before = entry.stack_before
            stack_after = entry.stack_after
            success = entry.success
        else:
            stack_before = entry.get("stack_before", [])
            stack_after = entry.get("stack_after", [])
            success = entry.get("success", True)
        
        # Per-step progress reward
        step_reward = dense_reward(stack_before, stack_after, goal)
        total_reward += step_reward
        
        # Step cost (encourages shorter programs)
        total_reward -= step_cost
        
        # Check for errors
        if not success:
            total_reward -= error_penalty
            break
    
    # Final state reward
    if trace:
        last_entry = trace[-1]
        if hasattr(last_entry, 'stack_after'):
            final_stack = last_entry.stack_after
            success = last_entry.success
        else:
            final_stack = last_entry.get("stack_after", [])
            success = last_entry.get("success", True)
        
        if success:
            if final_stack == goal:
                total_reward += terminal_bonus
            else:
                # Partial credit based on final distance
                final_similarity = stack_similarity(final_stack, goal)
                total_reward += terminal_bonus * final_similarity * 0.5
    
    return total_reward


def success_reward(
    final_stack: List[Any],
    goal: List[Any],
    program_length: int,
    max_length: int = 50,
) -> float:
    """
    Simple success-based reward with length penalty.
    
    Used for final evaluation, not for per-step MCTS.
    """
    if final_stack == goal:
        # Success! Bonus for shorter programs
        length_bonus = 0.5 * (1.0 - program_length / max_length)
        return 1.0 + max(0, length_bonus)
    else:
        # Partial credit
        return stack_similarity(final_stack, goal) * 0.5


@dataclass
class RewardConfig:
    """Configuration for reward computation."""
    # Weights for different reward components
    progress_weight: float = 1.0
    terminal_weight: float = 10.0
    step_cost: float = 0.01
    error_penalty: float = 2.0
    
    # Stack distance settings
    length_penalty_weight: float = 2.0
    type_mismatch_penalty: float = 1.5
    
    # Success criteria
    max_program_length: int = 50
    
    def compute_trace_reward(
        self,
        trace: List[Dict],
        goal: List[Any],
    ) -> float:
        """Compute reward using this configuration."""
        return trace_reward(
            trace=trace,
            goal=goal,
            terminal_bonus=self.terminal_weight,
            step_cost=self.step_cost,
            error_penalty=self.error_penalty,
        )
    
    def compute_success_reward(
        self,
        final_stack: List[Any],
        goal: List[Any],
        program_length: int,
    ) -> float:
        """Compute success reward using this configuration."""
        return success_reward(
            final_stack=final_stack,
            goal=goal,
            program_length=program_length,
            max_length=self.max_program_length,
        )


# Pre-configured reward settings
DEFAULT_REWARD_CONFIG = RewardConfig()

SPARSE_REWARD_CONFIG = RewardConfig(
    progress_weight=0.0,
    terminal_weight=1.0,
    step_cost=0.0,
)

DENSE_REWARD_CONFIG = RewardConfig(
    progress_weight=1.0,
    terminal_weight=10.0,
    step_cost=0.02,
)
