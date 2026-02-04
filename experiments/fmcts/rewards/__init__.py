"""
Reward functions for FMCTS.

Exploits Kore's observable stack semantics for dense rewards.
"""

from .stack_distance import stack_distance, stack_similarity
from .trace_reward import dense_reward, trace_reward, RewardConfig

__all__ = [
    "stack_distance",
    "stack_similarity", 
    "dense_reward",
    "trace_reward",
    "RewardConfig",
]
