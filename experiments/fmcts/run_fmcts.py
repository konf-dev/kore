#!/usr/bin/env python3
"""
Run FMCTS experiments.

Usage:
    python run_fmcts.py --task arithmetic --goal "[7]"
    python run_fmcts.py --task custom --goal "[42]" --simulations 1000
"""

import argparse
import json
import logging
import sys
from pathlib import Path

# Add parent to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from fmcts.core import FMCTS, MCTSConfig
from fmcts.executor import KoreExecutor


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


# Example tasks
EXAMPLE_TASKS = {
    "arithmetic": [
        {"goal": [7], "description": "Compute 3 + 4"},
        {"goal": [42], "description": "Compute 6 * 7"},
        {"goal": [100], "description": "Compute 10^2"},
    ],
    "stack": [
        {"goal": [5, 5], "description": "Duplicate 5"},
        {"goal": [2, 1], "description": "Swap 1 and 2"},
        {"goal": [3, 3, 3], "description": "Triple duplicate"},
    ],
    "combined": [
        {"goal": [14], "description": "Compute (3 + 4) * 2"},
        {"goal": [25], "description": "Square of 5"},
        {"goal": [10], "description": "Double 5"},
    ],
}


def parse_goal(goal_str: str):
    """Parse goal string like '[7]' or '[1, 2, 3]'."""
    try:
        return json.loads(goal_str)
    except json.JSONDecodeError:
        # Try simple int
        return [int(goal_str)]


def run_single_task(
    goal: list,
    config: MCTSConfig,
    executor: KoreExecutor,
    description: str = "",
) -> dict:
    """Run FMCTS on a single task."""
    logger.info(f"Task: {description or f'goal={goal}'}")
    
    fmcts = FMCTS(config=config, executor=executor)
    result = fmcts.search(goal=goal)
    
    status = "✓ SUCCESS" if result.success else "✗ FAILED"
    logger.info(f"{status}: '{result.program}' → {result.final_stack}")
    logger.info(f"  Nodes: {result.nodes_explored}, Simulations: {result.simulations}")
    
    return {
        "goal": goal,
        "description": description,
        "success": result.success,
        "program": result.program,
        "final_stack": result.final_stack,
        "nodes_explored": result.nodes_explored,
        "simulations": result.simulations,
    }


def run_task_suite(
    task_name: str,
    config: MCTSConfig,
    executor: KoreExecutor,
) -> list:
    """Run a suite of tasks."""
    tasks = EXAMPLE_TASKS.get(task_name, [])
    if not tasks:
        logger.error(f"Unknown task suite: {task_name}")
        return []
    
    results = []
    for task in tasks:
        result = run_single_task(
            goal=task["goal"],
            config=config,
            executor=executor,
            description=task.get("description", ""),
        )
        results.append(result)
    
    # Summary
    successes = sum(1 for r in results if r["success"])
    logger.info(f"\n=== Summary: {successes}/{len(results)} tasks solved ===")
    
    return results


def main():
    parser = argparse.ArgumentParser(description="Run FMCTS experiments")
    
    # Task selection
    parser.add_argument(
        "--task", 
        choices=["arithmetic", "stack", "combined", "custom"],
        default="arithmetic",
        help="Task suite to run"
    )
    parser.add_argument(
        "--goal",
        type=str,
        default=None,
        help="Custom goal (e.g., '[7]' or '[1, 2, 3]')"
    )
    
    # MCTS config
    parser.add_argument("--simulations", type=int, default=100, help="Simulations per move")
    parser.add_argument("--max-depth", type=int, default=20, help="Max program length")
    parser.add_argument("--exploration", type=float, default=1.414, help="UCB1 exploration constant")
    
    # Executor config
    parser.add_argument("--docker", action="store_true", help="Use Docker executor")
    parser.add_argument("--container", type=str, default="kore-runtime", help="Docker container name")
    parser.add_argument("--binary", type=str, default="kore-train", help="Local binary path")
    
    # Output
    parser.add_argument("--output", type=str, default=None, help="Save results to JSON file")
    parser.add_argument("--verbose", action="store_true", help="Verbose logging")
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Create config
    config = MCTSConfig(
        simulations_per_move=args.simulations,
        max_depth=args.max_depth,
        exploration_constant=args.exploration,
        use_docker=args.docker,
        container_name=args.container,
        kore_binary=args.binary,
        verbose=args.verbose,
    )
    
    # Create executor
    executor = KoreExecutor(
        use_docker=args.docker,
        container_name=args.container,
        kore_binary=args.binary,
    )
    
    # Check executor health
    if not executor.health_check():
        logger.warning("Executor health check failed! Make sure Kore runtime is available.")
        logger.info("Tip: Run 'cargo build --release -p kore-runtime' first")
    
    # Run tasks
    if args.task == "custom":
        if args.goal is None:
            parser.error("--goal is required for custom task")
        goal = parse_goal(args.goal)
        results = [run_single_task(goal, config, executor, f"Custom: {goal}")]
    else:
        results = run_task_suite(args.task, config, executor)
    
    # Save results
    if args.output:
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        logger.info(f"Results saved to {args.output}")
    
    # Print executor stats
    stats = executor.stats()
    logger.info(f"\nExecutor stats: {stats}")
    
    # Exit code
    successes = sum(1 for r in results if r["success"])
    sys.exit(0 if successes == len(results) else 1)


if __name__ == "__main__":
    main()
