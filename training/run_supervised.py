#!/usr/bin/env python3
"""
Supervised Kore Training Experiment

This script trains an LLM to write Kore programs using a progressive curriculum.
The LLM learns through GRPO (Group Relative Policy Optimization) with verified rewards.

Usage:
    python run_supervised.py --config config.yaml
    python run_supervised.py --model "Qwen/Qwen2.5-Coder-3B-Instruct" --phases 1,2
"""

import argparse
import json
import logging
import os
import random
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple

import yaml
import torch
import numpy as np
from tqdm import tqdm

from kore_runtime import (
    KoreRuntime, 
    ExecutionResult, 
    EffectSignature,
    SYSTEM_PROMPT,
    make_task_prompt,
    make_feedback_prompt,
)
from model_interface import load_model, GenerationConfig
from task_generators import generate_task, generate_task_batch, Task


# =============================================================================
# Logging Setup
# =============================================================================

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler('training.log'),
    ]
)
logger = logging.getLogger(__name__)


# =============================================================================
# GRPO Training Loop
# =============================================================================

@dataclass
class TrainingStats:
    """Track training statistics."""
    phase: int = 1
    epoch: int = 0
    step: int = 0
    total_tasks: int = 0
    correct_tasks: int = 0
    cumulative_reward: float = 0.0
    rewards_history: List[float] = field(default_factory=list)
    
    @property
    def accuracy(self) -> float:
        return self.correct_tasks / max(1, self.total_tasks)
    
    @property
    def avg_reward(self) -> float:
        return self.cumulative_reward / max(1, self.total_tasks)


def compute_reward(
    task: Task,
    program: str,
    runtime: KoreRuntime,
    trace: bool = True,
) -> Tuple[float, str, Optional[ExecutionResult]]:
    """
    Compute reward for a generated program.
    
    Returns:
        (reward, feedback_message, execution_result)
    
    Reward breakdown:
        - Correct solution: 1.0
        - Correct effect but wrong output: 0.3
        - Parses but wrong effect: 0.1
        - Syntax error: 0.0
    """
    
    # Try to get effect
    effect = runtime.get_effect(program)
    
    if effect is None:
        return 0.0, "Program has syntax errors", None
    
    # Check if effect matches
    effect_matches = effect.matches(task.expected_effect)
    
    if not effect_matches:
        distance = effect.distance(task.expected_effect)
        partial = max(0.0, 0.1 - distance * 0.01)
        return partial, f"Wrong effect: got {effect.to_string()}, expected {task.expected_effect.to_string()}", None
    
    # Run test cases
    all_correct = True
    first_failure = None
    result = None
    
    for inputs, expected in task.test_cases:
        result = runtime.execute(program, inputs.copy(), trace=trace)
        
        if not result.success:
            all_correct = False
            first_failure = f"Error on input {inputs}: {result.error}"
            break
        
        if not result.stack:
            all_correct = False
            first_failure = f"Empty stack on input {inputs}"
            break
        
        actual = result.stack[-1] if len(result.stack) == 1 else result.stack
        
        # Check output
        if isinstance(expected, float) and isinstance(actual, (int, float)):
            if abs(actual - expected) > 1e-6:
                all_correct = False
                first_failure = f"Wrong output on {inputs}: got {actual}, expected {expected}"
                break
        elif isinstance(expected, list) and isinstance(actual, list):
            if actual != expected:
                all_correct = False
                first_failure = f"Wrong output on {inputs}: got {actual}, expected {expected}"
                break
        elif actual != expected:
            all_correct = False
            first_failure = f"Wrong output on {inputs}: got {actual}, expected {expected}"
            break
    
    if all_correct:
        return 1.0, "Correct! All test cases passed.", result
    else:
        return 0.3, f"Effect is correct but: {first_failure}", result


def grpo_update(
    model,
    programs: List[str],
    rewards: List[float],
    prompts: List[str],
    config: Dict[str, Any],
) -> float:
    """
    Apply GRPO update.
    
    GRPO (Group Relative Policy Optimization):
    - Sample group of programs for each task
    - Reward = (individual_reward - group_mean) / group_std
    - This removes reward scale sensitivity
    """
    
    if not hasattr(model, 'train_step'):
        # For inference-only models, skip training
        logger.warning("Model does not support training, skipping update")
        return 0.0
    
    # Normalize rewards within group
    rewards_array = np.array(rewards)
    mean_reward = np.mean(rewards_array)
    std_reward = np.std(rewards_array) + 1e-8
    
    normalized_rewards = (rewards_array - mean_reward) / std_reward
    
    # Apply update
    loss = model.train_step(prompts, programs, normalized_rewards.tolist())
    
    return loss


def run_training_step(
    model,
    runtime: KoreRuntime,
    task: Task,
    config: Dict[str, Any],
    max_attempts: int = 3,
) -> Tuple[float, bool, str, List[str]]:
    """
    Run one training step on a single task.
    
    Returns:
        (final_reward, solved, best_program, all_attempts)
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.7),
        top_p=config.get("top_p", 0.95),
        stop_sequences=config.get("stop_sequences", ["```", "\n\n"]),
    )
    
    # Create prompt
    example = task.test_cases[0]
    prompt = make_task_prompt(
        task_description=task.description,
        expected_effect=task.expected_effect.to_string(),
        example_input=example[0],
        example_output=example[1],
    )
    
    best_reward = 0.0
    best_program = ""
    all_attempts = []
    
    current_prompt = prompt
    
    for attempt in range(max_attempts):
        # Generate program
        program = model.generate(SYSTEM_PROMPT, current_prompt, gen_config)
        
        # Extract code if wrapped in backticks
        if "```" in program:
            lines = program.split("```")
            for i, block in enumerate(lines):
                if i % 2 == 1:  # Code block
                    program = block.strip()
                    if program.startswith("kore"):
                        program = program[4:].strip()
                    break
        
        program = program.strip()
        all_attempts.append(program)
        
        # Compute reward
        reward, feedback, result = compute_reward(task, program, runtime, trace=True)
        
        if reward > best_reward:
            best_reward = reward
            best_program = program
        
        if reward >= 1.0:
            # Solved!
            break
        
        # Create feedback prompt for next attempt
        trace_str = ""
        if result and result.trace:
            trace_str = "\n".join(result.trace[-10:])  # Last 10 trace lines
        
        current_prompt = make_feedback_prompt(
            original_task=prompt,
            previous_attempt=program,
            error_message=feedback,
            execution_trace=trace_str if trace_str else None,
        )
    
    return best_reward, best_reward >= 1.0, best_program, all_attempts


# =============================================================================
# Main Training Loop
# =============================================================================

def train_phase(
    model,
    runtime: KoreRuntime,
    phase: int,
    config: Dict[str, Any],
    stats: TrainingStats,
) -> TrainingStats:
    """Train on one curriculum phase."""
    
    logger.info(f"\n{'='*60}")
    logger.info(f"Starting Phase {phase}")
    logger.info(f"{'='*60}")
    
    phase_config = config.get("supervised", {}).get(f"phase_{phase}", {})
    n_tasks = phase_config.get("n_tasks", config.get("training", {}).get("batch_size", 8) * 100)
    epochs = config.get("training", {}).get("epochs", 3)
    batch_size = config.get("training", {}).get("batch_size", 8)
    
    stats.phase = phase
    
    for epoch in range(epochs):
        stats.epoch = epoch
        
        # Generate tasks for this epoch
        tasks = generate_task_batch(phase, n_tasks)
        
        # Shuffle
        random.shuffle(tasks)
        
        # Process in batches
        n_batches = (len(tasks) + batch_size - 1) // batch_size
        
        pbar = tqdm(range(n_batches), desc=f"Phase {phase}, Epoch {epoch+1}/{epochs}")
        
        for batch_idx in pbar:
            batch_start = batch_idx * batch_size
            batch_end = min(batch_start + batch_size, len(tasks))
            batch_tasks = tasks[batch_start:batch_end]
            
            batch_rewards = []
            batch_solved = 0
            
            for task in batch_tasks:
                reward, solved, best_program, attempts = run_training_step(
                    model, runtime, task, config.get("training", {})
                )
                
                batch_rewards.append(reward)
                stats.total_tasks += 1
                stats.cumulative_reward += reward
                stats.rewards_history.append(reward)
                
                if solved:
                    batch_solved += 1
                    stats.correct_tasks += 1
                
                # Log occasional examples
                if random.random() < 0.01:  # 1% of examples
                    logger.info(f"\n--- Example ---")
                    logger.info(f"Task: {task.description}")
                    logger.info(f"Best program: {best_program}")
                    logger.info(f"Reward: {reward:.2f}")
            
            # Update progress bar
            pbar.set_postfix({
                "batch_acc": f"{batch_solved/len(batch_tasks):.2%}",
                "total_acc": f"{stats.accuracy:.2%}",
                "avg_reward": f"{stats.avg_reward:.3f}",
            })
            
            stats.step += 1
        
        # End of epoch logging
        logger.info(f"\nEpoch {epoch+1} complete:")
        logger.info(f"  Accuracy: {stats.accuracy:.2%}")
        logger.info(f"  Avg Reward: {stats.avg_reward:.3f}")
        
        # Save checkpoint
        checkpoint_dir = Path(config.get("logging", {}).get("checkpoint_dir", "checkpoints"))
        checkpoint_dir.mkdir(parents=True, exist_ok=True)
        
        checkpoint_path = checkpoint_dir / f"phase{phase}_epoch{epoch+1}.json"
        with open(checkpoint_path, 'w') as f:
            json.dump({
                "phase": phase,
                "epoch": epoch + 1,
                "total_tasks": stats.total_tasks,
                "correct_tasks": stats.correct_tasks,
                "accuracy": stats.accuracy,
                "avg_reward": stats.avg_reward,
            }, f, indent=2)
        
        logger.info(f"  Saved checkpoint to {checkpoint_path}")
    
    return stats


def run_supervised_training(config: Dict[str, Any], args: argparse.Namespace):
    """Main training entry point."""
    
    logger.info("="*60)
    logger.info("Kore Supervised Training")
    logger.info("="*60)
    logger.info(f"Config: {json.dumps(config, indent=2, default=str)}")
    
    # Initialize runtime
    kore_binary = config.get("runtime", {}).get("binary", "target/release/kore-train")
    runtime = KoreRuntime(kore_binary=kore_binary)
    
    # Check runtime works
    test_result = runtime.execute("1 2 add")
    if not test_result.success or test_result.stack != [3]:
        logger.error(f"Kore runtime test failed: {test_result}")
        logger.error("Make sure to run: cargo build --release --bin kore-train")
        sys.exit(1)
    
    logger.info("✓ Kore runtime verified")
    
    # Load model
    model_name = args.model or config.get("model", {}).get("name", "Qwen/Qwen2.5-Coder-7B-Instruct")
    quantization = args.quantization or config.get("model", {}).get("quantization")
    
    logger.info(f"Loading model: {model_name}")
    if quantization:
        logger.info(f"Using quantization: {quantization}")
    
    model = load_model(
        model_name=model_name,
        quantization=quantization,
        device="cuda" if torch.cuda.is_available() else "cpu",
    )
    
    logger.info("✓ Model loaded")
    
    # Determine phases to train
    if args.phases:
        phases = [int(p) for p in args.phases.split(",")]
    else:
        phases = list(range(1, 7))  # All phases
    
    logger.info(f"Training phases: {phases}")
    
    # Initialize stats
    stats = TrainingStats()
    
    # Train each phase
    start_time = time.time()
    
    for phase in phases:
        stats = train_phase(model, runtime, phase, config, stats)
        
        # Phase completion criteria
        if stats.accuracy >= 0.9:
            logger.info(f"✓ Phase {phase} mastered (accuracy >= 90%)")
        else:
            logger.warning(f"Phase {phase} not fully mastered (accuracy = {stats.accuracy:.2%})")
    
    # Final summary
    elapsed = time.time() - start_time
    
    logger.info("\n" + "="*60)
    logger.info("TRAINING COMPLETE")
    logger.info("="*60)
    logger.info(f"Total time: {elapsed/3600:.1f} hours")
    logger.info(f"Total tasks: {stats.total_tasks}")
    logger.info(f"Final accuracy: {stats.accuracy:.2%}")
    logger.info(f"Average reward: {stats.avg_reward:.3f}")
    
    # Save final results
    results_dir = Path(config.get("logging", {}).get("log_dir", "results"))
    results_dir.mkdir(parents=True, exist_ok=True)
    
    results_path = results_dir / f"supervised_results_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    with open(results_path, 'w') as f:
        json.dump({
            "config": config,
            "phases": phases,
            "total_tasks": stats.total_tasks,
            "correct_tasks": stats.correct_tasks,
            "final_accuracy": stats.accuracy,
            "avg_reward": stats.avg_reward,
            "elapsed_seconds": elapsed,
            "rewards_history": stats.rewards_history[-1000:],  # Last 1000
        }, f, indent=2)
    
    logger.info(f"Results saved to {results_path}")
    
    return stats


# =============================================================================
# CLI
# =============================================================================

def parse_args():
    parser = argparse.ArgumentParser(
        description="Train an LLM to write Kore programs using supervised curriculum learning."
    )
    
    parser.add_argument(
        "--config", "-c",
        type=str,
        default="config.yaml",
        help="Path to config file (default: config.yaml)"
    )
    
    parser.add_argument(
        "--model", "-m",
        type=str,
        help="Model name to use (overrides config)"
    )
    
    parser.add_argument(
        "--phases", "-p",
        type=str,
        help="Comma-separated phases to train (e.g., '1,2,3'). Default: all"
    )
    
    parser.add_argument(
        "--quantization", "-q",
        type=str,
        choices=["4bit", "8bit"],
        help="Quantization mode (4bit or 8bit)"
    )
    
    parser.add_argument(
        "--seed", "-s",
        type=int,
        default=42,
        help="Random seed (default: 42)"
    )
    
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Test setup without full training"
    )
    
    return parser.parse_args()


def main():
    args = parse_args()
    
    # Set seeds
    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    
    # Load config
    config_path = Path(args.config)
    if config_path.exists():
        with open(config_path) as f:
            config = yaml.safe_load(f)
    else:
        logger.warning(f"Config file {config_path} not found, using defaults")
        config = {}
    
    if args.dry_run:
        logger.info("DRY RUN - testing setup only")
        
        # Test runtime
        runtime = KoreRuntime()
        result = runtime.execute("1 2 add")
        print(f"Runtime test: 1 2 add = {result.stack}")
        
        # Test task generation
        from task_generators import generate_task
        task = generate_task(1)
        print(f"Task example: {task.description}")
        print(f"Effect: {task.expected_effect.to_string()}")
        
        # Test model loading
        print(f"Would load model: {args.model or config.get('model', {}).get('name', 'default')}")
        
        print("\n✓ Dry run successful!")
        return
    
    # Run training
    run_supervised_training(config, args)


if __name__ == "__main__":
    main()
