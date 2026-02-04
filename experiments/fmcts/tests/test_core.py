#!/usr/bin/env python3
"""
Tests for FMCTS core components.
"""

import pytest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from fmcts.core.node import MCTSNode, create_root
from fmcts.core.config import MCTSConfig
from fmcts.rewards.stack_distance import stack_distance, stack_similarity
from fmcts.rewards.trace_reward import dense_reward, trace_reward


class TestMCTSNode:
    """Tests for MCTSNode."""
    
    def test_create_root(self):
        root = create_root()
        assert root.program == ""
        assert root.stack == ()
        assert root.parent is None
        assert root.visits == 0
        assert root.value == 0.0
    
    def test_add_child(self):
        root = create_root()
        child = root.add_child(
            token="3",
            stack=(3,),
            trace=[],
            success=True,
        )
        
        assert child.program == "3"
        assert child.stack == (3,)
        assert child.parent == root
        assert "3" in root.children
    
    def test_ucb1_unvisited(self):
        root = create_root()
        root.visits = 10
        child = root.add_child("1", (1,), [], True)
        
        # Unvisited child should have infinite UCB1
        assert child.ucb1() == float('inf')
    
    def test_ucb1_visited(self):
        root = create_root()
        root.visits = 10
        child = root.add_child("1", (1,), [], True)
        child.visits = 5
        child.total_value = 2.5
        
        ucb = child.ucb1(c=1.414)
        assert 0 < ucb < 2  # Should be reasonable value
    
    def test_backpropagate(self):
        root = create_root()
        child = root.add_child("3", (3,), [], True)
        grandchild = child.add_child("4", (3, 4), [], True)
        
        grandchild.backpropagate(1.0)
        
        assert grandchild.visits == 1
        assert grandchild.total_value == 1.0
        assert child.visits == 1
        assert child.total_value == 1.0
        assert root.visits == 1
        assert root.total_value == 1.0
    
    def test_depth(self):
        root = create_root()
        assert root.depth() == 0
        
        child = root.add_child("3", (3,), [], True)
        assert child.depth() == 1
        
        grandchild = child.add_child("4", (3, 4), [], True)
        assert grandchild.depth() == 2


class TestStackDistance:
    """Tests for stack distance reward."""
    
    def test_exact_match(self):
        assert stack_distance([7], [7]) == 0.0
        assert stack_distance([1, 2, 3], [1, 2, 3]) == 0.0
        assert stack_distance([], []) == 0.0
    
    def test_different_length(self):
        dist = stack_distance([1], [1, 2])
        assert dist > 0  # Length difference should add penalty
    
    def test_different_values(self):
        dist = stack_distance([5], [7])
        assert 0 < dist < 1  # Close values should have small distance
    
    def test_similarity(self):
        assert stack_similarity([7], [7]) == 1.0
        assert 0 < stack_similarity([5], [7]) < 1.0


class TestDenseReward:
    """Tests for dense reward computation."""
    
    def test_positive_progress(self):
        # Moving from [3] to [3, 4] when goal is [7]
        # Not directly helping, so reward might be small
        reward = dense_reward([3], [3, 4], [7])
        # Reward depends on implementation
        assert isinstance(reward, float)
    
    def test_reaching_goal(self):
        # Reaching the goal should give bonus
        reward = dense_reward([3, 4], [7], [7])
        assert reward > 0  # Should be positive (reached goal)
    
    def test_error_state(self):
        # Moving away from goal
        reward = dense_reward([7], [1], [7])
        assert reward < 1  # Should not be maximum


class TestConfig:
    """Tests for MCTSConfig."""
    
    def test_default_config(self):
        config = MCTSConfig()
        assert config.simulations_per_move == 100
        assert config.max_depth == 50
        assert len(config.vocab) > 0
    
    def test_full_vocab(self):
        config = MCTSConfig()
        full_vocab = config.get_full_vocab()
        assert "add" in full_vocab
        assert "if" in full_vocab


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
