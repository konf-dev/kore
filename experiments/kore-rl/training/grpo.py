"""
GRPO (Group Relative Policy Optimization) for Kore
===================================================

From DeepSeek-R1: No critic model needed.
Uses group-relative rewards: compare samples within each group.

Key insight: For deterministic rewards (code execution), we don't need
a value function. We just need to know which samples are better.
"""

import torch
import torch.nn.functional as F
from torch import Tensor
from dataclasses import dataclass
from typing import List, Tuple, Optional
import math


@dataclass
class GRPOConfig:
    """GRPO hyperparameters"""
    # Sampling
    num_samples: int = 8        # Samples per prompt (K in paper)
    temperature: float = 0.7    # Sampling temperature
    top_p: float = 0.95         # Nucleus sampling
    max_new_tokens: int = 256   # Max generated tokens
    
    # Training
    learning_rate: float = 1e-6
    kl_coef: float = 0.1        # KL divergence penalty coefficient
    clip_range: float = 0.2     # PPO-style clipping (optional)
    
    # Normalization
    reward_baseline: str = "mean"  # "mean", "max", or "none"
    reward_scale: float = 1.0
    
    # Regularization
    entropy_coef: float = 0.01  # Entropy bonus
    max_grad_norm: float = 1.0  # Gradient clipping


class GRPOTrainer:
    """
    Group Relative Policy Optimization
    
    For each prompt, generate K samples, execute each in Kore runtime,
    compute rewards, then update policy to favor higher-reward samples.
    """
    
    def __init__(
        self,
        model: torch.nn.Module,
        ref_model: torch.nn.Module,
        tokenizer,
        config: GRPOConfig,
        optimizer: Optional[torch.optim.Optimizer] = None,
    ):
        self.model = model
        self.ref_model = ref_model  # Frozen reference for KL
        self.tokenizer = tokenizer
        self.config = config
        
        # Freeze reference model
        for param in self.ref_model.parameters():
            param.requires_grad = False
        
        # Optimizer
        if optimizer is None:
            self.optimizer = torch.optim.AdamW(
                model.parameters(),
                lr=config.learning_rate,
                betas=(0.9, 0.95),
                weight_decay=0.1,
            )
        else:
            self.optimizer = optimizer
    
    def generate_samples(
        self,
        prompts: List[str],
        num_samples: int = None,
    ) -> Tuple[List[List[str]], Tensor, Tensor]:
        """
        Generate K samples for each prompt.
        
        Returns:
            samples: List[List[str]] - K samples per prompt
            log_probs: Tensor[batch, K, seq] - Log probs under current policy
            ref_log_probs: Tensor[batch, K, seq] - Log probs under reference
        """
        if num_samples is None:
            num_samples = self.config.num_samples
        
        batch_size = len(prompts)
        all_samples = []
        all_log_probs = []
        all_ref_log_probs = []
        
        self.model.eval()
        
        with torch.no_grad():
            for prompt in prompts:
                # Tokenize prompt
                inputs = self.tokenizer(
                    prompt,
                    return_tensors="pt",
                    truncation=True,
                    max_length=1024,
                ).to(self.model.device)
                
                prompt_len = inputs.input_ids.shape[1]
                
                # Generate K samples
                samples = []
                sample_log_probs = []
                sample_ref_log_probs = []
                
                for _ in range(num_samples):
                    # Sample from model
                    outputs = self.model.generate(
                        **inputs,
                        max_new_tokens=self.config.max_new_tokens,
                        temperature=self.config.temperature,
                        top_p=self.config.top_p,
                        do_sample=True,
                        pad_token_id=self.tokenizer.pad_token_id,
                        return_dict_in_generate=True,
                        output_scores=True,
                    )
                    
                    generated_ids = outputs.sequences[0, prompt_len:]
                    generated_text = self.tokenizer.decode(
                        generated_ids, 
                        skip_special_tokens=True
                    )
                    samples.append(generated_text)
                    
                    # Compute log probs under current policy
                    full_ids = outputs.sequences
                    with torch.no_grad():
                        logits = self.model(full_ids).logits[:, prompt_len-1:-1]
                        log_probs = F.log_softmax(logits, dim=-1)
                        token_log_probs = log_probs.gather(
                            -1, generated_ids.unsqueeze(0).unsqueeze(-1)
                        ).squeeze(-1)
                        sample_log_probs.append(token_log_probs.sum().item())
                        
                        # Reference model log probs
                        ref_logits = self.ref_model(full_ids).logits[:, prompt_len-1:-1]
                        ref_log_probs = F.log_softmax(ref_logits, dim=-1)
                        ref_token_log_probs = ref_log_probs.gather(
                            -1, generated_ids.unsqueeze(0).unsqueeze(-1)
                        ).squeeze(-1)
                        sample_ref_log_probs.append(ref_token_log_probs.sum().item())
                
                all_samples.append(samples)
                all_log_probs.append(sample_log_probs)
                all_ref_log_probs.append(sample_ref_log_probs)
        
        return (
            all_samples,
            torch.tensor(all_log_probs),
            torch.tensor(all_ref_log_probs),
        )
    
    def compute_advantages(
        self,
        rewards: Tensor,  # [batch, K]
    ) -> Tensor:
        """
        Compute group-relative advantages.
        
        GRPO key insight: Normalize rewards within each group.
        No critic model needed!
        """
        if self.config.reward_baseline == "mean":
            # Subtract mean reward within group
            baseline = rewards.mean(dim=1, keepdim=True)
            advantages = rewards - baseline
        elif self.config.reward_baseline == "max":
            # Relative to best in group
            baseline = rewards.max(dim=1, keepdim=True).values
            advantages = rewards - baseline
        else:
            advantages = rewards
        
        # Scale
        advantages = advantages * self.config.reward_scale
        
        # Normalize across batch for stability
        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)
        
        return advantages
    
    def compute_loss(
        self,
        prompts: List[str],
        samples: List[List[str]],
        rewards: Tensor,  # [batch, K]
    ) -> Tuple[Tensor, dict]:
        """
        Compute GRPO loss.
        
        L = -E[advantage * log_prob] + kl_coef * KL(policy || ref)
        """
        batch_size = len(prompts)
        num_samples = len(samples[0])
        
        # Compute advantages
        advantages = self.compute_advantages(rewards)  # [batch, K]
        
        total_loss = 0.0
        total_policy_loss = 0.0
        total_kl_loss = 0.0
        total_entropy = 0.0
        
        self.model.train()
        
        for b in range(batch_size):
            prompt = prompts[b]
            
            for k in range(num_samples):
                sample = samples[b][k]
                advantage = advantages[b, k]
                
                # Tokenize full sequence
                full_text = prompt + sample
                inputs = self.tokenizer(
                    full_text,
                    return_tensors="pt",
                    truncation=True,
                    max_length=2048,
                ).to(self.model.device)
                
                prompt_inputs = self.tokenizer(
                    prompt,
                    return_tensors="pt",
                    truncation=True,
                ).to(self.model.device)
                prompt_len = prompt_inputs.input_ids.shape[1]
                
                # Forward pass
                outputs = self.model(**inputs)
                logits = outputs.logits[:, prompt_len-1:-1]  # Generated tokens only
                
                # Log probs
                log_probs = F.log_softmax(logits, dim=-1)
                target_ids = inputs.input_ids[:, prompt_len:]
                token_log_probs = log_probs.gather(-1, target_ids.unsqueeze(-1)).squeeze(-1)
                
                # Policy gradient loss: -advantage * log_prob
                policy_loss = -(advantage * token_log_probs.sum())
                total_policy_loss += policy_loss.item()
                
                # KL divergence from reference
                with torch.no_grad():
                    ref_outputs = self.ref_model(**inputs)
                    ref_logits = ref_outputs.logits[:, prompt_len-1:-1]
                    ref_log_probs = F.log_softmax(ref_logits, dim=-1)
                
                # KL = sum(exp(log_p) * (log_p - log_q))
                kl = (log_probs.exp() * (log_probs - ref_log_probs)).sum(dim=-1).mean()
                total_kl_loss += kl.item()
                
                # Entropy bonus
                entropy = -(log_probs.exp() * log_probs).sum(dim=-1).mean()
                total_entropy += entropy.item()
                
                # Total loss
                loss = policy_loss + self.config.kl_coef * kl - self.config.entropy_coef * entropy
                total_loss += loss
        
        # Average over samples
        n = batch_size * num_samples
        total_loss = total_loss / n
        
        metrics = {
            "loss": total_loss.item(),
            "policy_loss": total_policy_loss / n,
            "kl_loss": total_kl_loss / n,
            "entropy": total_entropy / n,
            "reward_mean": rewards.mean().item(),
            "reward_std": rewards.std().item(),
            "advantage_std": advantages.std().item(),
        }
        
        return total_loss, metrics
    
    def step(
        self,
        prompts: List[str],
        samples: List[List[str]],
        rewards: Tensor,
    ) -> dict:
        """
        Perform one GRPO update step.
        """
        self.optimizer.zero_grad()
        
        loss, metrics = self.compute_loss(prompts, samples, rewards)
        
        loss.backward()
        
        # Gradient clipping
        torch.nn.utils.clip_grad_norm_(
            self.model.parameters(),
            self.config.max_grad_norm,
        )
        
        self.optimizer.step()
        
        return metrics


def compute_kore_rewards(
    samples: List[List[str]],
    targets: List,
    runtime_client,
) -> Tensor:
    """
    Execute samples in Kore runtime and compute rewards.
    
    Args:
        samples: Generated Kore programs [batch, K]
        targets: Expected outputs [batch]
        runtime_client: HTTP client to Kore runtime container
    
    Returns:
        rewards: Tensor[batch, K]
    """
    batch_size = len(samples)
    num_samples = len(samples[0])
    rewards = torch.zeros(batch_size, num_samples)
    
    for b in range(batch_size):
        target = targets[b]
        
        for k in range(num_samples):
            program = samples[b][k]
            
            # Execute in Kore runtime
            result = runtime_client.execute(program)
            
            if not result.success:
                # Partial credit for how far it got
                rewards[b, k] = -1.0 + 0.1 * (result.steps_executed / 100)
            elif result.final_stack == [target]:
                # Perfect!
                rewards[b, k] = 1.0
            elif len(result.final_stack) == 1:
                # Partial credit for close answer
                rewards[b, k] = 0.5 * value_similarity(result.final_stack[0], target)
            else:
                # Wrong stack shape
                rewards[b, k] = -0.5
    
    return rewards


def value_similarity(got, expected) -> float:
    """Compute similarity between two values."""
    if got == expected:
        return 1.0
    
    if isinstance(got, (int, float)) and isinstance(expected, (int, float)):
        # Numeric similarity
        diff = abs(got - expected)
        return 1.0 / (1.0 + diff)
    
    if isinstance(got, str) and isinstance(expected, str):
        # String edit distance
        from difflib import SequenceMatcher
        return SequenceMatcher(None, got, expected).ratio()
    
    if isinstance(got, list) and isinstance(expected, list):
        if len(got) == 0 and len(expected) == 0:
            return 1.0
        if len(got) == 0 or len(expected) == 0:
            return 0.0
        # Element-wise similarity
        min_len = min(len(got), len(expected))
        sim = sum(value_similarity(g, e) for g, e in zip(got, expected)) / min_len
        # Penalize length mismatch
        length_penalty = min_len / max(len(got), len(expected))
        return sim * length_penalty
    
    return 0.0
