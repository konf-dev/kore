"""
MCTS Configuration for FMCTS.
"""

from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class MCTSConfig:
    """Configuration for FMCTS algorithm."""
    
    # === MCTS Parameters ===
    exploration_constant: float = 1.414  # UCB1 c parameter (sqrt(2))
    simulations_per_move: int = 100      # MCTS iterations per token
    max_depth: int = 50                  # Maximum program length
    
    # === Kore-Specific Optimizations ===
    use_type_pruning: bool = True        # Filter invalid actions by type
    use_fiber_cache: bool = True         # Memoize execution results
    use_trace_reward: bool = True        # Dense reward from traces
    use_algebraic_norm: bool = True      # Canonicalize programs
    
    # === Execution ===
    use_docker: bool = False             # Use Docker executor (vs local)
    container_name: str = "kore-runtime" # Docker container name
    kore_binary: str = "kore-train"      # Local binary path
    execution_timeout: float = 5.0       # Seconds per execution
    max_steps: int = 10000               # Max Kore steps (tick budget)
    
    # === Policy ===
    policy_type: str = "llm"             # "llm", "random", "uniform"
    model_name: str = "Qwen/Qwen2.5-0.5B-Instruct"
    temperature: float = 0.8             # Sampling temperature
    
    # === Reward ===
    terminal_bonus: float = 10.0         # Bonus for reaching goal
    step_cost: float = 0.01              # Penalty per step (encourages brevity)
    error_penalty: float = 2.0           # Penalty for execution errors
    
    # === Vocabulary ===
    vocab: List[str] = field(default_factory=lambda: [
        # Literals
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
        "-1", "-2",
        "true", "false", "null",
        # Stack manipulation
        "dup", "drop", "swap", "over", "rot", "nip", "tuck",
        # Arithmetic
        "add", "sub", "mul", "div", "mod", "neg", "abs", "min", "max",
        # Comparison
        "eq", "neq", "lt", "gt", "le", "ge",
        # Logic
        "and", "or", "not",
    ])
    
    # Extended vocab for control flow
    extended_vocab: List[str] = field(default_factory=lambda: [
        "[", "]",  # Quotation delimiters
        "call", "if", "times", "while", "dip",
    ])
    
    # === Caching ===
    cache_max_size: int = 100_000        # Max cached fiber states
    
    # === Logging ===
    verbose: bool = False
    log_every: int = 10                  # Log every N simulations
    
    def get_full_vocab(self) -> List[str]:
        """Get vocabulary including control flow."""
        return self.vocab + self.extended_vocab


# Pre-configured settings for different scenarios
FAST_CONFIG = MCTSConfig(
    simulations_per_move=50,
    max_depth=20,
    use_trace_reward=False,
)

THOROUGH_CONFIG = MCTSConfig(
    simulations_per_move=500,
    max_depth=100,
    exploration_constant=2.0,
)

DEBUG_CONFIG = MCTSConfig(
    simulations_per_move=10,
    max_depth=10,
    verbose=True,
    log_every=1,
)
