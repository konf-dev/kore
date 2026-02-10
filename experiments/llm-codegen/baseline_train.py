#!/usr/bin/env python3
"""
Baseline Trainer — Evolutionary program search .

No neural network. No LLM. Just mutation + selection + curriculum.
Proves the eval loop works end-to-end before investing in a policy network.

Architecture:
  1. Maintain a population of token sequences per level
  2. Evaluate all programs via korec serve (50K/sec)
  3. Score with dense reward (scorer.py)
  4. Select best, mutate, repeat
  5. Advance curriculum level when success rate > threshold

Expected results (from TRAINING_PLAN.md):
  - Levels 0-5 solved in ~45 seconds
  - Levels 6+ may not converge (need a learned policy)
  - Infrastructure validated for Phase 2

Usage:
  python baseline_train.py                     # Run with defaults
  python baseline_train.py --max-level 5       # Stop at level 5
  python baseline_train.py --pop-size 100      # Larger population
  python baseline_train.py --episodes 5000     # More episodes
"""

from __future__ import annotations
import argparse
import time
import random
import sys
import os

# Add this directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from experiment_runner import ServeRunner, EvalResult
from curriculum import Curriculum, Task, TASK_POOLS, MAX_LEVEL
from mutator import mutate, random_program, mutate_population, crossover
from scorer import score, ScoreBreakdown


# =============================================================================
# Hall of Fame — best programs found per task description
# =============================================================================

class HallOfFame:
    """Stores the best program found for each task description."""

    def __init__(self):
        self.best: dict[str, tuple[float, list[str], str]] = {}
        # key: task description
        # value: (score, tokens, source)

    def update(self, task: Task, tokens: list[str], total_score: float):
        source = " ".join(tokens)
        key = task.description
        if key not in self.best or total_score > self.best[key][0]:
            self.best[key] = (total_score, list(tokens), source)

    def get(self, description: str):
        return self.best.get(description)

    def solved_count(self) -> int:
        return sum(1 for s, _, _ in self.best.values() if s >= 1.0)

    def summary(self, level: int = None) -> str:
        lines = []
        for desc in sorted(self.best.keys()):
            sc, tokens, source = self.best[desc]
            marker = "✓" if sc >= 1.0 else "·"
            lines.append(f"  {marker} {sc:+.2f} | {desc:30s} | {source}")
        return "\n".join(lines)


# =============================================================================
# Seeding — inject known-good patterns into initial population
# =============================================================================

def seed_population(level: int, pop_size: int, rng: random.Random) -> list[list[str]]:
    """
    Create initial population. Mix of:
      - Random programs (diversity)
      - Simple patterns (push literal, push + op)
      - Hint programs from curriculum (if any — gives a head start)
    """
    pop: list[list[str]] = []

    # Seed from task hints (20% of pop)
    pool = TASK_POOLS.get(level, [])
    for task in pool:
        if task.hint and len(pop) < pop_size // 5:
            tokens = task.hint.split()
            pop.append(tokens)

    # Some "push literal" seeds for level 0
    if level == 0:
        for n in range(11):
            if len(pop) < pop_size:
                pop.append([str(n)])

    # Some "a b op" patterns for level 1
    if level >= 1:
        for a in ["1", "2", "3", "5"]:
            for b in ["2", "3", "4", "5"]:
                for op in ["+", "-", "*"]:
                    if len(pop) < pop_size * 0.4:
                        pop.append([a, b, op])

    # Some "a dup *" patterns for level 2
    if level >= 2:
        for a in ["2", "3", "4", "5"]:
            if len(pop) < pop_size * 0.5:
                pop.append([a, "dup", "*"])

    # Fill rest with random
    while len(pop) < pop_size:
        length = rng.randint(2, max(3, min(8, 3 + level)))
        pop.append(random_program(level, rng, length))

    return pop[:pop_size]


# =============================================================================
# Main training loop
# =============================================================================

def train(
    max_episodes: int = 10000,
    max_level: int = 9,
    pop_size: int = 50,
    max_steps_per_eval: int = 10000,
    advance_threshold: float = 0.8,
    eval_window: int = 50,
    seed: int = 42,
    log_interval: int = 50,
    verbose: bool = True,
):
    """
    Run evolutionary baseline training.

    Returns the HallOfFame with best programs found.
    """
    rng = random.Random(seed)
    runner = ServeRunner(max_steps=max_steps_per_eval)
    curriculum = Curriculum(
        start_level=0,
        advance_threshold=advance_threshold,
        eval_window=eval_window,
        seed=seed,
    )
    hof = HallOfFame()

    # Clamp max level to what we have tasks for
    max_level = min(max_level, MAX_LEVEL)

    # Initialize population for level 0
    population = seed_population(0, pop_size, rng)
    pop_scores = [0.0] * len(population)

    # Statistics
    t_start = time.monotonic()
    total_evals = 0
    total_correct = 0
    level_start_episode = 0
    best_score_ever = -999.0
    episodes_at_level: dict[int, int] = {}

    if verbose:
        print(f"╔══════════════════════════════════════════════════╗")
        print(f"║      Kore LLM Codegen — Evolutionary Baseline         ║")
        print(f"║      Pop: {pop_size}  Levels: 0-{max_level}  Episodes: {max_episodes:,}    ║")
        print(f"╚══════════════════════════════════════════════════╝")
        print()

    for episode in range(max_episodes):
        # Stop if we've reached max level and solved it
        if curriculum.level > max_level:
            if verbose:
                print(f"\n🎉 Reached level {max_level}! Training complete.")
            break

        # Sample a task
        task = curriculum.sample_task()

        # Evaluate entire population on this task
        batch_scores: list[float] = []
        batch_breakdowns: list[ScoreBreakdown] = []
        found_correct = False

        for i, tokens in enumerate(population):
            source = " ".join(tokens)
            result = runner.eval(source, max_steps=max_steps_per_eval)
            total_evals += 1

            s = score(result, task, tokens)
            batch_scores.append(s.total)
            batch_breakdowns.append(s)

            hof.update(task, tokens, s.total)

            if s.correct:
                found_correct = True
                total_correct += 1

            # Update running population scores (exponential moving average)
            pop_scores[i] = 0.7 * pop_scores[i] + 0.3 * s.total

        # Record success for curriculum
        curriculum.record(task, found_correct)

        # Track best
        best_idx = max(range(len(batch_scores)), key=lambda i: batch_scores[i])
        best_score = batch_scores[best_idx]
        best_source = " ".join(population[best_idx])
        best_score_ever = max(best_score_ever, best_score)

        # Evolve population
        population = mutate_population(
            population, pop_scores,
            level=curriculum.level,
            rng=rng,
            pop_size=pop_size,
            max_len=task.max_tokens,
        )
        pop_scores = [0.0] * len(population)  # Reset scores for new generation

        # Try to advance level
        old_level = curriculum.level
        if curriculum.maybe_advance():
            episodes_at_level[old_level] = episode - level_start_episode
            level_start_episode = episode
            # Re-seed population for new level
            population = seed_population(curriculum.level, pop_size, rng)
            pop_scores = [0.0] * len(population)

            if verbose:
                elapsed = time.monotonic() - t_start
                rate = curriculum.success_rate(old_level)
                print(f"\n  ★ LEVEL UP: {old_level} → {curriculum.level}  "
                      f"(rate={rate:.0%}, episode={episode}, "
                      f"t={elapsed:.1f}s, evals={total_evals:,})")

        # Logging
        if verbose and (episode + 1) % log_interval == 0:
            elapsed = time.monotonic() - t_start
            rate = curriculum.success_rate()
            eps_per_sec = (episode + 1) / elapsed
            evals_per_sec = total_evals / elapsed

            # Compact log line
            correct_mark = "✓" if found_correct else "·"
            print(f"  ep {episode+1:5d} | lvl {curriculum.level} | "
                  f"rate {rate:.0%} {correct_mark} | "
                  f"best {best_score:+.2f} | "
                  f"'{best_source[:40]}' | "
                  f"{eps_per_sec:.0f} ep/s | {evals_per_sec:.0f} ev/s | "
                  f"{elapsed:.1f}s")

    # ── Final report ──
    elapsed = time.monotonic() - t_start
    runner_stats = runner.stats()
    runner.close()

    if verbose:
        print(f"\n{'='*60}")
        print(f"  TRAINING COMPLETE")
        print(f"{'='*60}")
        print(f"  Episodes:        {episode+1:,}")
        print(f"  Total evals:     {total_evals:,}")
        print(f"  Elapsed:         {elapsed:.1f}s")
        print(f"  Episodes/sec:    {(episode+1)/elapsed:.0f}")
        print(f"  Evals/sec:       {total_evals/elapsed:.0f}")
        print(f"  Serve avg:       {runner_stats['avg_us']:.0f}µs/eval")
        print(f"  Max level:       {curriculum.max_level_reached}")
        print(f"  Total correct:   {total_correct:,}")
        print(f"  Solved tasks:    {hof.solved_count()}")

        # Per-level breakdown
        print(f"\n  Level breakdown:")
        for lvl in range(curriculum.max_level_reached + 1):
            rate = curriculum.success_rate(lvl)
            eps = episodes_at_level.get(lvl, episode + 1 - level_start_episode)
            status = "✓ solved" if rate >= advance_threshold else f"{rate:.0%}"
            print(f"    Level {lvl}: {status:>10s}  ({eps} episodes)")

        # Hall of Fame
        print(f"\n  Hall of Fame (best per task):")
        print(hof.summary())

    return hof, curriculum


# =============================================================================
# Entry point
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Kore LLM Codegen — Evolutionary Baseline Training ",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python baseline_train.py                      # Defaults: 10K episodes, levels 0-9
  python baseline_train.py --max-level 3        # Just arithmetic + stack + logic
  python baseline_train.py --episodes 50000     # Long run
  python baseline_train.py --pop-size 100       # Bigger population
  python baseline_train.py --seed 123           # Reproducible
        """,
    )
    parser.add_argument("--episodes", type=int, default=10000,
                        help="Max training episodes (default: 10000)")
    parser.add_argument("--max-level", type=int, default=9,
                        help="Highest curriculum level to attempt (default: 9)")
    parser.add_argument("--pop-size", type=int, default=50,
                        help="Population size (default: 50)")
    parser.add_argument("--max-steps", type=int, default=10000,
                        help="Gas limit per program (default: 10000)")
    parser.add_argument("--threshold", type=float, default=0.8,
                        help="Success rate to advance level (default: 0.8)")
    parser.add_argument("--eval-window", type=int, default=50,
                        help="Window for computing success rate (default: 50)")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed (default: 42)")
    parser.add_argument("--log-interval", type=int, default=50,
                        help="Log every N episodes (default: 50)")
    parser.add_argument("--quiet", action="store_true",
                        help="Suppress output")

    args = parser.parse_args()

    train(
        max_episodes=args.episodes,
        max_level=args.max_level,
        pop_size=args.pop_size,
        max_steps_per_eval=args.max_steps,
        advance_threshold=args.threshold,
        eval_window=args.eval_window,
        seed=args.seed,
        log_interval=args.log_interval,
        verbose=not args.quiet,
    )


if __name__ == "__main__":
    main()
