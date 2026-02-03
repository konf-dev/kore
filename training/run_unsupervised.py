#!/usr/bin/env python3
"""
Unsupervised Kore Exploration Experiment

This script trains an LLM to discover novel Kore programs through:
- Effect exploration: Find programs that produce interesting effects
- Self-play: Generate puzzle-solution pairs
- Equivalence discovery: Find multiple programs with the same behavior
- Linear type puzzles: Master resource management
- Capability descent: Learn security patterns

Usage:
    python run_unsupervised.py --config config.yaml
    python run_unsupervised.py --model "Qwen/Qwen2.5-Coder-3B-Instruct" --episodes 1000
"""

import argparse
import json
import logging
import os
import random
import sys
import time
import hashlib
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple, Set

import yaml
import torch
import numpy as np
from tqdm import tqdm

from kore_runtime import (
    KoreRuntime,
    ExecutionResult,
    EffectSignature,
    SYSTEM_PROMPT,
)
from model_interface import load_model, GenerationConfig


# =============================================================================
# Logging Setup
# =============================================================================

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler('unsupervised.log'),
    ]
)
logger = logging.getLogger(__name__)


# =============================================================================
# Discovery Tracking
# =============================================================================

@dataclass
class Discovery:
    """A discovered program or pattern."""
    program: str
    effect: EffectSignature
    episode_type: str
    novelty_score: float
    timestamp: float
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "program": self.program,
            "effect": self.effect.to_string() if self.effect else "unknown",
            "episode_type": self.episode_type,
            "novelty_score": self.novelty_score,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        }


class DiscoveryBank:
    """Track and store discoveries."""
    
    def __init__(self, max_size: int = 10000):
        self.max_size = max_size
        self.discoveries: List[Discovery] = []
        self.effect_index: Dict[str, List[int]] = defaultdict(list)
        self.program_hashes: Set[str] = set()
    
    def add(self, discovery: Discovery) -> bool:
        """Add discovery if novel. Returns True if added."""
        
        # Hash for deduplication
        prog_hash = hashlib.md5(discovery.program.encode()).hexdigest()
        if prog_hash in self.program_hashes:
            return False
        
        self.program_hashes.add(prog_hash)
        idx = len(self.discoveries)
        self.discoveries.append(discovery)
        
        # Index by effect
        if discovery.effect:
            effect_key = discovery.effect.to_string()
            self.effect_index[effect_key].append(idx)
        
        # Evict old discoveries if over limit
        if len(self.discoveries) > self.max_size:
            # Remove lowest novelty score
            min_idx = min(range(len(self.discoveries)), 
                         key=lambda i: self.discoveries[i].novelty_score)
            old = self.discoveries.pop(min_idx)
            old_hash = hashlib.md5(old.program.encode()).hexdigest()
            self.program_hashes.discard(old_hash)
        
        return True
    
    def get_equivalence_groups(self) -> Dict[str, List[str]]:
        """Get groups of programs with the same effect."""
        groups = {}
        for effect_key, indices in self.effect_index.items():
            if len(indices) >= 2:
                programs = [self.discoveries[i].program for i in indices]
                groups[effect_key] = programs
        return groups
    
    def sample(self, episode_type: Optional[str] = None, n: int = 1) -> List[Discovery]:
        """Sample discoveries, optionally filtered by type."""
        candidates = self.discoveries
        if episode_type:
            candidates = [d for d in candidates if d.episode_type == episode_type]
        
        if not candidates:
            return []
        
        return random.sample(candidates, min(n, len(candidates)))
    
    def save(self, path: Path):
        """Save discoveries to JSON."""
        with open(path, 'w') as f:
            json.dump([d.to_dict() for d in self.discoveries], f, indent=2)
    
    def stats(self) -> Dict[str, Any]:
        return {
            "total": len(self.discoveries),
            "unique_effects": len(self.effect_index),
            "by_type": {
                ep_type: len([d for d in self.discoveries if d.episode_type == ep_type])
                for ep_type in set(d.episode_type for d in self.discoveries)
            }
        }


# =============================================================================
# Episode Types
# =============================================================================

def effect_exploration_episode(
    model,
    runtime: KoreRuntime,
    bank: DiscoveryBank,
    config: Dict[str, Any],
) -> Tuple[float, Optional[Discovery]]:
    """
    Effect Exploration: Generate programs with novel effects.
    
    Prompt the LLM to generate a program, then reward based on:
    - Effect novelty (new effect signature)
    - Program compiles/runs
    - Interesting stack transformations
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.9),  # Higher for exploration
        top_p=config.get("top_p", 0.95),
    )
    
    # Sample some existing effects for context
    existing = bank.sample(n=3)
    existing_effects = [d.effect.to_string() for d in existing if d.effect]
    
    prompt = f"""Generate a novel Kore program that produces an interesting effect.

Known effects in the library:
{chr(10).join(f'- {e}' for e in existing_effects[:5]) if existing_effects else '- (none yet)'}

Your goal: Create a program with a DIFFERENT effect signature.
Focus on interesting stack transformations like:
- (1 -- 2): duplicate or expand
- (3 -- 1): aggregate or fold
- (1 -- 1) with interesting computation

Write a short, pure Kore program (no IO):"""
    
    program = model.generate(SYSTEM_PROMPT, prompt, gen_config)
    program = program.strip().split("\n")[0]  # First line only
    
    # Get effect
    effect = runtime.get_effect(program)
    
    if effect is None:
        return 0.0, None
    
    # Verify it runs
    test_result = runtime.execute(program, [1, 2, 3, 4, 5][:effect.consumes])
    if not test_result.success:
        return 0.05, None  # Tiny reward for valid syntax
    
    # Compute novelty
    effect_key = effect.to_string()
    existing_count = len(bank.effect_index.get(effect_key, []))
    
    if existing_count == 0:
        novelty = 1.0  # New effect!
    elif existing_count < 3:
        novelty = 0.5  # Still interesting
    else:
        novelty = 0.1  # Common effect
    
    # Bonus for complex transformations
    complexity_bonus = 0.1 * min(5, effect.consumes + effect.produces)
    
    total_reward = novelty + complexity_bonus
    
    discovery = Discovery(
        program=program,
        effect=effect,
        episode_type="effect_exploration",
        novelty_score=novelty,
        timestamp=time.time(),
        metadata={"complexity_bonus": complexity_bonus},
    )
    
    return total_reward, discovery


def self_play_episode(
    model,
    runtime: KoreRuntime,
    bank: DiscoveryBank,
    config: Dict[str, Any],
) -> Tuple[float, Optional[Discovery]]:
    """
    Self-Play: Generate a puzzle, then solve it.
    
    1. LLM generates a program (the "oracle")
    2. We extract its effect and some test cases
    3. LLM tries to generate a different program with same behavior
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.8),
    )
    
    # Phase 1: Generate oracle program
    oracle_prompt = """Generate a Kore program that does an interesting computation.
Keep it short (1-3 operations). Examples:
- "dup mul" (squares a number)
- "over add" (adds top to second)
- "rot sub" (subtracts after rotation)

Your program:"""
    
    oracle = model.generate(SYSTEM_PROMPT, oracle_prompt, gen_config)
    oracle = oracle.strip().split("\n")[0].strip()
    
    oracle_effect = runtime.get_effect(oracle)
    if oracle_effect is None:
        return 0.0, None
    
    # Generate test cases from oracle
    test_cases = []
    for _ in range(5):
        inputs = [random.randint(1, 20) for _ in range(oracle_effect.consumes)]
        result = runtime.execute(oracle, inputs.copy())
        if result.success:
            test_cases.append((inputs, result.stack))
    
    if len(test_cases) < 3:
        return 0.05, None  # Oracle doesn't work well
    
    # Phase 2: Solve the puzzle
    solve_prompt = f"""Find a Kore program that matches this behavior:

Effect: {oracle_effect.to_string()}
Test cases:
{chr(10).join(f'  {inp} -> {out}' for inp, out in test_cases[:3])}

Write a program that produces the same outputs (can be different from the original):"""
    
    solution = model.generate(SYSTEM_PROMPT, solve_prompt, gen_config)
    solution = solution.strip().split("\n")[0].strip()
    
    # Verify solution
    if solution == oracle:
        # Same program - partial credit
        return 0.3, None
    
    sol_effect = runtime.get_effect(solution)
    if sol_effect is None:
        return 0.1, None
    
    # Check test cases
    correct = 0
    for inputs, expected in test_cases:
        result = runtime.execute(solution, inputs.copy())
        if result.success and result.stack == expected:
            correct += 1
    
    accuracy = correct / len(test_cases)
    
    if accuracy >= 0.8:
        # Found an equivalent program!
        discovery = Discovery(
            program=f"{oracle} === {solution}",  # Store pair
            effect=oracle_effect,
            episode_type="self_play",
            novelty_score=1.0,
            timestamp=time.time(),
            metadata={"oracle": oracle, "solution": solution, "accuracy": accuracy},
        )
        return 1.0, discovery
    
    return 0.3 * accuracy, None


def equivalence_episode(
    model,
    runtime: KoreRuntime,
    bank: DiscoveryBank,
    config: Dict[str, Any],
) -> Tuple[float, Optional[Discovery]]:
    """
    Equivalence Discovery: Find alternative implementations.
    
    Given an existing program, find a semantically equivalent but
    syntactically different program.
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.7),
    )
    
    # Sample an existing program
    existing = bank.sample(episode_type="effect_exploration", n=1)
    if not existing:
        # Bootstrap with a simple program
        existing = [Discovery(
            program="dup add",
            effect=EffectSignature(1, 1),
            episode_type="bootstrap",
            novelty_score=0.5,
            timestamp=time.time(),
        )]
    
    target = existing[0]
    
    # Generate test cases
    test_cases = []
    for _ in range(5):
        inputs = [random.randint(1, 20) for _ in range(target.effect.consumes)]
        result = runtime.execute(target.program, inputs.copy())
        if result.success:
            test_cases.append((inputs, result.stack))
    
    if not test_cases:
        return 0.0, None
    
    prompt = f"""Find an ALTERNATIVE Kore program that produces the same results.

Original program: {target.program}
Effect: {target.effect.to_string()}

Test cases:
{chr(10).join(f'  {inp} -> {out}' for inp, out in test_cases[:3])}

Write a DIFFERENT program with the same behavior:"""
    
    alternative = model.generate(SYSTEM_PROMPT, prompt, gen_config)
    alternative = alternative.strip().split("\n")[0].strip()
    
    # Verify it's different
    if alternative == target.program:
        return 0.0, None
    
    alt_effect = runtime.get_effect(alternative)
    if alt_effect is None:
        return 0.05, None
    
    # Check equivalence
    correct = 0
    for inputs, expected in test_cases:
        result = runtime.execute(alternative, inputs.copy())
        if result.success and result.stack == expected:
            correct += 1
    
    accuracy = correct / len(test_cases)
    
    if accuracy >= 0.9:
        discovery = Discovery(
            program=alternative,
            effect=alt_effect,
            episode_type="equivalence",
            novelty_score=0.8,
            timestamp=time.time(),
            metadata={"equivalent_to": target.program, "accuracy": accuracy},
        )
        return 1.0, discovery
    
    return 0.2 * accuracy, None


def linear_puzzle_episode(
    model,
    runtime: KoreRuntime,
    bank: DiscoveryBank,
    config: Dict[str, Any],
) -> Tuple[float, Optional[Discovery]]:
    """
    Linear Type Puzzles: Master resource management.
    
    Generate puzzles involving linear types where resources
    must be used exactly once.
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.7),
    )
    
    puzzles = [
        {
            "desc": "Create a linear resource and consume it exactly once",
            "setup": "1 linear-new",  # Creates a linear value
            "goal": "Use the linear value in a computation, then consume it with linear-drop",
            "verify": lambda prog, runtime: (
                "linear-drop" in prog and 
                runtime.get_effect(prog) is not None
            ),
        },
        {
            "desc": "Split a linear resource, use both parts",
            "setup": "1 linear-new",
            "goal": "Split the linear value, use both parts, drop both",
            "verify": lambda prog, runtime: (
                prog.count("linear-drop") >= 1 and
                runtime.get_effect(prog) is not None
            ),
        },
        {
            "desc": "Thread a linear resource through computation",
            "setup": "42 linear-new",
            "goal": "Pass linear value through operations while maintaining linearity",
            "verify": lambda prog, runtime: runtime.get_effect(prog) is not None,
        },
    ]
    
    puzzle = random.choice(puzzles)
    
    prompt = f"""Solve this linear type puzzle:

Setup: {puzzle['setup']}
Goal: {puzzle['goal']}

Linear values must be used exactly once - not duplicated, not dropped (except with linear-drop).

Write a Kore program that correctly handles the linear resource:"""
    
    solution = model.generate(SYSTEM_PROMPT, prompt, gen_config)
    solution = solution.strip().split("\n")[0].strip()
    
    # Full program
    full_program = f"{puzzle['setup']} {solution}"
    
    # Verify
    if puzzle["verify"](full_program, runtime):
        result = runtime.execute(full_program, [])
        if result.success:
            discovery = Discovery(
                program=full_program,
                effect=runtime.get_effect(full_program),
                episode_type="linear_puzzle",
                novelty_score=0.7,
                timestamp=time.time(),
                metadata={"puzzle": puzzle["desc"]},
            )
            return 1.0, discovery
    
    return 0.1, None


def capability_descent_episode(
    model,
    runtime: KoreRuntime,
    bank: DiscoveryBank,
    config: Dict[str, Any],
) -> Tuple[float, Optional[Discovery]]:
    """
    Capability Descent: Learn security patterns.
    
    Start with high capabilities, progressively restrict,
    and accomplish goals with minimal capabilities.
    """
    
    gen_config = GenerationConfig(
        max_new_tokens=config.get("max_tokens", 256),
        temperature=config.get("temperature", 0.6),
    )
    
    scenarios = [
        {
            "desc": "File access with minimal permissions",
            "full_cap": "fs-read fs-write",
            "task": "Read a value, increment it, but DON'T write (read-only scenario)",
            "minimal_cap": "fs-read",
            "verify": lambda prog: "fs-write" not in prog and "fs-read" in prog,
        },
        {
            "desc": "Pure computation without IO",
            "full_cap": "all-io",
            "task": "Double a number without using any IO capabilities",
            "minimal_cap": "(none)",
            "verify": lambda prog: "fs-" not in prog and "net-" not in prog,
        },
        {
            "desc": "Restricted capability scope",
            "full_cap": "global",
            "task": "Compute factorial using only stack operations (no IO)",
            "minimal_cap": "stack-only",
            "verify": lambda prog: runtime.get_effect(prog) is not None and
                                   runtime.get_effect(prog).io_effects == [],
        },
    ]
    
    scenario = random.choice(scenarios)
    
    prompt = f"""Security-conscious Kore programming challenge:

Task: {scenario['task']}
Constraint: Use only minimal capabilities ({scenario['minimal_cap']})

Write a Kore program that accomplishes the task with minimal capability requirements:"""
    
    solution = model.generate(SYSTEM_PROMPT, prompt, gen_config)
    solution = solution.strip().split("\n")[0].strip()
    
    effect = runtime.get_effect(solution)
    if effect is None:
        return 0.0, None
    
    # Check if it uses minimal capabilities
    is_minimal = scenario["verify"](solution)
    
    # Check if it works
    result = runtime.execute(solution, [5])  # Test input
    works = result.success
    
    if is_minimal and works:
        discovery = Discovery(
            program=solution,
            effect=effect,
            episode_type="capability_descent",
            novelty_score=0.8,
            timestamp=time.time(),
            metadata={"scenario": scenario["desc"], "minimal": True},
        )
        return 1.0, discovery
    elif works:
        return 0.4, None  # Works but not minimal
    else:
        return 0.1, None


# Episode registry
EPISODE_TYPES = {
    "effect_exploration": effect_exploration_episode,
    "self_play": self_play_episode,
    "equivalence": equivalence_episode,
    "linear_puzzle": linear_puzzle_episode,
    "capability_descent": capability_descent_episode,
}


# =============================================================================
# Main Training Loop
# =============================================================================

def run_unsupervised_training(config: Dict[str, Any], args: argparse.Namespace):
    """Main unsupervised training entry point."""
    
    logger.info("="*60)
    logger.info("Kore Unsupervised Exploration")
    logger.info("="*60)
    logger.info(f"Config: {json.dumps(config, indent=2, default=str)}")
    
    # Initialize runtime
    kore_binary = config.get("runtime", {}).get("binary", "target/release/kore-train")
    runtime = KoreRuntime(kore_binary=kore_binary)
    
    # Verify runtime
    test_result = runtime.execute("1 2 add")
    if not test_result.success or test_result.stack != [3]:
        logger.error(f"Kore runtime test failed: {test_result}")
        sys.exit(1)
    
    logger.info("✓ Kore runtime verified")
    
    # Load model
    model_name = args.model or config.get("model", {}).get("name", "Qwen/Qwen2.5-Coder-7B-Instruct")
    quantization = args.quantization or config.get("model", {}).get("quantization")
    
    logger.info(f"Loading model: {model_name}")
    model = load_model(
        model_name=model_name,
        quantization=quantization,
        device="cuda" if torch.cuda.is_available() else "cpu",
    )
    
    logger.info("✓ Model loaded")
    
    # Initialize discovery bank
    bank = DiscoveryBank(max_size=config.get("unsupervised", {}).get("bank_size", 10000))
    
    # Episode weights
    unsupervised_config = config.get("unsupervised", {})
    weights = unsupervised_config.get("episode_weights", {})
    episode_weights = {
        "effect_exploration": weights.get("effect_exploration", 0.25),
        "self_play": weights.get("self_play", 0.25),
        "equivalence": weights.get("equivalence", 0.20),
        "linear_puzzle": weights.get("linear_puzzle", 0.15),
        "capability_descent": weights.get("capability_descent", 0.15),
    }
    
    # Normalize weights
    total_weight = sum(episode_weights.values())
    episode_weights = {k: v/total_weight for k, v in episode_weights.items()}
    
    logger.info(f"Episode weights: {episode_weights}")
    
    # Training loop
    n_episodes = args.episodes or unsupervised_config.get("n_episodes", 10000)
    training_config = config.get("training", {})
    
    stats = {
        "total_episodes": 0,
        "discoveries": 0,
        "total_reward": 0.0,
        "by_type": {k: {"episodes": 0, "discoveries": 0, "reward": 0.0} for k in EPISODE_TYPES},
    }
    
    start_time = time.time()
    
    pbar = tqdm(range(n_episodes), desc="Exploring")
    
    for episode_idx in pbar:
        # Sample episode type
        episode_type = random.choices(
            list(episode_weights.keys()),
            weights=list(episode_weights.values()),
        )[0]
        
        # Run episode
        episode_fn = EPISODE_TYPES[episode_type]
        reward, discovery = episode_fn(model, runtime, bank, training_config)
        
        # Update stats
        stats["total_episodes"] += 1
        stats["total_reward"] += reward
        stats["by_type"][episode_type]["episodes"] += 1
        stats["by_type"][episode_type]["reward"] += reward
        
        if discovery:
            added = bank.add(discovery)
            if added:
                stats["discoveries"] += 1
                stats["by_type"][episode_type]["discoveries"] += 1
                
                # Log notable discoveries
                if discovery.novelty_score >= 0.8:
                    logger.info(f"\n🔍 Notable discovery ({episode_type}): {discovery.program}")
                    logger.info(f"   Effect: {discovery.effect.to_string()}")
        
        # Update progress
        if episode_idx % 10 == 0:
            pbar.set_postfix({
                "discoveries": stats["discoveries"],
                "avg_reward": f"{stats['total_reward']/max(1, stats['total_episodes']):.3f}",
                "unique_effects": len(bank.effect_index),
            })
        
        # Periodic save
        if episode_idx % 1000 == 0 and episode_idx > 0:
            save_dir = Path(config.get("logging", {}).get("log_dir", "results"))
            save_dir.mkdir(parents=True, exist_ok=True)
            bank.save(save_dir / f"discoveries_{episode_idx}.json")
            logger.info(f"Saved {len(bank.discoveries)} discoveries")
    
    # Final summary
    elapsed = time.time() - start_time
    
    logger.info("\n" + "="*60)
    logger.info("EXPLORATION COMPLETE")
    logger.info("="*60)
    logger.info(f"Total time: {elapsed/3600:.1f} hours")
    logger.info(f"Total episodes: {stats['total_episodes']}")
    logger.info(f"Total discoveries: {stats['discoveries']}")
    logger.info(f"Unique effects: {len(bank.effect_index)}")
    logger.info(f"Average reward: {stats['total_reward']/max(1, stats['total_episodes']):.3f}")
    
    logger.info("\nBy episode type:")
    for ep_type, ep_stats in stats["by_type"].items():
        if ep_stats["episodes"] > 0:
            logger.info(f"  {ep_type}:")
            logger.info(f"    Episodes: {ep_stats['episodes']}")
            logger.info(f"    Discoveries: {ep_stats['discoveries']}")
            logger.info(f"    Avg reward: {ep_stats['reward']/ep_stats['episodes']:.3f}")
    
    # Save final results
    results_dir = Path(config.get("logging", {}).get("log_dir", "results"))
    results_dir.mkdir(parents=True, exist_ok=True)
    
    # Save discoveries
    bank.save(results_dir / f"discoveries_final.json")
    
    # Save stats
    stats_path = results_dir / f"unsupervised_stats_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    with open(stats_path, 'w') as f:
        json.dump({
            "config": config,
            "stats": stats,
            "bank_stats": bank.stats(),
            "equivalence_groups": bank.get_equivalence_groups(),
            "elapsed_seconds": elapsed,
        }, f, indent=2, default=str)
    
    logger.info(f"\nResults saved to {results_dir}")
    
    return stats, bank


# =============================================================================
# CLI
# =============================================================================

def parse_args():
    parser = argparse.ArgumentParser(
        description="Unsupervised Kore exploration - discover novel programs through intrinsic motivation."
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
        "--episodes", "-e",
        type=int,
        help="Number of episodes to run (overrides config)"
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
        
        # Test discovery bank
        bank = DiscoveryBank()
        discovery = Discovery(
            program="dup mul",
            effect=EffectSignature(1, 1),
            episode_type="test",
            novelty_score=1.0,
            timestamp=time.time(),
        )
        bank.add(discovery)
        print(f"Discovery bank: {bank.stats()}")
        
        print("\n✓ Dry run successful!")
        return
    
    # Run training
    run_unsupervised_training(config, args)


if __name__ == "__main__":
    main()
