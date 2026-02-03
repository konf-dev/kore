#!/usr/bin/env python3
"""
Kore-RL Training with Sandboxed Docker Execution
=================================================

Main training script that:
1. Runs LLM to generate Kore programs
2. Executes in sandboxed Docker container
3. Computes rewards from execution results
4. Updates model with GRPO

Features:
- Auto-restart on container crash
- Resume from checkpoint
- Curriculum learning
- Comprehensive Kore documentation as system prompt
"""

import os
import sys
import json
import time
import argparse
import logging
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional, Dict, Any

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from accelerate import Accelerator
from tqdm import tqdm

# Add training directory to path
sys.path.insert(0, str(Path(__file__).parent))

from grpo import GRPOTrainer, GRPOConfig
from sandbox_executor import KoreSandboxedExecutor, TrainingCheckpoint
from prompts import (
    KORE_SYSTEM_PROMPT,
    create_chat_messages,
    generate_task,
    generate_batch,
)

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


@dataclass
class TrainingArgs:
    """Training arguments"""
    # Model
    model_name: str = "Qwen/Qwen2.5-Coder-7B-Instruct"
    
    # Execution
    use_sandbox: bool = True  # Use Docker sandbox
    exchange_dir: str = "/tmp/kore-exchange"
    kore_binary: str = "kore-train"  # For local execution
    
    # Training
    batch_size: int = 4
    num_samples: int = 8  # Samples per prompt for GRPO
    max_steps: int = 50000
    
    # GRPO
    learning_rate: float = 1e-6
    temperature: float = 0.7
    kl_coef: float = 0.1
    
    # Curriculum
    phase: int = 1  # Start phase (1-5)
    phase_steps: int = 10000  # Steps per phase
    
    # Checkpointing
    output_dir: str = "./checkpoints"
    save_steps: int = 1000
    resume: bool = True  # Resume from checkpoint
    
    # Hardware
    bf16: bool = True


def extract_program(text: str) -> str:
    """Extract Kore program from LLM output"""
    # Look for code block end
    if "```" in text:
        text = text.split("```")[0]
    
    # Clean up
    text = text.strip()
    
    # Take first non-empty lines until comment or explanation
    lines = []
    for line in text.split("\n"):
        line = line.strip()
        if not line:
            continue
        if line.startswith("#") and lines:
            break
        if any(word in line.lower() for word in ["explanation:", "note:", "result:"]):
            break
        lines.append(line)
    
    return " ".join(lines)


def compute_reward(result, target) -> float:
    """
    Compute reward from execution result.
    
    Rewards:
        +1.0: Perfect match
        +0.5-0.99: Close numeric answer
        -0.3: Wrong type
        -0.5: Wrong stack shape
        -1.0: Execution error
    """
    if not result.success:
        # Partial credit for execution progress
        return -1.0 + 0.1 * min(result.steps_executed / 20, 0.5)
    
    if len(result.final_stack) == 1:
        got = result.final_stack[0]
        
        if got == target:
            return 1.0
        
        # Numeric closeness
        if isinstance(got, (int, float)) and isinstance(target, (int, float)):
            diff = abs(got - target)
            return max(0.0, 0.5 / (1 + diff * 0.1))
        
        return -0.3
    
    elif isinstance(target, list) and result.final_stack == target:
        return 1.0
    
    return -0.5  # Wrong stack shape


class KoreRLTrainer:
    """
    Main Kore-RL training loop.
    
    Architecture:
        1. Generate tasks for current curriculum phase
        2. Create prompts with Kore documentation
        3. Sample K programs per prompt from LLM
        4. Execute all programs in Docker sandbox
        5. Compute rewards
        6. GRPO update
        7. Checkpoint periodically
    """
    
    def __init__(self, args: TrainingArgs):
        self.args = args
        self.accelerator = Accelerator(
            mixed_precision="bf16" if args.bf16 else None,
        )
        
        # Load model
        logger.info(f"Loading model: {args.model_name}")
        self.tokenizer = AutoTokenizer.from_pretrained(
            args.model_name,
            trust_remote_code=True,
        )
        if self.tokenizer.pad_token is None:
            self.tokenizer.pad_token = self.tokenizer.eos_token
        
        self.model = AutoModelForCausalLM.from_pretrained(
            args.model_name,
            trust_remote_code=True,
            torch_dtype=torch.bfloat16 if args.bf16 else torch.float32,
            device_map="auto",
        )
        
        # Reference model (frozen)
        self.ref_model = AutoModelForCausalLM.from_pretrained(
            args.model_name,
            trust_remote_code=True,
            torch_dtype=torch.bfloat16 if args.bf16 else torch.float32,
            device_map="auto",
        )
        for param in self.ref_model.parameters():
            param.requires_grad = False
        
        # GRPO trainer
        grpo_config = GRPOConfig(
            num_samples=args.num_samples,
            temperature=args.temperature,
            learning_rate=args.learning_rate,
            kl_coef=args.kl_coef,
        )
        self.grpo = GRPOTrainer(
            model=self.model,
            ref_model=self.ref_model,
            tokenizer=self.tokenizer,
            config=grpo_config,
        )
        
        # Setup executor
        if args.use_sandbox:
            logger.info(f"Using sandbox executor: {args.exchange_dir}")
            self.executor = KoreSandboxedExecutor(
                exchange_dir=args.exchange_dir,
                auto_restart=True,
            )
        else:
            from docker_executor import KoreLocalExecutor
            logger.info(f"Using local executor: {args.kore_binary}")
            self.executor = KoreLocalExecutor(
                kore_binary=args.kore_binary,
            )
        
        # Checkpoint manager
        self.checkpoint = TrainingCheckpoint(args.output_dir)
        
        # State
        self.global_step = 0
        self.current_phase = args.phase
        self.best_reward = float("-inf")
    
    def _maybe_resume(self):
        """Resume from checkpoint if available"""
        if not self.args.resume:
            return
        
        state = self.checkpoint.load()
        if state is None:
            logger.info("No checkpoint found, starting fresh")
            return
        
        self.global_step = state["step"]
        self.current_phase = state["phase"]
        self.best_reward = state["best_reward"]
        
        # Load model weights
        model_path = state.get("model_path")
        if model_path and Path(model_path).exists():
            logger.info(f"Loading model from {model_path}")
            self.model.load_state_dict(torch.load(model_path))
        
        logger.info(f"Resumed from step {self.global_step}, phase {self.current_phase}")
    
    def _save_checkpoint(self):
        """Save training checkpoint"""
        model_path = Path(self.args.output_dir) / f"model_step_{self.global_step}.pt"
        torch.save(self.model.state_dict(), model_path)
        
        self.checkpoint.save(
            step=self.global_step,
            phase=self.current_phase,
            best_reward=self.best_reward,
            model_path=str(model_path),
        )
        logger.info(f"Saved checkpoint at step {self.global_step}")
    
    def _generate_samples(self, prompts: List[str]) -> List[List[str]]:
        """Generate K samples per prompt"""
        all_samples = []
        
        for prompt in prompts:
            # Create chat messages
            messages = [
                {"role": "system", "content": KORE_SYSTEM_PROMPT},
                {"role": "user", "content": prompt},
            ]
            
            # Format for model
            if hasattr(self.tokenizer, "apply_chat_template"):
                text = self.tokenizer.apply_chat_template(
                    messages,
                    tokenize=False,
                    add_generation_prompt=True,
                )
            else:
                text = f"{KORE_SYSTEM_PROMPT}\n\nUser: {prompt}\n\nAssistant: "
            
            inputs = self.tokenizer(text, return_tensors="pt").to(self.model.device)
            
            # Generate K samples
            with torch.no_grad():
                outputs = self.model.generate(
                    **inputs,
                    max_new_tokens=100,
                    num_return_sequences=self.args.num_samples,
                    do_sample=True,
                    temperature=self.args.temperature,
                    pad_token_id=self.tokenizer.pad_token_id,
                )
            
            # Decode
            samples = []
            for output in outputs:
                text = self.tokenizer.decode(
                    output[inputs.input_ids.shape[1]:],
                    skip_special_tokens=True,
                )
                program = extract_program(text)
                samples.append(program)
            
            all_samples.append(samples)
        
        return all_samples
    
    def _execute_and_reward(
        self,
        programs: List[List[str]],
        targets: List[Any],
    ) -> torch.Tensor:
        """Execute programs and compute rewards"""
        batch_size = len(programs)
        num_samples = len(programs[0])
        rewards = torch.zeros(batch_size, num_samples)
        
        # Flatten for batch execution
        flat_programs = []
        for batch in programs:
            flat_programs.extend(batch)
        
        # Execute all
        results = self.executor.execute_batch(flat_programs, trace=False)
        
        # Compute rewards
        idx = 0
        for b in range(batch_size):
            target = targets[b]
            for k in range(num_samples):
                rewards[b, k] = compute_reward(results[idx], target)
                idx += 1
        
        return rewards
    
    def train(self):
        """Main training loop"""
        self._maybe_resume()
        
        logger.info(f"Starting training from step {self.global_step}")
        logger.info(f"Phase {self.current_phase}, target steps: {self.args.max_steps}")
        
        os.makedirs(self.args.output_dir, exist_ok=True)
        
        progress = tqdm(
            total=self.args.max_steps,
            initial=self.global_step,
            desc="Training",
        )
        
        while self.global_step < self.args.max_steps:
            # Check phase progression
            phase_step = self.global_step % self.args.phase_steps
            if phase_step == 0 and self.global_step > 0:
                if self.current_phase < 5:
                    self.current_phase += 1
                    logger.info(f"Advancing to phase {self.current_phase}")
            
            # Generate tasks
            tasks = generate_batch(self.current_phase, self.args.batch_size)
            
            # Create prompts
            prompts = []
            targets = []
            for task in tasks:
                prompt = f"""## Task
{task['description']}

### Expected Result
The final stack should contain exactly: {task['expected_str']}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""
                prompts.append(prompt)
                targets.append(task['expected'])
            
            # Generate samples
            try:
                samples = self._generate_samples(prompts)
            except Exception as e:
                logger.error(f"Generation error: {e}")
                continue
            
            # Execute and reward
            try:
                rewards = self._execute_and_reward(samples, targets)
            except Exception as e:
                logger.error(f"Execution error: {e}")
                # Check if container crashed
                if hasattr(self.executor, 'health_check'):
                    if not self.executor.health_check():
                        logger.warning("Executor unhealthy, restarting...")
                        time.sleep(1)
                continue
            
            # GRPO update
            try:
                metrics = self.grpo.step(prompts, samples, rewards)
            except Exception as e:
                logger.error(f"GRPO error: {e}")
                continue
            
            # Track best reward
            mean_reward = rewards.mean().item()
            if mean_reward > self.best_reward:
                self.best_reward = mean_reward
            
            # Update progress
            self.global_step += 1
            progress.update(1)
            progress.set_postfix({
                "loss": f"{metrics.get('loss', 0):.4f}",
                "reward": f"{mean_reward:.3f}",
                "phase": self.current_phase,
            })
            
            # Logging
            if self.global_step % 10 == 0:
                success_rate = (rewards > 0.9).float().mean().item()
                logger.info(
                    f"Step {self.global_step}: "
                    f"loss={metrics.get('loss', 0):.4f}, "
                    f"reward={mean_reward:.3f}, "
                    f"success={success_rate:.1%}"
                )
                
                # Log a sample
                best_b = rewards.mean(dim=1).argmax().item()
                best_k = rewards[best_b].argmax().item()
                logger.info(f"  Best: {samples[best_b][best_k]!r} -> r={rewards[best_b, best_k]:.2f}")
            
            # Checkpoint
            if self.global_step % self.args.save_steps == 0:
                self._save_checkpoint()
        
        progress.close()
        self._save_checkpoint()
        logger.info("Training complete!")
    
    def close(self):
        """Cleanup"""
        if hasattr(self.executor, 'close'):
            self.executor.close()


def main():
    parser = argparse.ArgumentParser(description="Kore-RL Training")
    
    parser.add_argument("--model", type=str, default="Qwen/Qwen2.5-Coder-7B-Instruct")
    parser.add_argument("--use-sandbox", action="store_true", default=True)
    parser.add_argument("--exchange-dir", type=str, default="/tmp/kore-exchange")
    parser.add_argument("--kore-binary", type=str, default="kore-train")
    parser.add_argument("--batch-size", type=int, default=4)
    parser.add_argument("--num-samples", type=int, default=8)
    parser.add_argument("--max-steps", type=int, default=50000)
    parser.add_argument("--learning-rate", type=float, default=1e-6)
    parser.add_argument("--phase", type=int, default=1)
    parser.add_argument("--output-dir", type=str, default="./checkpoints")
    parser.add_argument("--resume", action="store_true", default=True)
    parser.add_argument("--no-resume", action="store_false", dest="resume")
    
    args = parser.parse_args()
    
    training_args = TrainingArgs(
        model_name=args.model,
        use_sandbox=args.use_sandbox,
        exchange_dir=args.exchange_dir,
        kore_binary=args.kore_binary,
        batch_size=args.batch_size,
        num_samples=args.num_samples,
        max_steps=args.max_steps,
        learning_rate=args.learning_rate,
        phase=args.phase,
        output_dir=args.output_dir,
        resume=args.resume,
    )
    
    trainer = KoreRLTrainer(training_args)
    
    try:
        trainer.train()
    finally:
        trainer.close()


if __name__ == "__main__":
    main()
