"""
Main Training Loop for Kore-RL
==============================

Train LLMs to generate Kore programs using GRPO.
Uses local kore-train binary or Docker executor for fast execution.
"""

import os
import json
import time
import logging
from pathlib import Path
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any, Union
import random

import torch
from torch.utils.data import DataLoader, Dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    get_linear_schedule_with_warmup,
)
from accelerate import Accelerator
from tqdm import tqdm

from grpo import GRPOTrainer, GRPOConfig, compute_kore_rewards
from docker_executor import KoreLocalExecutor, KoreDockerExecutor, ExecuteResult

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


@dataclass
class TrainingConfig:
    """Training configuration"""
    # Model
    model_name: str = "Qwen/Qwen2.5-Coder-7B-Instruct"
    model_revision: str = "main"
    trust_remote_code: bool = True
    
    # Runtime
    kore_binary: str = "kore-train"  # Path to kore-train binary
    use_docker: bool = False  # Use docker executor
    container_name: str = "kore-runtime"  # For docker executor
    max_workers: int = 32  # Parallel execution workers
    
    # Training
    batch_size: int = 4
    gradient_accumulation_steps: int = 4
    num_epochs: int = 10
    max_steps: int = 50000
    
    # GRPO
    num_samples: int = 8
    temperature: float = 0.7
    learning_rate: float = 1e-6
    kl_coef: float = 0.1
    
    # Curriculum
    curriculum_phase: int = 1  # 1=arithmetic, 2=stack, 3=control, etc.
    
    # Checkpointing
    output_dir: str = "./checkpoints"
    save_steps: int = 1000
    eval_steps: int = 500
    logging_steps: int = 10
    
    # Hardware
    bf16: bool = True
    gradient_checkpointing: bool = True
    
    @classmethod
    def from_yaml(cls, path: str) -> "TrainingConfig":
        import yaml
        with open(path) as f:
            data = yaml.safe_load(f)
        return cls(**data)


class KoreTaskDataset(Dataset):
    """Dataset of Kore programming tasks"""
    
    def __init__(self, tasks: List[Dict[str, Any]]):
        self.tasks = tasks
    
    def __len__(self):
        return len(self.tasks)
    
    def __getitem__(self, idx):
        return self.tasks[idx]


def generate_arithmetic_tasks(n: int) -> List[Dict[str, Any]]:
    """Generate Phase 1 arithmetic tasks"""
    tasks = []
    ops = ["add", "sub", "mul"]
    
    for _ in range(n):
        a = random.randint(1, 100)
        b = random.randint(1, 100)
        op = random.choice(ops)
        
        if op == "add":
            result = a + b
        elif op == "sub":
            result = a - b
        else:
            result = a * b
        
        tasks.append({
            "prompt": f"Write a Kore program that computes {a} {op} {b}.\n\n```kore\n",
            "target": result,
            "difficulty": 1,
            "type": "arithmetic",
        })
    
    # Two-operation expressions
    for _ in range(n // 2):
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        c = random.randint(1, 50)
        
        # (a + b) * c
        result = (a + b) * c
        tasks.append({
            "prompt": f"Write a Kore program that computes ({a} + {b}) * {c}.\n\n```kore\n",
            "target": result,
            "difficulty": 2,
            "type": "arithmetic",
        })
    
    return tasks


def generate_stack_tasks(n: int) -> List[Dict[str, Any]]:
    """Generate Phase 2 stack manipulation tasks"""
    tasks = []
    
    # Duplicate and add
    for _ in range(n // 3):
        a = random.randint(1, 50)
        tasks.append({
            "prompt": f"Write a Kore program that duplicates {a} and adds the copies (result: {2*a}).\n\n```kore\n",
            "target": 2 * a,
            "difficulty": 2,
            "type": "stack",
        })
    
    # Swap and subtract
    for _ in range(n // 3):
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        tasks.append({
            "prompt": f"Write a Kore program that swaps {a} and {b}, then subtracts (result: {b-a}).\n\n```kore\n",
            "target": b - a,
            "difficulty": 2,
            "type": "stack",
        })
    
    # Over and add
    for _ in range(n // 3):
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        tasks.append({
            "prompt": f"Write a Kore program with {a} and {b} on stack, use 'over' to copy {a}, add top two (result: {a+b} with {a} below).\n\n```kore\n",
            "target": [a, a + b],  # Stack has a and (a+b)
            "difficulty": 3,
            "type": "stack",
        })
    
    return tasks


def generate_control_tasks(n: int) -> List[Dict[str, Any]]:
    """Generate Phase 3 control flow tasks"""
    tasks = []
    
    # Max of two numbers
    for _ in range(n // 3):
        a = random.randint(1, 100)
        b = random.randint(1, 100)
        tasks.append({
            "prompt": f"Write a Kore program that computes max({a}, {b}) using 'if'.\n\n```kore\n",
            "target": max(a, b),
            "difficulty": 3,
            "type": "control",
        })
    
    # Absolute value
    for _ in range(n // 3):
        a = random.randint(-50, 50)
        tasks.append({
            "prompt": f"Write a Kore program that computes abs({a}) using 'if'.\n\n```kore\n",
            "target": abs(a),
            "difficulty": 3,
            "type": "control",
        })
    
    # Sign function
    for _ in range(n // 3):
        a = random.randint(-50, 50)
        sign = 1 if a > 0 else (-1 if a < 0 else 0)
        tasks.append({
            "prompt": f"Write a Kore program that returns the sign of {a} (-1, 0, or 1).\n\n```kore\n",
            "target": sign,
            "difficulty": 4,
            "type": "control",
        })
    
    return tasks


def generate_loop_tasks(n: int) -> List[Dict[str, Any]]:
    """Generate Phase 4 loop tasks"""
    tasks = []
    
    # Sum 1 to n
    for _ in range(n // 3):
        k = random.randint(5, 20)
        tasks.append({
            "prompt": f"Write a Kore program that computes sum(1..{k}) = {k*(k+1)//2} using 'times'.\n\n```kore\n",
            "target": k * (k + 1) // 2,
            "difficulty": 4,
            "type": "loop",
        })
    
    # Factorial
    for _ in range(n // 3):
        k = random.randint(3, 8)
        fact = 1
        for i in range(1, k + 1):
            fact *= i
        tasks.append({
            "prompt": f"Write a Kore program that computes {k}! = {fact}.\n\n```kore\n",
            "target": fact,
            "difficulty": 5,
            "type": "loop",
        })
    
    # Fibonacci
    for _ in range(n // 3):
        k = random.randint(5, 15)
        fibs = [0, 1]
        for _ in range(k - 1):
            fibs.append(fibs[-1] + fibs[-2])
        tasks.append({
            "prompt": f"Write a Kore program that computes fibonacci({k}) = {fibs[k]}.\n\n```kore\n",
            "target": fibs[k],
            "difficulty": 6,
            "type": "loop",
        })
    
    return tasks


def get_curriculum_tasks(phase: int, n: int = 1000) -> List[Dict[str, Any]]:
    """Get tasks for a curriculum phase"""
    if phase == 1:
        return generate_arithmetic_tasks(n)
    elif phase == 2:
        return generate_arithmetic_tasks(n // 2) + generate_stack_tasks(n // 2)
    elif phase == 3:
        return (
            generate_arithmetic_tasks(n // 3) +
            generate_stack_tasks(n // 3) +
            generate_control_tasks(n // 3)
        )
    elif phase >= 4:
        return (
            generate_arithmetic_tasks(n // 4) +
            generate_stack_tasks(n // 4) +
            generate_control_tasks(n // 4) +
            generate_loop_tasks(n // 4)
        )
    else:
        return generate_arithmetic_tasks(n)


def extract_program(text: str) -> str:
    """Extract Kore program from generated text"""
    # Look for code block end
    if "```" in text:
        text = text.split("```")[0]
    
    # Clean up
    text = text.strip()
    
    # Remove any trailing explanation
    lines = text.split("\n")
    program_lines = []
    for line in lines:
        # Stop at explanation
        if line.strip().startswith("#") and len(program_lines) > 0:
            break
        if line.strip():
            program_lines.append(line.strip())
    
    return " ".join(program_lines)


class Trainer:
    """Main training loop"""
    
    def __init__(self, config: TrainingConfig):
        self.config = config
        self.accelerator = Accelerator(
            gradient_accumulation_steps=config.gradient_accumulation_steps,
            mixed_precision="bf16" if config.bf16 else None,
        )
        
        logger.info(f"Loading model: {config.model_name}")
        
        # Load model and tokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(
            config.model_name,
            trust_remote_code=config.trust_remote_code,
        )
        if self.tokenizer.pad_token is None:
            self.tokenizer.pad_token = self.tokenizer.eos_token
        
        self.model = AutoModelForCausalLM.from_pretrained(
            config.model_name,
            trust_remote_code=config.trust_remote_code,
            torch_dtype=torch.bfloat16 if config.bf16 else torch.float32,
            device_map="auto",
        )
        
        if config.gradient_checkpointing:
            self.model.gradient_checkpointing_enable()
        
        # Reference model (frozen copy)
        self.ref_model = AutoModelForCausalLM.from_pretrained(
            config.model_name,
            trust_remote_code=config.trust_remote_code,
            torch_dtype=torch.bfloat16 if config.bf16 else torch.float32,
            device_map="auto",
        )
        
        # GRPO trainer
        grpo_config = GRPOConfig(
            num_samples=config.num_samples,
            temperature=config.temperature,
            learning_rate=config.learning_rate,
            kl_coef=config.kl_coef,
        )
        
        self.grpo = GRPOTrainer(
            model=self.model,
            ref_model=self.ref_model,
            tokenizer=self.tokenizer,
            config=grpo_config,
        )
        
        # Runtime executor (local binary or docker)
        if config.use_docker:
            logger.info(f"Using Docker executor: {config.container_name}")
            self.executor = KoreDockerExecutor(
                container_name=config.container_name,
                max_workers=config.max_workers,
            )
        else:
            logger.info(f"Using local executor: {config.kore_binary}")
            self.executor = KoreLocalExecutor(
                kore_binary=config.kore_binary,
                max_workers=config.max_workers,
            )
        
        # Check runtime health
        if not self.executor.health_check():
            logger.warning("Kore executor not healthy!")
        
        # Metrics
        self.global_step = 0
        self.best_reward = float("-inf")
    
    def train(self):
        """Main training loop"""
        config = self.config
        
        # Create output directory
        os.makedirs(config.output_dir, exist_ok=True)
        
        # Get curriculum tasks
        tasks = get_curriculum_tasks(config.curriculum_phase)
        dataset = KoreTaskDataset(tasks)
        dataloader = DataLoader(
            dataset,
            batch_size=config.batch_size,
            shuffle=True,
        )
        
        logger.info(f"Training on {len(tasks)} tasks (Phase {config.curriculum_phase})")
        logger.info(f"Batch size: {config.batch_size} x {config.gradient_accumulation_steps}")
        logger.info(f"Samples per prompt: {config.num_samples}")
        
        # Training loop
        for epoch in range(config.num_epochs):
            logger.info(f"Epoch {epoch + 1}/{config.num_epochs}")
            
            epoch_metrics = {
                "loss": [],
                "reward": [],
                "success_rate": [],
            }
            
            progress = tqdm(dataloader, desc=f"Epoch {epoch + 1}")
            
            for batch in progress:
                if self.global_step >= config.max_steps:
                    logger.info("Reached max steps")
                    break
                
                # Get prompts and targets
                prompts = batch["prompt"]
                targets = batch["target"]
                
                # Generate samples
                samples, _, _ = self.grpo.generate_samples(prompts)
                
                # Extract programs from generated text
                programs = [
                    [extract_program(s) for s in sample_group]
                    for sample_group in samples
                ]
                
                # Execute in Kore runtime and compute rewards
                rewards = self._compute_batch_rewards(programs, targets)
                
                # GRPO update
                metrics = self.grpo.step(prompts, programs, rewards)
                
                # Track metrics
                epoch_metrics["loss"].append(metrics["loss"])
                epoch_metrics["reward"].append(metrics["reward_mean"])
                
                success_rate = (rewards > 0.5).float().mean().item()
                epoch_metrics["success_rate"].append(success_rate)
                
                # Update progress bar
                progress.set_postfix({
                    "loss": f"{metrics['loss']:.4f}",
                    "reward": f"{metrics['reward_mean']:.3f}",
                    "success": f"{success_rate:.2%}",
                })
                
                self.global_step += 1
                
                # Logging
                if self.global_step % config.logging_steps == 0:
                    self._log_metrics(metrics, programs, targets, rewards)
                
                # Evaluation
                if self.global_step % config.eval_steps == 0:
                    self._evaluate()
                
                # Checkpointing
                if self.global_step % config.save_steps == 0:
                    self._save_checkpoint()
            
            # Epoch summary
            avg_loss = sum(epoch_metrics["loss"]) / len(epoch_metrics["loss"])
            avg_reward = sum(epoch_metrics["reward"]) / len(epoch_metrics["reward"])
            avg_success = sum(epoch_metrics["success_rate"]) / len(epoch_metrics["success_rate"])
            
            logger.info(
                f"Epoch {epoch + 1} - Loss: {avg_loss:.4f}, "
                f"Reward: {avg_reward:.3f}, Success: {avg_success:.2%}"
            )
        
        # Final checkpoint
        self._save_checkpoint()
        logger.info("Training complete!")
    
    def _compute_batch_rewards(
        self,
        programs: List[List[str]],
        targets: List[Any],
    ) -> torch.Tensor:
        """Execute programs in parallel and compute rewards"""
        batch_size = len(programs)
        num_samples = len(programs[0])
        rewards = torch.zeros(batch_size, num_samples)
        
        # Flatten programs for batch execution
        flat_programs = []
        flat_targets = []
        for b in range(batch_size):
            target = targets[b]
            if isinstance(target, torch.Tensor):
                target = target.item()
            for k in range(num_samples):
                flat_programs.append(programs[b][k])
                flat_targets.append(target)
        
        # Execute all programs in parallel
        results = self.executor.execute_batch(flat_programs, trace=False)
        
        # Compute rewards for each result
        idx = 0
        for b in range(batch_size):
            for k in range(num_samples):
                result = results[idx]
                target = flat_targets[idx]
                
                try:
                    if not result.success:
                        # Partial credit for execution progress
                        rewards[b, k] = -1.0 + 0.1 * min(result.steps_executed / 20, 0.5)
                    elif len(result.final_stack) == 1 and result.final_stack[0] == target:
                        # Perfect!
                        rewards[b, k] = 1.0
                    elif len(result.final_stack) == 1:
                        # Close answer
                        got = result.final_stack[0]
                        if isinstance(got, (int, float)) and isinstance(target, (int, float)):
                            diff = abs(got - target)
                            rewards[b, k] = 0.5 / (1 + diff * 0.1)
                        else:
                            rewards[b, k] = -0.3
                    else:
                        # Wrong stack shape
                        rewards[b, k] = -0.5
                except Exception as e:
                    logger.warning(f"Reward computation error: {e}")
                    rewards[b, k] = -1.0
                
                idx += 1
        
        return rewards
    
    def _log_metrics(
        self,
        metrics: dict,
        programs: List[List[str]],
        targets: List[Any],
        rewards: torch.Tensor,
    ):
        """Log training metrics"""
        # Find best program in batch
        best_idx = rewards.argmax()
        b, k = best_idx // rewards.shape[1], best_idx % rewards.shape[1]
        best_program = programs[b.item()][k.item()]
        best_target = targets[b.item()]
        best_reward = rewards[b, k].item()
        
        logger.info(
            f"Step {self.global_step}: "
            f"loss={metrics['loss']:.4f}, "
            f"reward={metrics['reward_mean']:.3f}, "
            f"kl={metrics['kl_loss']:.4f}"
        )
        logger.info(f"  Best: '{best_program}' → target={best_target}, reward={best_reward:.3f}")
    
    def _evaluate(self):
        """Run evaluation"""
        # Generate a few test cases
        test_tasks = get_curriculum_tasks(self.config.curriculum_phase, n=50)
        
        correct = 0
        total = 0
        
        self.model.eval()
        with torch.no_grad():
            for task in test_tasks[:20]:  # Quick eval
                prompt = task["prompt"]
                target = task["target"]
                
                # Generate single sample (greedy)
                inputs = self.tokenizer(prompt, return_tensors="pt").to(self.model.device)
                outputs = self.model.generate(
                    **inputs,
                    max_new_tokens=64,
                    temperature=0.1,
                    do_sample=True,
                    pad_token_id=self.tokenizer.pad_token_id,
                )
                
                generated = self.tokenizer.decode(outputs[0], skip_special_tokens=True)
                program = extract_program(generated[len(prompt):])
                
                result = self.runtime.execute(program)
                
                if result.success and len(result.final_stack) == 1:
                    if result.final_stack[0] == target:
                        correct += 1
                
                total += 1
        
        accuracy = correct / total if total > 0 else 0
        logger.info(f"Eval accuracy: {correct}/{total} = {accuracy:.2%}")
        
        if accuracy > self.best_reward:
            self.best_reward = accuracy
            self._save_checkpoint(suffix="_best")
    
    def _save_checkpoint(self, suffix: str = ""):
        """Save model checkpoint"""
        path = os.path.join(
            self.config.output_dir,
            f"step_{self.global_step}{suffix}"
        )
        os.makedirs(path, exist_ok=True)
        
        self.model.save_pretrained(path)
        self.tokenizer.save_pretrained(path)
        
        # Save training state
        state = {
            "global_step": self.global_step,
            "best_reward": self.best_reward,
            "config": vars(self.config),
        }
        with open(os.path.join(path, "training_state.json"), "w") as f:
            json.dump(state, f, indent=2)
        
        logger.info(f"Saved checkpoint to {path}")


def main():
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", type=str, default=None)
    parser.add_argument("--model", type=str, default="Qwen/Qwen2.5-Coder-7B-Instruct")
    parser.add_argument("--runtime-url", type=str, default="http://localhost:8080")
    parser.add_argument("--output-dir", type=str, default="./checkpoints")
    parser.add_argument("--phase", type=int, default=1)
    parser.add_argument("--batch-size", type=int, default=4)
    parser.add_argument("--num-samples", type=int, default=8)
    parser.add_argument("--max-steps", type=int, default=10000)
    args = parser.parse_args()
    
    if args.config:
        config = TrainingConfig.from_yaml(args.config)
    else:
        config = TrainingConfig(
            model_name=args.model,
            runtime_urls=[args.runtime_url],
            output_dir=args.output_dir,
            curriculum_phase=args.phase,
            batch_size=args.batch_size,
            num_samples=args.num_samples,
            max_steps=args.max_steps,
        )
    
    trainer = Trainer(config)
    trainer.train()


if __name__ == "__main__":
    main()
