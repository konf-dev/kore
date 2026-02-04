"""
FMCTS: Fiber-Native Monte Carlo Tree Search for Kore Program Synthesis

This module implements MCTS that exploits Kore's mathematical guarantees:
- Deterministic execution (no sampling variance)
- Observable stack state (dense reward)
- Trace semantics (supervised signal at every step)
- Fiber immutability (O(1) backtracking via copy)

Components:
- core: MCTS algorithm with fiber integration
- rewards: Stack distance and trace-based rewards
- fiber_cache: Memoization of execution states
- type_pruner: Valid action filtering
- trace_extractor: Convert traces to supervision
"""

from .core import FMCTS, MCTSConfig, MCTSNode
from .rewards import stack_distance, trace_reward, dense_reward
from .fiber_cache import FiberCache
from .type_pruner import valid_tokens, get_token_requirements
from .trace_extractor import extract_supervision, SupervisionPair

__all__ = [
    # Core MCTS
    "FMCTS",
    "MCTSConfig", 
    "MCTSNode",
    # Rewards
    "stack_distance",
    "trace_reward",
    "dense_reward",
    # Caching
    "FiberCache",
    # Type pruning
    "valid_tokens",
    "get_token_requirements",
    # Trace supervision
    "extract_supervision",
    "SupervisionPair",
]

__version__ = "0.1.0"
