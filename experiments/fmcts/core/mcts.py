"""
FMCTS: Fiber-Native Monte Carlo Tree Search

The main MCTS algorithm that exploits Kore's guarantees:
1. Deterministic execution → cache results
2. Observable stack → dense reward at every step
3. Trace semantics → supervised signal
4. Type safety → prune invalid actions
"""

from typing import List, Any, Optional, Tuple, Dict, Callable
import random
import math
import logging
from dataclasses import dataclass

from .node import MCTSNode, create_root
from .config import MCTSConfig

# Import from parent package - handle both cases
try:
    from ..executor import KoreExecutor, ExecuteResult
    from ..rewards import stack_distance, dense_reward, trace_reward
except ImportError:
    # When running directly
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).parent.parent))
    from executor import KoreExecutor, ExecuteResult
    from rewards import stack_distance, dense_reward, trace_reward

logger = logging.getLogger(__name__)


@dataclass
class SearchResult:
    """Result of MCTS search."""
    program: str                    # Best program found
    success: bool                   # Did it reach the goal?
    final_stack: List[Any]         # Final stack state
    goal: List[Any]                # Target stack
    root: MCTSNode                 # Root of search tree
    nodes_explored: int            # Total nodes in tree
    simulations: int               # Total MCTS simulations
    trace: List[Dict]              # Execution trace of best program
    
    def to_dict(self) -> Dict:
        return {
            "program": self.program,
            "success": self.success,
            "final_stack": self.final_stack,
            "goal": self.goal,
            "nodes_explored": self.nodes_explored,
            "simulations": self.simulations,
        }


class FMCTS:
    """
    Fiber-Native Monte Carlo Tree Search.
    
    Key innovations:
    1. Uses real Kore execution (not simulation)
    2. Caches execution results (determinism guarantee)
    3. Dense reward from stack distance (observable state)
    4. Type-based action pruning (static types)
    """
    
    def __init__(
        self,
        config: MCTSConfig,
        executor: Optional[KoreExecutor] = None,
        policy: Optional[Callable] = None,
    ):
        self.config = config
        
        # Create executor if not provided
        if executor is None:
            self.executor = KoreExecutor(
                use_docker=config.use_docker,
                container_name=config.container_name,
                kore_binary=config.kore_binary,
                timeout=config.execution_timeout,
            )
        else:
            self.executor = executor
        
        # Policy for action selection (defaults to uniform)
        self.policy = policy or self._uniform_policy
        
        # Stats
        self.total_simulations = 0
        self.total_nodes = 0
    
    def search(
        self,
        goal: List[Any],
        initial_stack: List[Any] = None,
        max_simulations: Optional[int] = None,
    ) -> SearchResult:
        """
        Search for a program that produces the goal stack.
        
        Args:
            goal: Target stack state
            initial_stack: Starting stack (default: empty)
            max_simulations: Override config.simulations_per_move * max_depth
        
        Returns:
            SearchResult with best program found
        """
        if initial_stack is None:
            initial_stack = []
        
        # Create root node
        root = create_root()
        root.stack = tuple(initial_stack)
        
        # Calculate total simulations
        if max_simulations is None:
            max_simulations = self.config.simulations_per_move * self.config.max_depth
        
        # Main MCTS loop
        for sim in range(max_simulations):
            self.total_simulations += 1
            
            # 1. SELECTION: Find promising node
            node = self._select(root, goal)
            
            # 2. Check if we found the solution
            if list(node.stack) == goal:
                logger.info(f"Found solution in {sim+1} simulations: {node.program}")
                return self._build_result(node, goal, root, sim + 1)
            
            # 3. EXPANSION: Add a new child
            if not node.is_terminal and node.depth() < self.config.max_depth:
                child = self._expand(node, goal)
                if child is not None:
                    node = child
                    
                    # Check if expansion found solution
                    if list(node.stack) == goal:
                        logger.info(f"Found solution in {sim+1} simulations: {node.program}")
                        return self._build_result(node, goal, root, sim + 1)
            
            # 4. SIMULATION: Evaluate the node
            value = self._evaluate(node, goal)
            
            # 5. BACKPROPAGATION: Update tree
            node.backpropagate(value)
            
            # Logging
            if self.config.verbose and (sim + 1) % self.config.log_every == 0:
                best = root.most_visited_child()
                best_prog = best[1].program if best else ""
                logger.info(
                    f"Sim {sim+1}: nodes={self._count_nodes(root)}, "
                    f"best='{best_prog[:30]}...'"
                )
        
        # Return best program found
        return self._build_best_result(root, goal, max_simulations)
    
    def _select(self, node: MCTSNode, goal: List[Any]) -> MCTSNode:
        """
        SELECT phase: Traverse tree using UCB1 until reaching a leaf.
        """
        while node.children and not node.is_terminal:
            # Check if fully expanded
            vocab = self._get_valid_actions(node)
            if len(node.children) < len(vocab):
                return node  # Not fully expanded, expand here
            
            # Select best child by UCB1
            node = node.best_child(self.config.exploration_constant)
        
        return node
    
    def _expand(self, node: MCTSNode, goal: List[Any]) -> Optional[MCTSNode]:
        """
        EXPAND phase: Add one new child to the node.
        
        Uses policy to select which action to try.
        """
        vocab = self._get_valid_actions(node)
        
        # Filter out already-tried actions
        untried = [a for a in vocab if a not in node.children]
        
        if not untried:
            return None
        
        # Use policy to select action
        action = self.policy(node, goal, untried)
        
        # Execute action using REAL Kore
        result = self.executor.execute_step(node.program, action)
        
        # Create child node with execution result
        child = node.add_child(
            token=action,
            stack=tuple(result.final_stack),
            trace=[t.to_dict() for t in result.trace],
            success=result.success,
            error=result.error,
        )
        
        self.total_nodes += 1
        
        return child
    
    def _evaluate(self, node: MCTSNode, goal: List[Any]) -> float:
        """
        EVALUATE phase: Compute value of a node.
        
        Uses dense reward based on stack distance.
        """
        if not node.success:
            return -self.config.error_penalty
        
        if list(node.stack) == goal:
            # Found solution!
            length_bonus = 1.0 - (node.depth() / self.config.max_depth)
            return self.config.terminal_bonus * (1.0 + length_bonus)
        
        # Use stack distance for partial credit
        similarity = 1.0 / (1.0 + stack_distance(list(node.stack), goal))
        
        # Penalty for program length (encourage shorter programs)
        length_penalty = node.depth() * self.config.step_cost
        
        return similarity - length_penalty
    
    def _get_valid_actions(self, node: MCTSNode) -> List[str]:
        """
        Get valid actions from this node.
        
        Uses type-based pruning if enabled.
        """
        vocab = self.config.vocab.copy()
        
        if not self.config.use_type_pruning:
            return vocab
        
        # Filter by stack requirements
        stack = list(node.stack)
        stack_size = len(stack)
        
        valid = []
        for token in vocab:
            # Stack size requirements for operations
            requirements = {
                "dup": 1, "drop": 1, "swap": 2, "over": 2, "rot": 3,
                "nip": 2, "tuck": 2,
                "add": 2, "sub": 2, "mul": 2, "div": 2, "mod": 2,
                "neg": 1, "abs": 1, "min": 2, "max": 2,
                "eq": 2, "neq": 2, "lt": 2, "gt": 2, "le": 2, "ge": 2,
                "and": 2, "or": 2, "not": 1,
            }
            
            req = requirements.get(token, 0)
            if stack_size >= req:
                # Additional check: division by zero
                if token == "div" and stack_size >= 2 and stack[-1] == 0:
                    continue
                valid.append(token)
        
        return valid if valid else vocab  # Fallback to full vocab
    
    def _uniform_policy(
        self,
        node: MCTSNode,
        goal: List[Any],
        actions: List[str]
    ) -> str:
        """Default uniform random policy."""
        return random.choice(actions)
    
    def _build_result(
        self,
        node: MCTSNode,
        goal: List[Any],
        root: MCTSNode,
        simulations: int,
    ) -> SearchResult:
        """Build SearchResult from a solution node."""
        # Get full trace by re-executing
        result = self.executor.execute(node.program, trace=True)
        
        return SearchResult(
            program=node.program,
            success=list(node.stack) == goal,
            final_stack=list(node.stack),
            goal=goal,
            root=root,
            nodes_explored=self._count_nodes(root),
            simulations=simulations,
            trace=[t.to_dict() for t in result.trace],
        )
    
    def _build_best_result(
        self,
        root: MCTSNode,
        goal: List[Any],
        simulations: int,
    ) -> SearchResult:
        """Build SearchResult from best node found."""
        # Find best leaf by value
        best_node = self._find_best_leaf(root)
        
        # Get full trace
        result = self.executor.execute(best_node.program, trace=True)
        
        return SearchResult(
            program=best_node.program,
            success=list(best_node.stack) == goal,
            final_stack=list(best_node.stack),
            goal=goal,
            root=root,
            nodes_explored=self._count_nodes(root),
            simulations=simulations,
            trace=[t.to_dict() for t in result.trace],
        )
    
    def _find_best_leaf(self, node: MCTSNode) -> MCTSNode:
        """Find leaf with highest value."""
        if not node.children:
            return node
        
        best = node
        best_value = node.value
        
        for child in node.children.values():
            leaf = self._find_best_leaf(child)
            if leaf.value > best_value:
                best = leaf
                best_value = leaf.value
        
        return best
    
    def _count_nodes(self, node: MCTSNode) -> int:
        """Count total nodes in tree."""
        count = 1
        for child in node.children.values():
            count += self._count_nodes(child)
        return count
    
    def stats(self) -> Dict:
        """Get search statistics."""
        return {
            "total_simulations": self.total_simulations,
            "total_nodes": self.total_nodes,
            "executor_stats": self.executor.stats(),
        }
