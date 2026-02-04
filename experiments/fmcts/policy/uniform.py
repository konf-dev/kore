"""
Uniform policy - random action selection.

Useful as a baseline and for exploration.
"""

from typing import Dict, List

from .base import Policy, PolicyResult


class UniformPolicy(Policy):
    """Uniform random policy over vocabulary."""
    
    def get_priors(
        self,
        program_so_far: str,
        goal: List,
        vocab: List[str],
        top_k: int = 10,
    ) -> PolicyResult:
        """Return uniform probabilities."""
        n = len(vocab)
        prob = 1.0 / n if n > 0 else 0.0
        
        priors = {token: prob for token in vocab}
        top_tokens = vocab[:top_k]
        
        return PolicyResult(
            priors=priors,
            top_tokens=top_tokens,
            prefix_logprob=0.0,
        )
    
    def batch_get_priors(
        self,
        prefixes: List[str],
        goals: List[List],
        vocab: List[str],
        top_k: int = 10,
    ) -> List[PolicyResult]:
        """Batched uniform priors."""
        return [
            self.get_priors(prefix, goal, vocab, top_k)
            for prefix, goal in zip(prefixes, goals)
        ]
