"""
Reward functions for FMCTS.

Exploits Kore's observable stack semantics to provide dense rewards
at every step, eliminating the credit assignment problem.

Key insight: Stack IS the semantic state. We can measure progress
toward the goal at every step, not just at the end.
"""

from typing import List, Tuple, Any, Optional
from dataclasses import dataclass
import math


def stack_distance(current: List[Any], goal: List[Any]) -> float:
    """
    Compute distance between current stack and goal stack.
    
    Lower is better. 0.0 = exact match.
    
    This is the CORE reward signal for FMCTS. Because Kore's stack
    is fully observable, we can compute this at every step.
    
    Components:
    1. Length difference penalty
    2. Element-wise distance (weighted by position)
    3. Type mismatch penalty
    """
    if current == goal:
        return 0.0
    
    # Length difference (heavily penalized)
    len_diff = abs(len(current) - len(goal))
    len_penalty = len_diff * 2.0
    
    # Element-wise comparison (top of stack is more important)
    element_distance = 0.0
    max_len = max(len(current), len(goal))
    
    for i in range(max_len):
        # Weight: top of stack (end of list) has higher weight
        weight = 1.0 + (i / max_len)  # 1.0 to 2.0
        
        # Get values (None if index out of bounds)
        curr_val = current[i] if i < len(current) else None
        goal_val = goal[i] if i < len(goal) else None
        
        if curr_val == goal_val:
            continue  # Perfect match, no penalty
        
        if curr_val is None or goal_val is None:
            element_distance += weight * 1.0  # Missing element
            continue
        
        # Type check
        if type(curr_val) != type(goal_val):
            element_distance += weight * 1.5  # Type mismatch
            continue
        
        # Numeric distance
        if isinstance(curr_val, (int, float)) and isinstance(goal_val, (int, float)):
            # Logarithmic distance for numbers (handles large differences)
            if curr_val == goal_val:
                numeric_dist = 0.0
            elif curr_val == 0 or goal_val == 0:
                numeric_dist = abs(curr_val - goal_val) / (abs(max(curr_val, goal_val)) + 1)
            else:
                # Relative error
                numeric_dist = min(1.0, abs(curr_val - goal_val) / (abs(goal_val) + 1e-10))
            element_distance += weight * numeric_dist
            continue
        
        # String distance (simplified)
        if isinstance(curr_val, str) and isinstance(goal_val, str):
            if curr_val == goal_val:
                str_dist = 0.0
            else:
                # Normalized edit distance (approximate)
                max_str_len = max(len(curr_val), len(goal_val), 1)
                common_prefix = 0
                for c1, c2 in zip(curr_val, goal_val):
                    if c1 == c2:
                        common_prefix += 1
                    else:
                        break
                str_dist = 1.0 - (common_prefix / max_str_len)
            element_distance += weight * str_dist
            continue
        
        # List distance (recursive, but capped)
        if isinstance(curr_val, list) and isinstance(goal_val, list):
            # Shallow comparison
            if curr_val == goal_val:
                list_dist = 0.0
            else:
                list_dist = 0.5 + 0.5 * min(1.0, abs(len(curr_val) - len(goal_val)) / (max(len(goal_val), 1)))
            element_distance += weight * list_dist
            continue
        
        # Default: different values
        element_distance += weight * 1.0
    
    return len_penalty + element_distance


def stack_similarity(current: List[Any], goal: List[Any]) -> float:
    """
    Compute similarity between current stack and goal stack.
    
    Higher is better. 1.0 = exact match. 0.0 = completely different.
    
    This is the inverse of distance, normalized to [0, 1].
    """
    if current == goal:
        return 1.0
    
    dist = stack_distance(current, goal)
    # Normalize: distance of 0 -> similarity of 1
    # Use exponential decay for smooth gradient
    return math.exp(-dist)


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
    reward = math.tanh(progress)  # Squash to [-1, 1]
    
    # Bonus for reaching goal
    if stack_after == goal:
        reward += 1.0
    
    return reward


def trace_reward(
    trace: List[dict],
    goal: List[Any],
    terminal_bonus: float = 10.0,
    step_cost: float = 0.01
) -> float:
    """
    Compute total reward from an execution trace.
    
    Uses trace information for rich reward signal:
    - Progress toward goal at each step
    - Penalty for unnecessary steps (encourages shorter programs)
    - Large bonus for reaching the goal
    
    Args:
        trace: List of trace entries from Kore execution
        goal: Target stack state
        terminal_bonus: Bonus for reaching exact goal
        step_cost: Small penalty per step (encourages brevity)
    
    Returns:
        Total reward for the trace
    """
    if not trace:
        return -1.0  # No execution = bad
    
    total_reward = 0.0
    
    for i, entry in enumerate(trace):
        # Get stack states
        stack_before = entry.get("stack_before", [])
        stack_after = entry.get("stack_after", [])
        
        # Per-step progress reward
        step_reward = dense_reward(stack_before, stack_after, goal)
        total_reward += step_reward
        
        # Step cost (encourages shorter programs)
        total_reward -= step_cost
        
        # Check for errors
        if not entry.get("success", True):
            total_reward -= 2.0  # Penalty for errors
            break
    
    # Final state
    if trace:
        final_stack = trace[-1].get("stack_after", [])
        
        # Terminal reward
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
    max_length: int = 50
) -> float:
    """
    Simple success-based reward with length penalty.
    
    Used for final evaluation, not for per-step MCTS.
    
    Returns:
        1.0 for success (with length bonus)
        Partial credit based on similarity otherwise
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
    
    def compute_reward(
        self,
        trace: List[dict],
        goal: List[Any]
    ) -> float:
        """Compute reward using this configuration."""
        return trace_reward(
            trace=trace,
            goal=goal,
            terminal_bonus=self.terminal_weight,
            step_cost=self.step_cost
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
