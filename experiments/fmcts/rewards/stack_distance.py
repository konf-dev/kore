"""
Stack distance metrics for FMCTS.

Kore Advantage: Stack is THE semantic state. We can measure
progress toward the goal at every step.
"""

from typing import List, Any
import math


def stack_distance(current: List[Any], goal: List[Any]) -> float:
    """
    Compute distance between current stack and goal stack.
    
    Lower is better. 0.0 = exact match.
    
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
    
    # Element-wise comparison (top of stack = end of list, more important)
    element_distance = 0.0
    max_len = max(len(current), len(goal))
    
    if max_len == 0:
        return len_penalty
    
    for i in range(max_len):
        # Weight: top of stack has higher weight
        weight = 1.0 + (i / max_len)  # 1.0 to 2.0
        
        curr_val = current[i] if i < len(current) else None
        goal_val = goal[i] if i < len(goal) else None
        
        if curr_val == goal_val:
            continue  # Perfect match
        
        if curr_val is None or goal_val is None:
            element_distance += weight * 1.0  # Missing element
            continue
        
        # Type check
        if type(curr_val) != type(goal_val):
            element_distance += weight * 1.5  # Type mismatch
            continue
        
        # Numeric distance
        if isinstance(curr_val, (int, float)) and isinstance(goal_val, (int, float)):
            if curr_val == goal_val:
                numeric_dist = 0.0
            elif goal_val == 0:
                numeric_dist = min(1.0, abs(curr_val) / 10.0)
            else:
                # Relative error, capped at 1.0
                numeric_dist = min(1.0, abs(curr_val - goal_val) / (abs(goal_val) + 1e-10))
            element_distance += weight * numeric_dist
            continue
        
        # String distance (simplified)
        if isinstance(curr_val, str) and isinstance(goal_val, str):
            if curr_val == goal_val:
                str_dist = 0.0
            else:
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
        
        # List distance (shallow)
        if isinstance(curr_val, list) and isinstance(goal_val, list):
            if curr_val == goal_val:
                list_dist = 0.0
            else:
                list_dist = 0.5 + 0.5 * min(1.0, abs(len(curr_val) - len(goal_val)) / max(len(goal_val), 1))
            element_distance += weight * list_dist
            continue
        
        # Default: different values
        element_distance += weight * 1.0
    
    return len_penalty + element_distance


def stack_similarity(current: List[Any], goal: List[Any]) -> float:
    """
    Compute similarity between current stack and goal stack.
    
    Higher is better. 1.0 = exact match. 0.0 = completely different.
    """
    if current == goal:
        return 1.0
    
    dist = stack_distance(current, goal)
    # Exponential decay for smooth gradient
    return math.exp(-dist)


def numeric_match_reward(current: List[Any], goal: List[Any]) -> float:
    """
    Simple reward for numeric tasks.
    
    1.0 if stacks match exactly
    Partial credit if close
    0.0 if wrong structure
    -1.0 if error
    """
    if current == goal:
        return 1.0
    
    if len(current) != len(goal):
        return 0.0
    
    # Check each element
    total_credit = 0.0
    for c, g in zip(current, goal):
        if c == g:
            total_credit += 1.0
        elif isinstance(c, (int, float)) and isinstance(g, (int, float)):
            # Partial credit for close numbers
            if g == 0:
                credit = 0.5 if abs(c) < 10 else 0.0
            else:
                error = abs(c - g) / abs(g)
                credit = max(0, 1.0 - error)
            total_credit += credit
        else:
            total_credit += 0.0
    
    return total_credit / len(goal) if goal else 0.0
