"""
FMCTS: Fiber-Native Monte Carlo Tree Search for Kore Program Synthesis

Exploits Kore's mathematical guarantees for efficient program synthesis:
- Deterministic execution → no sampling variance
- Observable stack → dense reward
- Trace semantics → supervised signal
- Fiber immutability → O(1) backtracking
"""

from .core import FMCTS, MCTSConfig, MCTSNode
from .executor import KoreExecutor
from .rewards import stack_distance, dense_reward, trace_reward

__all__ = [
    "FMCTS",
    "MCTSConfig",
    "MCTSNode",
    "KoreExecutor",
    "stack_distance",
    "dense_reward",
    "trace_reward",
]

__version__ = "0.1.0"
