"""
LLM Policy for FMCTS.

Uses a language model to predict next tokens with learned priors.
This is the key component that makes FMCTS work well - the LLM
provides informed priors that guide the search toward likely programs.
"""

import torch
from typing import Dict, List, Optional
from dataclasses import dataclass

from .base import Policy, PolicyResult


@dataclass 
class LLMConfig:
    """Configuration for LLM policy."""
    model_name: str = "Qwen/Qwen2.5-0.5B-Instruct"
    device: str = "cuda"
    max_length: int = 512
    temperature: float = 1.0
    dtype: str = "bfloat16"


class LLMPolicy(Policy):
    """
    LLM-based policy for FMCTS.
    
    The model is prompted with the goal and program-so-far,
    then we extract the probability distribution over the
    vocabulary tokens for the next position.
    """
    
    def __init__(self, config: Optional[LLMConfig] = None):
        self.config = config or LLMConfig()
        self.model = None
        self.tokenizer = None
        self._vocab_token_ids: Dict[str, int] = {}
        
    def _lazy_load(self):
        """Lazy load model on first use."""
        if self.model is not None:
            return
            
        from transformers import AutoModelForCausalLM, AutoTokenizer
        
        dtype_map = {
            "float32": torch.float32,
            "float16": torch.float16,
            "bfloat16": torch.bfloat16,
        }
        dtype = dtype_map.get(self.config.dtype, torch.bfloat16)
        
        print(f"Loading LLM: {self.config.model_name}")
        self.tokenizer = AutoTokenizer.from_pretrained(
            self.config.model_name,
            trust_remote_code=True,
        )
        self.model = AutoModelForCausalLM.from_pretrained(
            self.config.model_name,
            torch_dtype=dtype,
            device_map=self.config.device,
            trust_remote_code=True,
        )
        self.model.eval()
        print(f"Loaded on {self.config.device}")
        
    def _get_token_id(self, token: str) -> Optional[int]:
        """Get token ID for a vocabulary token."""
        if token in self._vocab_token_ids:
            return self._vocab_token_ids[token]
            
        # Try with and without space prefix
        candidates = [token, f" {token}", f"\n{token}"]
        for candidate in candidates:
            ids = self.tokenizer.encode(candidate, add_special_tokens=False)
            if len(ids) == 1:
                self._vocab_token_ids[token] = ids[0]
                return ids[0]
                
        # Fallback: use first token of encoding
        ids = self.tokenizer.encode(f" {token}", add_special_tokens=False)
        if ids:
            self._vocab_token_ids[token] = ids[0]
            return ids[0]
            
        return None
        
    def _format_prompt(self, program_so_far: str, goal: List) -> str:
        """Format input prompt for the model."""
        goal_str = " ".join(str(x) for x in goal)
        
        prompt = f"""You are a stack-based programming expert. Generate a Kore program that produces the goal stack.

Goal stack (bottom to top): [{goal_str}]

Instructions:
- Write tokens separated by spaces
- Numbers push to stack
- 'add', 'sub', 'mul', 'div' operate on top two elements
- 'dup' duplicates top, 'drop' removes top, 'swap' swaps top two

Program: {program_so_far}"""
        return prompt
    
    def get_priors(
        self,
        program_so_far: str,
        goal: List,
        vocab: List[str],
        top_k: int = 10,
    ) -> PolicyResult:
        """Get prior probabilities from LLM."""
        self._lazy_load()
        
        prompt = self._format_prompt(program_so_far, goal)
        
        # Tokenize
        inputs = self.tokenizer(
            prompt,
            return_tensors="pt",
            truncation=True,
            max_length=self.config.max_length,
        ).to(self.config.device)
        
        # Get logits for next token
        with torch.no_grad():
            outputs = self.model(**inputs)
            logits = outputs.logits[0, -1, :]  # Last position
            
        # Apply temperature
        if self.config.temperature != 1.0:
            logits = logits / self.config.temperature
            
        # Softmax to probabilities
        probs = torch.softmax(logits, dim=-1)
        
        # Extract probabilities for vocabulary tokens
        priors = {}
        for token in vocab:
            token_id = self._get_token_id(token)
            if token_id is not None:
                priors[token] = probs[token_id].item()
            else:
                priors[token] = 1e-6  # Fallback
                
        # Normalize
        total = sum(priors.values())
        if total > 0:
            priors = {k: v / total for k, v in priors.items()}
            
        # Sort by probability
        sorted_tokens = sorted(priors.keys(), key=lambda x: priors[x], reverse=True)
        top_tokens = sorted_tokens[:top_k]
        
        return PolicyResult(
            priors=priors,
            top_tokens=top_tokens,
            prefix_logprob=0.0,  # TODO: compute actual prefix logprob
        )
    
    def batch_get_priors(
        self,
        prefixes: List[str],
        goals: List[List],
        vocab: List[str],
        top_k: int = 10,
    ) -> List[PolicyResult]:
        """Batched priors for efficiency."""
        self._lazy_load()
        
        # Format all prompts
        prompts = [
            self._format_prompt(prefix, goal)
            for prefix, goal in zip(prefixes, goals)
        ]
        
        # Tokenize with padding
        inputs = self.tokenizer(
            prompts,
            return_tensors="pt",
            padding=True,
            truncation=True,
            max_length=self.config.max_length,
        ).to(self.config.device)
        
        # Get logits
        with torch.no_grad():
            outputs = self.model(**inputs)
            
        results = []
        for i in range(len(prefixes)):
            # Find last non-padding position
            attention_mask = inputs.attention_mask[i]
            last_pos = attention_mask.sum().item() - 1
            
            logits = outputs.logits[i, last_pos, :]
            
            if self.config.temperature != 1.0:
                logits = logits / self.config.temperature
                
            probs = torch.softmax(logits, dim=-1)
            
            priors = {}
            for token in vocab:
                token_id = self._get_token_id(token)
                if token_id is not None:
                    priors[token] = probs[token_id].item()
                else:
                    priors[token] = 1e-6
                    
            total = sum(priors.values())
            if total > 0:
                priors = {k: v / total for k, v in priors.items()}
                
            sorted_tokens = sorted(priors.keys(), key=lambda x: priors[x], reverse=True)
            
            results.append(PolicyResult(
                priors=priors,
                top_tokens=sorted_tokens[:top_k],
                prefix_logprob=0.0,
            ))
            
        return results
