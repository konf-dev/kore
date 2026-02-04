"""
MCTS Node for FMCTS.

Each node represents a state in the search tree:
- The program generated so far
- The resulting stack state (from real Kore execution)
- The execution trace (for supervision)
"""

from dataclasses import dataclass, field
from typing import Dict, List, Any, Optional, Tuple
import math


@dataclass
class MCTSNode:
    """
    A node in the MCTS tree.
    
    Kore Advantage: We store the actual execution result (stack, trace)
    because Kore is deterministic. No need to re-execute on revisit.
    """
    
    # === Program State ===
    program: str                          # Kore code so far (space-separated tokens)
    stack: Tuple[Any, ...]               # Current stack state (from Kore execution)
    trace: List[Dict] = field(default_factory=list)  # Execution trace
    
    # === Tree Structure ===
    parent: Optional["MCTSNode"] = None
    children: Dict[str, "MCTSNode"] = field(default_factory=dict)
    
    # === MCTS Statistics ===
    visits: int = 0
    total_value: float = 0.0
    
    # === Execution Status ===
    success: bool = True                  # Did execution succeed?
    error: Optional[str] = None           # Error message if failed
    is_terminal: bool = False             # Reached goal or error?
    
    @property
    def value(self) -> float:
        """Average value of this node."""
        if self.visits == 0:
            return 0.0
        return self.total_value / self.visits
    
    def ucb1(self, c: float = 1.414) -> float:
        """
        Upper Confidence Bound for Trees (UCB1).
        
        Balances exploitation (high value) with exploration (low visits).
        
        UCB1(n) = V(n)/N(n) + c * sqrt(ln(N_parent) / N(n))
        """
        if self.visits == 0:
            return float('inf')  # Always explore unvisited
        
        if self.parent is None:
            return self.value
        
        exploitation = self.value
        exploration = c * math.sqrt(math.log(self.parent.visits) / self.visits)
        
        return exploitation + exploration
    
    def is_fully_expanded(self, vocab: List[str]) -> bool:
        """Check if all children have been created."""
        return len(self.children) >= len(vocab)
    
    def best_child(self, c: float = 1.414) -> Optional["MCTSNode"]:
        """Select child with highest UCB1 value."""
        if not self.children:
            return None
        return max(self.children.values(), key=lambda n: n.ucb1(c))
    
    def most_visited_child(self) -> Optional[Tuple[str, "MCTSNode"]]:
        """Select child with most visits (for final selection)."""
        if not self.children:
            return None
        return max(self.children.items(), key=lambda x: x[1].visits)
    
    def add_child(
        self,
        token: str,
        stack: Tuple[Any, ...],
        trace: List[Dict],
        success: bool = True,
        error: Optional[str] = None,
    ) -> "MCTSNode":
        """
        Add a child node for the given token.
        
        Kore Advantage: We already have the execution result,
        no need to simulate later.
        """
        new_program = f"{self.program} {token}".strip() if self.program else token
        
        child = MCTSNode(
            program=new_program,
            stack=stack,
            trace=trace,
            parent=self,
            success=success,
            error=error,
            is_terminal=not success,  # Errors are terminal
        )
        
        self.children[token] = child
        return child
    
    def backpropagate(self, value: float):
        """
        Propagate value up to root.
        
        Standard MCTS backpropagation.
        """
        node = self
        while node is not None:
            node.visits += 1
            node.total_value += value
            node = node.parent
    
    def get_path(self) -> List[str]:
        """Get list of tokens from root to this node."""
        tokens = []
        node = self
        while node is not None and node.program:
            # Get last token in program
            parts = node.program.split()
            if parts:
                tokens.append(parts[-1])
            node = node.parent
        return list(reversed(tokens))
    
    def depth(self) -> int:
        """Get depth of this node (0 for root)."""
        return len(self.program.split()) if self.program else 0
    
    def __repr__(self) -> str:
        return (
            f"MCTSNode(program='{self.program[:30]}...', "
            f"stack={list(self.stack)[:3]}, "
            f"visits={self.visits}, value={self.value:.3f})"
        )
    
    def to_dict(self) -> Dict:
        """Convert to dictionary for serialization."""
        return {
            "program": self.program,
            "stack": list(self.stack),
            "visits": self.visits,
            "value": self.value,
            "success": self.success,
            "error": self.error,
            "num_children": len(self.children),
        }


def create_root() -> MCTSNode:
    """Create an empty root node."""
    return MCTSNode(
        program="",
        stack=(),
        trace=[],
        parent=None,
    )
