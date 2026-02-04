"""
Base policy interface for FMCTS.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Dict, List, Optional


@dataclass
class PolicyResult:
    """Result from a policy query."""
    
    # Token -> probability
    priors: Dict[str, float]
    
    # Top tokens by probability
    top_tokens: List[str]
    
    # Log probability of the entire prefix
    prefix_logprob: float = 0.0
    
    # Raw logits (for training)
    logits: Optional[List[float]] = None


class Policy(ABC):
    """Abstract base class for policies."""
    
    @abstractmethod
    def get_priors(
        self,
        program_so_far: str,
        goal: List,
        vocab: List[str],
        top_k: int = 10,
    ) -> PolicyResult:
        """
        Get prior probabilities for next tokens.
        
        Args:
            program_so_far: Current program prefix (may be empty)
            goal: Target stack state
            vocab: Available tokens
            top_k: Return top-k tokens by probability
            
        Returns:
            PolicyResult with priors and top tokens
        """
        pass
    
    @abstractmethod
    def batch_get_priors(
        self,
        prefixes: List[str],
        goals: List[List],
        vocab: List[str],
        top_k: int = 10,
    ) -> List[PolicyResult]:
        """Batched version for efficiency."""
        pass
    
    def format_prompt(self, program_so_far: str, goal: List) -> str:
        """Format the input for the policy model."""
        return f"Goal: {goal}\nProgram: {program_so_far}"
