#!/usr/bin/env python3
"""
Kore Training Loop with Incremental Execution

Architecture:
    LLM → token → KoreSimulator.step() → (stack, trace, error) → loss

Key Features:
1. Execute tokens as they're generated (not after)
2. Rich error signals for learning
3. Stack state conditioning for next token
4. Full trace for analysis

This is fundamentally different from standard code generation:
- We catch errors immediately, not after full generation
- LLM can see actual stack state at each step
- Partial credit based on execution progress
"""

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader
from transformers import AutoTokenizer, AutoModelForCausalLM
from typing import List, Dict, Tuple, Optional, Any
from dataclasses import dataclass
import json
import random
from pathlib import Path

from kore_sim import KoreSimulator, IncrementalExecutor, ExecutionResult


# ============================================================================
# Training Data
# ============================================================================

@dataclass
class TrainingSample:
    """A single training example."""
    goal_stack: List[Any]           # What we want to achieve
    target_program: str             # A correct solution
    initial_stack: List[Any] = None # Starting stack (default empty)


def generate_arithmetic_samples(n: int = 1000) -> List[TrainingSample]:
    """Generate arithmetic training samples."""
    samples = []
    sim = KoreSimulator()
    
    # Simple operations
    for _ in range(n // 4):
        a = random.randint(0, 10)
        b = random.randint(0, 10)
        
        # Addition
        result = sim.execute(f"{a} {b} add")
        if result.success:
            samples.append(TrainingSample(
                goal_stack=result.final_stack,
                target_program=f"{a} {b} add"
            ))
        
        # Multiplication
        result = sim.execute(f"{a} {b} mul")
        if result.success:
            samples.append(TrainingSample(
                goal_stack=result.final_stack,
                target_program=f"{a} {b} mul"
            ))
        
        # Compound
        if b != 0:
            result = sim.execute(f"{a} {b} add 2 mul")
            if result.success:
                samples.append(TrainingSample(
                    goal_stack=result.final_stack,
                    target_program=f"{a} {b} add 2 mul"
                ))
    
    return samples


# ============================================================================
# Kore Vocabulary
# ============================================================================

KORE_VOCAB = [
    # Numbers
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    # Stack ops
    "dup", "drop", "swap", "over", "rot", "nip",
    # Arithmetic
    "add", "sub", "mul", "div", "mod", "neg",
    # Comparison
    "eq", "neq", "lt", "gt", "le", "ge",
    # Logic
    "and", "or", "not",
    # Control
    "call", "if",
    # Special
    "[", "]", "def",
    # Boolean literals
    "true", "false", "null",
    # End of program
    "<END>",
]

KORE_TOKEN_TO_ID = {t: i for i, t in enumerate(KORE_VOCAB)}
KORE_ID_TO_TOKEN = {i: t for i, t in enumerate(KORE_VOCAB)}


# ============================================================================
# Loss Functions
# ============================================================================

def compute_execution_loss(
    result: ExecutionResult,
    goal_stack: List[Any],
    tokens_generated: List[str],
) -> Tuple[float, Dict[str, Any]]:
    """
    Compute loss based on execution result.
    
    Returns: (loss, info_dict)
    
    Loss components:
    1. Success bonus: -1.0 if correct, 0.0 otherwise
    2. Stack distance: How far from goal
    3. Error penalty: +0.5 if error occurred
    4. Length penalty: Small penalty for longer programs
    """
    info = {
        "success": result.success,
        "stack": result.final_stack,
        "goal": goal_stack,
        "error": result.error,
        "tokens": len(tokens_generated),
    }
    
    loss = 0.0
    
    # Success bonus (negative = good)
    if result.success and result.final_stack == goal_stack:
        loss -= 1.0
        info["correct"] = True
    else:
        info["correct"] = False
    
    # Stack distance penalty
    stack_dist = compute_stack_distance(result.final_stack, goal_stack)
    loss += stack_dist * 0.5
    info["stack_distance"] = stack_dist
    
    # Error penalty
    if result.error:
        loss += 0.5
        info["error_penalty"] = 0.5
    
    # Length penalty (encourage shorter solutions)
    length_penalty = len(tokens_generated) * 0.01
    loss += length_penalty
    info["length_penalty"] = length_penalty
    
    return loss, info


def compute_stack_distance(s1: List[Any], s2: List[Any]) -> float:
    """Distance metric between stacks."""
    # Length difference
    len_diff = abs(len(s1) - len(s2))
    
    # Value differences
    val_diff = 0.0
    for i in range(min(len(s1), len(s2))):
        v1, v2 = s1[i], s2[i]
        if isinstance(v1, (int, float)) and isinstance(v2, (int, float)):
            val_diff += min(abs(v1 - v2) / (1 + abs(v2)), 1.0)
        elif v1 != v2:
            val_diff += 1.0
    
    return len_diff + val_diff


def compute_step_rewards(
    trace: List[dict],
    goal_stack: List[Any],
) -> List[float]:
    """
    Compute per-step rewards from execution trace.
    
    For reinforcement learning with partial credit.
    """
    rewards = []
    
    for i, entry in enumerate(trace):
        if not entry["success"]:
            rewards.append(-1.0)  # Error = negative reward
            continue
        
        # Reward for moving closer to goal
        dist_before = compute_stack_distance(entry["stack_before"], goal_stack)
        dist_after = compute_stack_distance(entry["stack_after"], goal_stack)
        
        improvement = dist_before - dist_after
        
        if entry["stack_after"] == goal_stack:
            rewards.append(1.0)  # Reached goal
        elif improvement > 0:
            rewards.append(0.1 * improvement)  # Moved closer
        elif improvement < 0:
            rewards.append(-0.1)  # Moved away
        else:
            rewards.append(0.0)  # Neutral
    
    return rewards


# ============================================================================
# Training Loop
# ============================================================================

class KoreTrainer:
    """
    Training loop for Kore program generation.
    
    Key innovation: Execute during generation, not after.
    """
    
    def __init__(
        self,
        model_name: str = "Qwen/Qwen2-0.5B",
        device: str = "cuda",
        max_tokens: int = 20,
    ):
        self.device = device
        self.max_tokens = max_tokens
        
        # Load base model
        print(f"Loading model: {model_name}")
        self.tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=True)
        self.model = AutoModelForCausalLM.from_pretrained(
            model_name,
            torch_dtype=torch.float16,
            trust_remote_code=True,
        ).to(device)
        
        # Add Kore tokens to vocabulary
        # (In practice, we'd use a projection head instead)
        
        # Kore executor
        self.executor = IncrementalExecutor()
    
    def generate_with_execution(
        self,
        goal_stack: List[Any],
        temperature: float = 0.7,
    ) -> Tuple[List[str], ExecutionResult]:
        """
        Generate a Kore program with incremental execution.
        
        Returns: (tokens, final_result)
        """
        self.executor.reset()
        tokens = []
        
        # Build prompt
        prompt = f"Generate Kore program for stack: {goal_stack}\nProgram:"
        input_ids = self.tokenizer.encode(prompt, return_tensors="pt").to(self.device)
        
        for step in range(self.max_tokens):
            # Get LLM prediction
            with torch.no_grad():
                outputs = self.model(input_ids)
                logits = outputs.logits[:, -1, :]
            
            # Sample next token
            probs = F.softmax(logits / temperature, dim=-1)
            next_token_id = torch.multinomial(probs, num_samples=1).item()
            next_token = self.tokenizer.decode([next_token_id]).strip()
            
            # Map to Kore token (simplified)
            kore_token = self._map_to_kore_token(next_token)
            
            if kore_token == "<END>" or kore_token is None:
                break
            
            tokens.append(kore_token)
            
            # Execute immediately!
            result = self.executor.step(kore_token)
            
            if result.error:
                # Error during generation - could stop or continue
                break
            
            # Update prompt with execution state (for next iteration)
            state = self.executor.get_state()
            new_text = f" {kore_token} [stack:{state['stack']}]"
            new_ids = self.tokenizer.encode(new_text, add_special_tokens=False, return_tensors="pt").to(self.device)
            input_ids = torch.cat([input_ids, new_ids], dim=1)
            
            # Check if we've reached the goal
            if state['stack'] == goal_stack and not state['in_quote']:
                break
        
        # Get final execution result
        final_result = ExecutionResult(
            success=not self.executor.sim.error,
            final_stack=self.executor.sim.get_stack_python(),
            trace=self.executor.sim.trace,
            error=self.executor.sim.error,
            tokens_executed=len(tokens),
        )
        
        return tokens, final_result
    
    def _map_to_kore_token(self, llm_token: str) -> Optional[str]:
        """Map LLM output to Kore token."""
        llm_token = llm_token.strip().lower()
        
        # Direct match
        if llm_token in KORE_TOKEN_TO_ID:
            return llm_token
        
        # Number extraction
        import re
        nums = re.findall(r'\d+', llm_token)
        if nums:
            n = int(nums[0])
            if 0 <= n <= 10:
                return str(n)
        
        # End detection
        if any(end in llm_token for end in ['end', 'done', 'stop', '\n\n']):
            return "<END>"
        
        return None
    
    def train_step(
        self,
        sample: TrainingSample,
    ) -> Dict[str, Any]:
        """
        Single training step.
        
        1. Generate program with incremental execution
        2. Compute loss based on execution result
        3. Return loss and metrics
        """
        tokens, result = self.generate_with_execution(sample.goal_stack)
        
        loss, info = compute_execution_loss(
            result,
            sample.goal_stack,
            tokens,
        )
        
        info["generated"] = " ".join(tokens)
        info["target"] = sample.target_program
        
        return {"loss": loss, **info}
    
    def supervised_train_step(
        self,
        sample: TrainingSample,
        optimizer: torch.optim.Optimizer,
    ) -> Dict[str, Any]:
        """
        Supervised training: teach model to reproduce target program.
        
        Uses teacher forcing with execution verification.
        """
        self.executor.reset()
        
        # Tokenize target program
        target_tokens = sample.target_program.split()
        
        # Build training sequence
        prompt = f"Generate Kore program for stack: {sample.goal_stack}\nProgram:"
        
        losses = []
        
        for i, target_token in enumerate(target_tokens):
            # Get model prediction
            input_ids = self.tokenizer.encode(prompt, return_tensors="pt").to(self.device)
            
            self.model.train()
            outputs = self.model(input_ids)
            logits = outputs.logits[:, -1, :]
            
            # Compute loss against target token
            # (Simplified - in practice, need proper token mapping)
            target_text = f" {target_token}"
            target_ids = self.tokenizer.encode(target_text, add_special_tokens=False)
            
            if target_ids:
                target_id = target_ids[0]
                loss = F.cross_entropy(logits, torch.tensor([target_id]).to(self.device))
                losses.append(loss)
            
            # Execute the target token
            result = self.executor.step(target_token)
            state = self.executor.get_state()
            
            # Update prompt with actual execution
            prompt += f" {target_token}"
            if not state['in_quote']:
                prompt += f" [stack:{state['stack']}]"
        
        # Aggregate loss
        if losses:
            total_loss = torch.stack(losses).mean()
            
            optimizer.zero_grad()
            total_loss.backward()
            optimizer.step()
            
            return {
                "loss": total_loss.item(),
                "tokens": len(target_tokens),
                "success": not self.executor.sim.error,
                "final_stack": self.executor.sim.get_stack_python(),
            }
        
        return {"loss": 0.0, "tokens": 0}


# ============================================================================
# Simplified Training (No LLM, just test the pipeline)
# ============================================================================

def test_execution_pipeline():
    """Test the execution-during-generation pipeline."""
    
    print("=" * 60)
    print("Testing execution-during-generation pipeline")
    print("=" * 60)
    
    # Generate samples
    samples = generate_arithmetic_samples(10)
    
    executor = IncrementalExecutor()
    
    for sample in samples[:5]:
        print(f"\nGoal: {sample.goal_stack}")
        print(f"Target: {sample.target_program}")
        
        executor.reset()
        
        # Simulate LLM generating token by token
        tokens = sample.target_program.split()
        
        for token in tokens:
            result = executor.step(token)
            state = executor.get_state()
            
            print(f"  Token '{token}' → stack={state['stack']}, error={state['error']}")
            
            if result.error:
                break
        
        # Compute loss
        loss, info = compute_execution_loss(
            result,
            sample.goal_stack,
            tokens,
        )
        
        print(f"  Loss: {loss:.3f}, Correct: {info['correct']}")


def test_incremental_error_detection():
    """Test that errors are caught during generation."""
    
    print("\n" + "=" * 60)
    print("Testing incremental error detection")
    print("=" * 60)
    
    executor = IncrementalExecutor()
    
    # This program has an error at token 3
    tokens = ["3", "add", "5"]  # add needs 2 values, only 1 available
    
    print(f"Program: {' '.join(tokens)}")
    
    for i, token in enumerate(tokens):
        result = executor.step(token)
        state = executor.get_state()
        
        print(f"  Step {i}: '{token}' → stack={state['stack']}, error={state['error']}")
        
        if result.error:
            print(f"  *** Error detected at step {i}! ***")
            print(f"  *** In standard approach, we'd waste compute on tokens 3+ ***")
            break
    
    # Compute step rewards
    if executor.sim.trace:
        print("\nPer-step rewards:")
        for entry in executor.sim.trace:
            print(f"  {entry.op}: success={entry.success}, stack={entry.stack_after}")


def test_partial_credit():
    """Test partial credit for programs that make progress."""
    
    print("\n" + "=" * 60)
    print("Testing partial credit")
    print("=" * 60)
    
    goal = [14]
    
    programs = [
        ("3 4 add 2 mul", "Correct"),
        ("3 4 add", "Partial - got [7] instead of [14]"),
        ("3 4", "Partial - just pushed values"),
        ("3 add", "Error - stack underflow"),
    ]
    
    for program, desc in programs:
        sim = KoreSimulator()
        result = sim.execute(program)
        
        loss, info = compute_execution_loss(
            result,
            goal,
            program.split(),
        )
        
        print(f"\n{desc}")
        print(f"  Program: {program}")
        print(f"  Stack: {result.final_stack}")
        print(f"  Loss: {loss:.3f}")
        print(f"  Stack distance: {info['stack_distance']:.3f}")
        print(f"  Correct: {info['correct']}")


if __name__ == "__main__":
    test_execution_pipeline()
    test_incremental_error_detection()
    test_partial_credit()
