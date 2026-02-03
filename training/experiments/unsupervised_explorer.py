"""
Unsupervised Kore Exploration Trainer

Key insight: Kore's algebraic structure enables intrinsic motivation
signals that are impossible with conventional languages:

1. Effect Novelty - reward discovering new (consumes, produces, io) signatures
2. Trace Fingerprint - reward behaviorally novel execution patterns
3. Equivalence Discovery - reward finding algebraic identities
4. Capability Descent - reward solving tasks with fewer capabilities
5. Linear Resource Games - reward correct linear type handling

The agent explores, talks to itself, and develops capabilities through
self-play without any task supervision.
"""

import json
import hashlib
import random
import numpy as np
from dataclasses import dataclass, field
from typing import List, Dict, Tuple, Set, Optional, Any
from collections import defaultdict
from pathlib import Path
import subprocess
import time


# =============================================================================
# Effect Space Representation
# =============================================================================

@dataclass
class Effect:
    """Kore program effect signature."""
    consumes: int
    produces: int
    io_effects: frozenset  # e.g., frozenset({"fs", "net"})
    
    def to_vector(self, io_dim: int = 8) -> np.ndarray:
        """Embed effect into continuous space."""
        # Stack effect as normalized 2D
        stack_vec = np.array([self.consumes, self.produces]) / 10.0
        
        # IO effects as one-hot
        io_names = ["fs", "net", "io", "spawn", "exec", "time", "env", "mem"]
        io_vec = np.zeros(io_dim)
        for i, name in enumerate(io_names):
            if name in self.io_effects:
                io_vec[i] = 1.0
        
        return np.concatenate([stack_vec, io_vec])
    
    def __hash__(self):
        return hash((self.consumes, self.produces, self.io_effects))
    
    def distance(self, other: 'Effect') -> float:
        """Distance in effect space."""
        v1 = self.to_vector()
        v2 = other.to_vector()
        return np.linalg.norm(v1 - v2)


def parse_effect(analysis: Dict) -> Effect:
    """Parse effect from Kore effect-infer output."""
    effect = analysis.get("effect", {})
    return Effect(
        consumes=effect.get("consumes", 0),
        produces=effect.get("produces", 0),
        io_effects=frozenset(analysis.get("io", []))
    )


# =============================================================================
# Novelty Memory
# =============================================================================

class EffectMemory:
    """
    Memory of seen effects for novelty computation.
    Uses K-nearest neighbors for efficient novelty lookup.
    """
    
    def __init__(self, max_size: int = 10000):
        self.max_size = max_size
        self.effects: List[Effect] = []
        self.vectors: List[np.ndarray] = []
        self.programs: List[str] = []  # Store programs that produced each effect
        
    def add(self, effect: Effect, program: str):
        """Add effect to memory."""
        if len(self.effects) >= self.max_size:
            # Remove oldest
            self.effects.pop(0)
            self.vectors.pop(0)
            self.programs.pop(0)
        
        self.effects.append(effect)
        self.vectors.append(effect.to_vector())
        self.programs.append(program)
    
    def novelty(self, effect: Effect, k: int = 5) -> float:
        """
        Compute novelty as mean distance to K nearest neighbors.
        Higher = more novel.
        """
        if len(self.vectors) < k:
            return 1.0  # Everything is novel initially
        
        vec = effect.to_vector()
        distances = [np.linalg.norm(vec - v) for v in self.vectors]
        k_nearest = sorted(distances)[:k]
        return np.mean(k_nearest)
    
    def coverage(self) -> Dict[str, Any]:
        """Statistics about effect space coverage."""
        unique_effects = set(self.effects)
        unique_stack = set((e.consumes, e.produces) for e in self.effects)
        unique_io = set(e.io_effects for e in self.effects)
        
        return {
            "total_seen": len(self.effects),
            "unique_effects": len(unique_effects),
            "unique_stack_effects": len(unique_stack),
            "unique_io_patterns": len(unique_io),
        }


class TraceMemory:
    """Memory of execution traces for behavioral novelty."""
    
    def __init__(self, max_size: int = 5000):
        self.max_size = max_size
        self.fingerprints: Set[str] = set()
        self.fingerprint_list: List[str] = []  # For ordering
        
    def add(self, fingerprint: str) -> bool:
        """Add fingerprint, return True if novel."""
        is_novel = fingerprint not in self.fingerprints
        
        if is_novel:
            if len(self.fingerprints) >= self.max_size:
                # Remove oldest
                oldest = self.fingerprint_list.pop(0)
                self.fingerprints.discard(oldest)
            
            self.fingerprints.add(fingerprint)
            self.fingerprint_list.append(fingerprint)
        
        return is_novel
    
    def novelty(self, fingerprint: str) -> float:
        """Binary novelty: 1.0 if never seen, 0.0 otherwise."""
        return 1.0 if fingerprint not in self.fingerprints else 0.0


class EquivalenceMemory:
    """Memory of discovered algebraic equivalences."""
    
    def __init__(self):
        self.equivalences: List[Tuple[str, str]] = []
        self.involutions: Set[str] = set()  # X where X X = ε
        self.annihilations: List[Tuple[str, str]] = []  # (A, B) where A B = ε
        
    def add_equivalence(self, p1: str, p2: str):
        """Record discovered equivalence."""
        self.equivalences.append((p1, p2))
        
        # Detect patterns
        tokens1 = p1.split()
        tokens2 = p2.split()
        
        # Involution: X X = ε
        if len(tokens1) == 2 and tokens1[0] == tokens1[1] and not tokens2:
            self.involutions.add(tokens1[0])
        
        # Annihilation: A B = ε
        if len(tokens1) == 2 and not tokens2:
            self.annihilations.append((tokens1[0], tokens1[1]))
    
    def summary(self) -> Dict[str, Any]:
        return {
            "total_equivalences": len(self.equivalences),
            "involutions": list(self.involutions),
            "annihilations": self.annihilations[:10],  # First 10
        }


# =============================================================================
# Kore Runtime Interface
# =============================================================================

class KoreRuntime:
    """Interface to Kore runtime for exploration."""
    
    def __init__(self, kore_binary: str = "target/release/kore-train"):
        self.kore_binary = kore_binary
        self.timeout = 5.0
        
    def analyze(self, program: str) -> Optional[Dict]:
        """
        Run effect-infer on program.
        Returns analysis dict or None if invalid.
        """
        # Wrap program with effect-infer
        analysis_program = f"[ {program} ] effect-infer"
        
        result = self.execute(analysis_program)
        if result and not result.get("error"):
            # Parse the analysis from stack
            stack = result.get("stack", [])
            if stack and isinstance(stack[0], dict):
                return stack[0]
        return None
    
    def execute(self, program: str, initial_stack: List = None) -> Optional[Dict]:
        """Execute program and return result."""
        try:
            input_data = json.dumps({
                "program": program,
                "stack": initial_stack or [],
                "trace": True,
            })
            
            result = subprocess.run(
                [self.kore_binary],
                input=input_data,
                capture_output=True,
                text=True,
                timeout=self.timeout
            )
            
            if result.returncode == 0:
                return json.loads(result.stdout)
            else:
                return {"error": result.stderr}
                
        except subprocess.TimeoutExpired:
            return {"error": "timeout"}
        except Exception as e:
            return {"error": str(e)}
    
    def verify_equivalence(self, p1: str, p2: str, n_tests: int = 50) -> bool:
        """Test if two programs are equivalent on random inputs."""
        for _ in range(n_tests):
            # Generate random stack
            depth = random.randint(0, 5)
            stack = [random.randint(-100, 100) for _ in range(depth)]
            
            r1 = self.execute(p1, stack.copy())
            r2 = self.execute(p2, stack.copy())
            
            # Both must succeed with same result
            if r1 is None or r2 is None:
                return False
            if r1.get("error") or r2.get("error"):
                # Both error = still equivalent
                if r1.get("error") and r2.get("error"):
                    continue
                return False
            if r1.get("stack") != r2.get("stack"):
                return False
        
        return True
    
    def get_trace_fingerprint(self, program: str) -> Optional[str]:
        """Execute and get trace fingerprint."""
        result = self.execute(program)
        if result and not result.get("error"):
            trace = result.get("trace", [])
            # Hash the trace sequence
            trace_str = json.dumps(trace, sort_keys=True)
            return hashlib.sha256(trace_str.encode()).hexdigest()[:16]
        return None


# =============================================================================
# Exploration Episodes
# =============================================================================

@dataclass
class ExplorationResult:
    """Result of one exploration episode."""
    program: str
    reward: float
    episode_type: str
    info: Dict[str, Any] = field(default_factory=dict)
    

class EffectExplorationEpisode:
    """
    Episode: Generate program, reward effect novelty.
    
    The agent is rewarded for discovering new effect signatures,
    encouraging exploration of Kore's capability space.
    """
    
    def __init__(self, runtime: KoreRuntime, effect_memory: EffectMemory):
        self.runtime = runtime
        self.memory = effect_memory
        
    def run(self, model, temperature: float = 1.0) -> ExplorationResult:
        prompt = """Generate an interesting Kore program that does something new.
Try to create a unique stack effect or use capabilities in novel ways.

Program:"""
        
        program = model.generate(prompt, temperature=temperature, max_tokens=100)
        
        # Analyze effect
        analysis = self.runtime.analyze(program)
        
        if analysis is None:
            return ExplorationResult(
                program=program,
                reward=-0.5,
                episode_type="effect_exploration",
                info={"error": "invalid_program"}
            )
        
        effect = parse_effect(analysis)
        novelty = self.memory.novelty(effect)
        
        # Add to memory
        self.memory.add(effect, program)
        
        # Bonus for pure programs (harder to make interesting)
        purity_bonus = 0.2 if not effect.io_effects else 0.0
        
        reward = novelty + purity_bonus
        
        return ExplorationResult(
            program=program,
            reward=reward,
            episode_type="effect_exploration",
            info={
                "effect": {"consumes": effect.consumes, 
                          "produces": effect.produces,
                          "io": list(effect.io_effects)},
                "novelty": novelty,
            }
        )


class SelfPlayCompositionEpisode:
    """
    Episode: Proposer generates p1, Composer extends to reach target effect.
    
    This is self-play where the agent learns to compose programs
    to achieve specific effect goals.
    """
    
    def __init__(self, runtime: KoreRuntime, effect_memory: EffectMemory):
        self.runtime = runtime
        self.memory = effect_memory
    
    def sample_target_effect(self, base_effect: Effect) -> Effect:
        """Sample a target effect that's compositionally reachable."""
        # Random delta to base effect
        new_produces = max(0, base_effect.produces + random.randint(-2, 3))
        new_consumes = max(0, base_effect.consumes + random.randint(-1, 2))
        
        # Maybe add/remove IO
        new_io = set(base_effect.io_effects)
        if random.random() < 0.3:
            io_options = ["fs", "net", "io", "time"]
            if random.random() < 0.5 and new_io:
                new_io.discard(random.choice(list(new_io)))
            else:
                new_io.add(random.choice(io_options))
        
        return Effect(new_consumes, new_produces, frozenset(new_io))
    
    def run(self, model) -> ExplorationResult:
        # Phase 1: Proposer generates base program
        p1 = model.generate(
            "Generate a short Kore program (2-5 operations):\n",
            temperature=1.0,
            max_tokens=50
        )
        
        analysis1 = self.runtime.analyze(p1)
        if analysis1 is None:
            return ExplorationResult(p1, -0.5, "self_play", {"error": "invalid_p1"})
        
        e1 = parse_effect(analysis1)
        
        # Phase 2: Sample target effect
        target = self.sample_target_effect(e1)
        
        # Phase 3: Composer extends p1 to reach target
        composer_prompt = f"""Given this Kore program:
{p1}

Its effect is: consumes={e1.consumes}, produces={e1.produces}, io={list(e1.io_effects)}

Write a continuation that makes the total effect:
consumes={target.consumes}, produces={target.produces}, io={list(target.io_effects)}

Continuation:"""
        
        p2 = model.generate(composer_prompt, temperature=0.7, max_tokens=50)
        
        # Phase 4: Verify composition
        composed = f"{p1} {p2}"
        analysis_total = self.runtime.analyze(composed)
        
        if analysis_total is None:
            return ExplorationResult(composed, -0.3, "self_play", {"error": "invalid_composition"})
        
        e_total = parse_effect(analysis_total)
        
        # Reward: how close did we get to target?
        distance = target.distance(e_total)
        effect_match_reward = max(0, 1.0 - distance / 5.0)
        
        # Novelty bonus
        novelty = self.memory.novelty(e_total)
        self.memory.add(e_total, composed)
        
        reward = 0.6 * effect_match_reward + 0.4 * novelty
        
        return ExplorationResult(
            program=composed,
            reward=reward,
            episode_type="self_play",
            info={
                "p1": p1,
                "p2": p2,
                "e1": {"c": e1.consumes, "p": e1.produces},
                "target": {"c": target.consumes, "p": target.produces},
                "achieved": {"c": e_total.consumes, "p": e_total.produces},
                "distance": distance,
            }
        )


class EquivalenceDiscoveryEpisode:
    """
    Episode: Generate program, simplify it, verify equivalence.
    
    Reward for discovering non-trivial algebraic identities.
    """
    
    def __init__(self, runtime: KoreRuntime, equiv_memory: EquivalenceMemory):
        self.runtime = runtime
        self.memory = equiv_memory
    
    def run(self, model) -> ExplorationResult:
        # Generate a short program
        p1 = model.generate(
            "Generate a short Kore program (3-8 operations) using stack ops:\n",
            temperature=0.9,
            max_tokens=40
        )
        
        # Ask model to simplify
        p2 = model.generate(
            f"Simplify this Kore program to shortest equivalent form:\n{p1}\n\nSimplified:",
            temperature=0.3,
            max_tokens=40
        )
        
        # Clean up
        p1 = p1.strip()
        p2 = p2.strip()
        
        # Verify equivalence
        equivalent = self.runtime.verify_equivalence(p1, p2)
        
        if not equivalent:
            return ExplorationResult(
                program=p1,
                reward=-0.3,
                episode_type="equivalence",
                info={"error": "not_equivalent", "p1": p1, "p2": p2}
            )
        
        # Reward based on compression ratio
        len1, len2 = len(p1.split()), len(p2.split())
        
        if len2 >= len1:
            # No simplification
            reward = 0.0
        elif len2 == 0:
            # Simplified to identity - very interesting!
            reward = 2.0 if len1 >= 2 else 0.5
            self.memory.add_equivalence(p1, "")
        else:
            # Non-trivial simplification
            ratio = len1 / len2
            reward = min(ratio, 3.0)
            self.memory.add_equivalence(p1, p2)
        
        return ExplorationResult(
            program=p1,
            reward=reward,
            episode_type="equivalence",
            info={
                "p1": p1,
                "p2": p2,
                "len1": len1,
                "len2": len2,
                "ratio": len1 / max(len2, 1),
            }
        )


class LinearResourceEpisode:
    """
    Episode: Solve linear resource puzzles.
    
    Linear values must be used exactly once - no dup, no drop.
    Agent learns resource management through constrained puzzles.
    """
    
    def __init__(self, runtime: KoreRuntime):
        self.runtime = runtime
    
    def generate_puzzle(self, n_resources: int = 2) -> Tuple[str, int]:
        """Generate a linear resource puzzle."""
        values = [random.randint(1, 10) for _ in range(n_resources)]
        setup = " ".join(f"{v} linear-new" for v in values)
        target = sum(values)
        return setup, target
    
    def run(self, model, difficulty: int = 2) -> ExplorationResult:
        setup, target = self.generate_puzzle(difficulty)
        
        prompt = f"""Solve this linear resource puzzle:

Setup: {setup}
You have {difficulty} linear values on the stack.
Each MUST be used exactly once with linear-unwrap.
Compute their sum.

Expected result: single integer {target}

Solution:"""
        
        solution = model.generate(prompt, temperature=0.5, max_tokens=60)
        
        # Full program
        program = f"{setup} {solution}"
        
        result = self.runtime.execute(program)
        
        if result is None or result.get("error"):
            error_msg = result.get("error", "unknown") if result else "timeout"
            
            # Partial credit for linear violation (it means they tried)
            if "linear" in error_msg.lower():
                reward = -0.2  # Tried but violated linearity
            else:
                reward = -0.5
            
            return ExplorationResult(
                program=program,
                reward=reward,
                episode_type="linear_puzzle",
                info={"error": error_msg, "difficulty": difficulty}
            )
        
        # Check result
        stack = result.get("stack", [])
        
        if len(stack) == 1 and stack[0] == target:
            reward = 1.0 + 0.2 * difficulty  # Bonus for harder puzzles
        elif len(stack) == 1 and isinstance(stack[0], int):
            # Wrong answer but valid computation
            reward = 0.3
        else:
            reward = 0.0
        
        return ExplorationResult(
            program=program,
            reward=reward,
            episode_type="linear_puzzle",
            info={
                "target": target,
                "got": stack,
                "correct": len(stack) == 1 and stack[0] == target,
                "difficulty": difficulty,
            }
        )


class CapabilityDescentEpisode:
    """
    Episode: Solve task with minimal capabilities.
    
    Agent learns to accomplish goals while using fewer IO effects,
    discovering pure alternatives to effectful operations.
    """
    
    TASKS = [
        {
            "name": "reverse_list",
            "prompt": "Reverse the list [1 2 3 4 5]",
            "test_input": "[1 2 3 4 5]",
            "expected": [5, 4, 3, 2, 1],
            "minimal_io": frozenset(),  # Can be done pure
        },
        {
            "name": "sum_list",
            "prompt": "Sum all elements of [10 20 30]",
            "test_input": "[10 20 30]",
            "expected": 60,
            "minimal_io": frozenset(),
        },
        {
            "name": "fetch_and_parse",
            "prompt": "Fetch data from URL and parse as JSON",
            "test_input": '"https://api.example.com/data"',
            "expected": None,  # Can't test without network
            "minimal_io": frozenset({"net"}),  # Requires net
        },
    ]
    
    def __init__(self, runtime: KoreRuntime):
        self.runtime = runtime
    
    def run(self, model) -> ExplorationResult:
        task = random.choice(self.TASKS)
        
        prompt = f"""Task: {task['prompt']}
Input on stack: {task['test_input']}

Write a Kore program that solves this.
Try to use as few IO effects as possible (prefer pure computation).

Program:"""
        
        program = model.generate(prompt, temperature=0.6, max_tokens=80)
        
        # Analyze effects
        analysis = self.runtime.analyze(program)
        
        if analysis is None:
            return ExplorationResult(
                program=program,
                reward=-0.5,
                episode_type="capability_descent",
                info={"error": "invalid_program", "task": task["name"]}
            )
        
        effect = parse_effect(analysis)
        
        # Execute to check correctness (for testable tasks)
        if task["expected"] is not None:
            full_program = f"{task['test_input']} {program}"
            result = self.runtime.execute(full_program)
            
            if result and not result.get("error"):
                stack = result.get("stack", [])
                correct = len(stack) == 1 and stack[0] == task["expected"]
            else:
                correct = False
        else:
            correct = None  # Can't verify
        
        # Compute reward
        if correct is False:
            reward = -0.2  # Incorrect
        else:
            # Reward for correctness + capability minimality
            correctness = 1.0 if correct else 0.5  # Partial credit if can't test
            
            # How close to minimal IO?
            actual_io = effect.io_effects
            minimal_io = task["minimal_io"]
            extra_io = len(actual_io - minimal_io)
            io_penalty = 0.2 * extra_io
            
            reward = correctness - io_penalty
        
        return ExplorationResult(
            program=program,
            reward=reward,
            episode_type="capability_descent",
            info={
                "task": task["name"],
                "io_used": list(effect.io_effects),
                "minimal_io": list(task["minimal_io"]),
                "correct": correct,
            }
        )


# =============================================================================
# Unified Trainer
# =============================================================================

class UnsupervisedKoreTrainer:
    """
    Unified trainer for unsupervised Kore exploration.
    
    Combines multiple intrinsic motivation signals:
    - Effect novelty
    - Self-play composition
    - Equivalence discovery
    - Linear resource games
    - Capability descent
    """
    
    def __init__(
        self,
        model,
        runtime: KoreRuntime,
        log_dir: Path = Path("logs/unsupervised"),
    ):
        self.model = model
        self.runtime = runtime
        self.log_dir = log_dir
        self.log_dir.mkdir(parents=True, exist_ok=True)
        
        # Memory modules
        self.effect_memory = EffectMemory()
        self.trace_memory = TraceMemory()
        self.equiv_memory = EquivalenceMemory()
        
        # Episode types with weights
        self.episodes = {
            "effect_exploration": (
                EffectExplorationEpisode(runtime, self.effect_memory),
                0.25,
            ),
            "self_play": (
                SelfPlayCompositionEpisode(runtime, self.effect_memory),
                0.25,
            ),
            "equivalence": (
                EquivalenceDiscoveryEpisode(runtime, self.equiv_memory),
                0.20,
            ),
            "linear_puzzle": (
                LinearResourceEpisode(runtime),
                0.15,
            ),
            "capability_descent": (
                CapabilityDescentEpisode(runtime),
                0.15,
            ),
        }
        
        # Statistics
        self.stats = defaultdict(lambda: {"count": 0, "total_reward": 0.0})
        self.discoveries = []
        
    def sample_episode_type(self) -> str:
        """Sample episode type based on weights."""
        types = list(self.episodes.keys())
        weights = [self.episodes[t][1] for t in types]
        return np.random.choice(types, p=weights)
    
    def train_step(self) -> ExplorationResult:
        """One step of unsupervised training."""
        
        episode_type = self.sample_episode_type()
        episode_runner, _ = self.episodes[episode_type]
        
        if episode_type == "linear_puzzle":
            # Curriculum: increase difficulty over time
            difficulty = min(2 + self.stats["linear_puzzle"]["count"] // 100, 5)
            result = episode_runner.run(self.model, difficulty=difficulty)
        else:
            result = episode_runner.run(self.model)
        
        # Update stats
        self.stats[episode_type]["count"] += 1
        self.stats[episode_type]["total_reward"] += result.reward
        
        # Log discoveries
        if result.reward > 1.5:
            self.discoveries.append({
                "type": episode_type,
                "reward": result.reward,
                "program": result.program,
                "info": result.info,
                "step": sum(s["count"] for s in self.stats.values()),
            })
        
        return result
    
    def train(self, n_steps: int, log_every: int = 100):
        """Run training loop."""
        
        for step in range(n_steps):
            result = self.train_step()
            
            # TODO: Update model with policy gradient
            # For now, just collecting data
            
            if (step + 1) % log_every == 0:
                self._log_progress(step + 1)
        
        self._save_final_report()
    
    def _log_progress(self, step: int):
        """Log training progress."""
        print(f"\n=== Step {step} ===")
        
        for ep_type, stats in self.stats.items():
            if stats["count"] > 0:
                avg_reward = stats["total_reward"] / stats["count"]
                print(f"{ep_type}: count={stats['count']}, avg_reward={avg_reward:.3f}")
        
        print(f"Effect coverage: {self.effect_memory.coverage()}")
        print(f"Equivalences found: {self.equiv_memory.summary()}")
        print(f"Total discoveries: {len(self.discoveries)}")
    
    def _save_final_report(self):
        """Save final training report."""
        report = {
            "stats": dict(self.stats),
            "effect_coverage": self.effect_memory.coverage(),
            "equivalences": self.equiv_memory.summary(),
            "discoveries": self.discoveries[-100:],  # Last 100
        }
        
        report_path = self.log_dir / "final_report.json"
        with open(report_path, "w") as f:
            json.dump(report, f, indent=2, default=str)
        
        print(f"\nReport saved to {report_path}")


# =============================================================================
# Reward Shaping via Effect Algebra
# =============================================================================

def compute_effect_potential(effect: Effect, target: Effect) -> float:
    """
    Potential-based reward shaping using effect distance.
    
    Φ(e) = -||e - target||
    
    This provides dense reward signal towards target effect.
    """
    return -effect.distance(target)


def shaped_reward(
    old_effect: Effect,
    new_effect: Effect,
    target: Effect,
    gamma: float = 0.99,
    base_reward: float = 0.0
) -> float:
    """
    Compute shaped reward: R + γΦ(s') - Φ(s)
    
    This is guaranteed to preserve optimal policy (Ng et al., 1999)
    while providing denser learning signal.
    """
    potential_old = compute_effect_potential(old_effect, target)
    potential_new = compute_effect_potential(new_effect, target)
    
    shaping = gamma * potential_new - potential_old
    return base_reward + shaping


# =============================================================================
# Gradient Computation
# =============================================================================

def compute_policy_gradient(
    model,
    results: List[ExplorationResult],
    baseline: str = "mean"
) -> Dict[str, Any]:
    """
    Compute policy gradient from exploration results.
    
    Uses REINFORCE with baseline subtraction.
    """
    rewards = [r.reward for r in results]
    programs = [r.program for r in results]
    
    # Compute baseline
    if baseline == "mean":
        b = np.mean(rewards)
    elif baseline == "median":
        b = np.median(rewards)
    else:
        b = 0.0
    
    # Compute advantages
    advantages = [r - b for r in rewards]
    
    # Normalize
    std = np.std(advantages) + 1e-8
    advantages = [a / std for a in advantages]
    
    # Return gradient info (actual gradient computation depends on model)
    return {
        "programs": programs,
        "advantages": advantages,
        "mean_reward": np.mean(rewards),
        "max_reward": max(rewards),
        "positive_samples": sum(1 for a in advantages if a > 0),
    }


# =============================================================================
# Main Entry Point
# =============================================================================

if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(description="Unsupervised Kore Exploration")
    parser.add_argument("--steps", type=int, default=1000)
    parser.add_argument("--log-every", type=int, default=100)
    parser.add_argument("--kore-binary", default="target/release/kore-train")
    args = parser.parse_args()
    
    # Placeholder model (replace with actual LLM)
    class DummyModel:
        def generate(self, prompt, temperature=1.0, max_tokens=100):
            # Random Kore program for testing
            ops = ["dup", "drop", "swap", "rot", "over", 
                   "add", "sub", "mul", "1", "2", "3"]
            n = random.randint(2, 6)
            return " ".join(random.choice(ops) for _ in range(n))
    
    model = DummyModel()
    runtime = KoreRuntime(args.kore_binary)
    
    trainer = UnsupervisedKoreTrainer(model, runtime)
    trainer.train(args.steps, args.log_every)
