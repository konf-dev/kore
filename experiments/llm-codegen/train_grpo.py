#!/usr/bin/env python3
"""
GRPO Training — Stage 2: Reinforcement learning with korec serve verification.

The model generates Kore programs, korec serve compiles/runs/scores them,
and GRPO updates the policy toward better programs.

Uses Unsloth + TRL for efficient QLoRA training with group-relative rewards.

Prerequisites:
  - SFT adapter from train_sft.py (or start from base model)
  - korec binary built (cargo build --release)

Usage:
  python train_grpo.py                                      # Default
  python train_grpo.py --adapter checkpoints/sft-v1         # Start from SFT
  python train_grpo.py --max-level 3 --steps 500            # Quick test
  python train_grpo.py --num-generations 4 --batch-size 2   # VRAM-tight
"""

from __future__ import annotations
import argparse
import os
import sys
import json
import re
import time
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


# =============================================================================
# Reward functions — bridges to korec serve + scorer
# =============================================================================

# Global runner instance (initialized in main)
_runner = None
_format_reward_weight = 0.05


def get_runner():
    """Get or create the global ServeRunner."""
    global _runner
    if _runner is None:
        from experiment_runner import ServeRunner
        _runner = ServeRunner(max_steps=10000, timeout=2.0)
    return _runner


def kore_correctness_reward(prompts, completions, **kwargs) -> list[float]:
    """
    Primary reward: korec serve compilation + execution + correctness.

    Uses the dense scorer for multi-stage reward signal.
    Returns rewards in [-0.10, +1.70] range.

    TRL 0.24 conversational format:
      prompts: list of list[dict] — e.g. [[{"role":"user","content":"..."}], ...]
      completions: list of list[dict] — e.g. [[{"role":"assistant","content":"..."}], ...]
    """
    from data_prep import extract_kore_code, extract_task_from_prompt
    from scorer import score

    runner = get_runner()
    rewards = []

    for prompt_msgs, completion_msgs in zip(prompts, completions):
        try:
            # Extract text from conversational format
            prompt_text = prompt_msgs[-1]["content"] if isinstance(prompt_msgs, list) else str(prompt_msgs)
            completion_text = completion_msgs[-1]["content"] if isinstance(completion_msgs, list) else str(completion_msgs)

            # Extract Kore code from model output
            kore_code = extract_kore_code(completion_text)

            if not kore_code.strip():
                rewards.append(-0.10)
                continue

            # Reconstruct task from prompt for scoring
            task = extract_task_from_prompt(prompt_text)

            # Evaluate via korec serve
            result = runner.eval(kore_code, max_steps=10000)

            # Score with dense reward
            tokens = kore_code.strip().split()
            breakdown = score(result, task, tokens)
            rewards.append(breakdown.total)

        except Exception as e:
            # Any error → minimum reward
            rewards.append(-0.10)

    return rewards


def kore_format_reward(prompts, completions, **kwargs) -> list[float]:
    """
    Format reward: bonus for well-structured output.

    Encourages the model to maintain <think>...</think> reasoning format
    and produce clean Kore code afterwards.

    TRL 0.24 conversational format:
      completions: list of list[dict] — e.g. [[{"role":"assistant","content":"..."}], ...]
    """
    from data_prep import extract_kore_code

    rewards = []
    for completion_msgs in completions:
        # Extract text from conversational format
        completion = completion_msgs[-1]["content"] if isinstance(completion_msgs, list) else str(completion_msgs)

        r = 0.0

        # Has proper think block?
        if "<think>" in completion and "</think>" in completion:
            r += 0.03

            # Think block has content (not empty)?
            think_match = re.search(r'<think>\s*(.+?)\s*</think>', completion, re.DOTALL)
            if think_match and len(think_match.group(1).strip()) > 10:
                r += 0.02

        # Has non-empty Kore code after think block?
        kore = extract_kore_code(completion)
        if kore.strip():
            r += 0.02

            # Kore code compiles to something parseable?
            # (no token count penalty — execution steps handle efficiency)
            r += 0.01

        rewards.append(r)

    return rewards


# =============================================================================
# Dataset generation for GRPO
# =============================================================================

def make_grpo_dataset(max_level: int, n_prompts: int, seed: int):
    """Create the GRPO prompt dataset."""
    from data_prep import generate_grpo_prompts
    from datasets import Dataset

    prompts = generate_grpo_prompts(
        n_prompts=n_prompts,
        max_level=max_level,
        seed=seed,
    )

    # GRPO expects: {"prompt": "text"} where prompt is already formatted
    # We need to format as chat messages and apply template
    dataset_rows = []
    for item in prompts:
        dataset_rows.append({
            "prompt": item["prompt"],  # list of message dicts
            # Store metadata for the reward function to extract task info
            "_task_expected": str(item.get("_task_expected", "")),
            "_task_expected_type": item.get("_task_expected_type", "int"),
            "_task_level": item.get("_task_level", 0),
            "_task_description": item.get("_task_description", ""),
        })

    return Dataset.from_list(dataset_rows)


# =============================================================================
# Main training loop
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Stage 2: GRPO training with korec serve rewards",
    )
    # Model
    parser.add_argument("--model", type=str,
                        default="deepseek-ai/DeepSeek-R1-Distill-Qwen-14B",
                        help="Base model name or path")
    parser.add_argument("--adapter", type=str, default=None,
                        help="Path to SFT adapter to start from")

    # Data
    parser.add_argument("--max-level", type=int, default=5,
                        help="Max curriculum level for GRPO (default: 5, start easy)")
    parser.add_argument("--n-prompts", type=int, default=1000,
                        help="Number of prompts in dataset (default: 1000)")

    # LoRA
    parser.add_argument("--lora-r", type=int, default=32)
    parser.add_argument("--lora-alpha", type=int, default=32)
    parser.add_argument("--lora-dropout", type=float, default=0.0)

    # GRPO
    parser.add_argument("--num-generations", type=int, default=4,
                        help="Candidates per prompt (default: 4)")
    parser.add_argument("--max-new-tokens", type=int, default=256,
                        help="Max generation length (default: 256)")
    parser.add_argument("--steps", type=int, default=1000,
                        help="Max training steps (default: 1000)")
    parser.add_argument("--batch-size", type=int, default=1,
                        help="Per-device batch size (default: 1)")
    parser.add_argument("--grad-accum", type=int, default=4,
                        help="Gradient accumulation steps")
    parser.add_argument("--lr", type=float, default=5e-6,
                        help="Learning rate (default: 5e-6)")
    parser.add_argument("--beta", type=float, default=0.04,
                        help="KL penalty coefficient (default: 0.04)")
    parser.add_argument("--temperature", type=float, default=0.6,
                        help="Generation temperature (default: 0.6)")
    parser.add_argument("--max-seq-len", type=int, default=1024)
    parser.add_argument("--warmup-ratio", type=float, default=0.1)
    parser.add_argument("--seed", type=int, default=42)

    # Output
    parser.add_argument("--output-dir", type=str, default="checkpoints/grpo-v1")
    parser.add_argument("--logging-steps", type=int, default=5)
    parser.add_argument("--save-steps", type=int, default=100)

    # Hardware
    parser.add_argument("--gpu", type=int, default=0)

    args = parser.parse_args()

    # ── Set GPU ──
    os.environ["CUDA_VISIBLE_DEVICES"] = str(args.gpu)
    os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "expandable_segments:True"
    # Verify we got the right GPU
    import torch as _torch
    if _torch.cuda.is_available():
        _gname = _torch.cuda.get_device_name(0)
        _gmem = _torch.cuda.get_device_properties(0).total_memory / 1024**3
        print(f"  GPU: {_gname} ({_gmem:.1f} GB)")
    else:
        print("  ⚠ No CUDA device available!")
        sys.exit(1)

    print("╔══════════════════════════════════════════════════╗")
    print("║  Stage 2 — GRPO Training                ║")
    print("║  Reinforcement Learning with korec serve        ║")
    print("╚══════════════════════════════════════════════════╝\n")

    # ── Step 1: Load model ──
    print("Step 1: Loading model...")

    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("\n  ⚠ Unsloth not installed. Install with:")
        print("    pip install 'unsloth[colab-new]'")
        sys.exit(1)

    if args.adapter:
        # Unsloth natively handles adapter dirs: it reads adapter_config.json,
        # resolves the base model, loads both, and applies the adapter.
        load_name = args.adapter
        print(f"  Loading SFT adapter: {load_name}")
        print(f"  (Unsloth will auto-resolve the base model)")
    else:
        load_name = args.model
        print(f"  Model: {load_name}")
        print(f"  ⚠ No SFT adapter — starting from base model (may be slow to converge)")

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=load_name,
        max_seq_length=args.max_seq_len,
        dtype=None,
        load_in_4bit=True,
    )

    print(f"  Loaded: {model.config.num_hidden_layers} layers, "
          f"hidden={model.config.hidden_size}")

    # ── Step 2: Configure LoRA for GRPO ──
    print(f"\nStep 2: Configuring LoRA adapters for GRPO...")

    if args.adapter:
        # SFT adapter already loaded — continue training those same LoRA weights.
        # Unsloth's get_peft_model passes through if config matches exactly.
        # We read the SFT config to match it, ensuring passthrough works.
        import json as _json
        _acfg_path = os.path.join(args.adapter, "adapter_config.json")
        with open(_acfg_path) as _f:
            _acfg = _json.load(_f)
        sft_r = _acfg.get("r", args.lora_r)
        sft_alpha = _acfg.get("lora_alpha", args.lora_alpha)
        sft_dropout = _acfg.get("lora_dropout", 0.0)
        sft_targets = list(_acfg.get("target_modules", [
            "q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj",
        ]))
        print(f"  Continuing SFT LoRA: r={sft_r}, alpha={sft_alpha}, "
              f"dropout={sft_dropout}")
        model = FastLanguageModel.get_peft_model(
            model,
            r=sft_r,
            lora_alpha=sft_alpha,
            lora_dropout=sft_dropout,
            target_modules=sft_targets,
            bias="none",
            use_gradient_checkpointing="unsloth",
            random_state=args.seed,
        )
    else:
        model = FastLanguageModel.get_peft_model(
            model,
            r=args.lora_r,
            lora_alpha=args.lora_alpha,
            lora_dropout=args.lora_dropout,
            target_modules=[
                "q_proj", "k_proj", "v_proj", "o_proj",
                "gate_proj", "up_proj", "down_proj",
            ],
            bias="none",
            use_gradient_checkpointing="unsloth",
            random_state=args.seed,
        )

    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total = sum(p.numel() for p in model.parameters())
    print(f"  Trainable: {trainable:,} / {total:,} ({100*trainable/total:.2f}%)")

    # ── Step 3: Prepare dataset ──
    print(f"\nStep 3: Generating GRPO prompt dataset...")
    print(f"  Max level: {args.max_level}")
    print(f"  Prompts: {args.n_prompts}")

    dataset = make_grpo_dataset(
        max_level=args.max_level,
        n_prompts=args.n_prompts,
        seed=args.seed,
    )
    print(f"  Dataset: {len(dataset)} prompts")

    # ── Step 4: Verify korec serve ──
    print(f"\nStep 4: Verifying korec serve...")
    runner = get_runner()
    test = runner.eval("3 5 +")
    assert test.ok and test.value == 8, f"korec serve broken: {test}"
    print(f"  korec serve OK: 3 5 + = {test.value} ({test.elapsed_us}µs)")

    # ── Step 5: Configure GRPO trainer ──
    print(f"\nStep 5: Configuring GRPO trainer...")
    print(f"  Generations per prompt: {args.num_generations}")
    print(f"  Max new tokens: {args.max_new_tokens}")
    print(f"  Temperature: {args.temperature}")
    print(f"  Steps: {args.steps}")
    print(f"  LR: {args.lr}")
    print(f"  Beta (KL): {args.beta}")

    from trl import GRPOTrainer, GRPOConfig

    grpo_config = GRPOConfig(
        output_dir=args.output_dir,
        max_steps=args.steps,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        learning_rate=args.lr,
        lr_scheduler_type="cosine",
        warmup_ratio=args.warmup_ratio,
        weight_decay=0.01,
        bf16=True,
        fp16=False,
        logging_steps=args.logging_steps,
        save_steps=args.save_steps,
        save_total_limit=5,
        optim="adamw_8bit",
        seed=args.seed,
        report_to="none",

        # GRPO-specific
        num_generations=args.num_generations,
        max_completion_length=args.max_new_tokens,
        max_prompt_length=args.max_seq_len - args.max_new_tokens,
        beta=args.beta,
        temperature=args.temperature,
        top_p=0.95,

        # Use vLLM for fast generation if available
        # use_vllm=True,  # Enable when vLLM is installed
    )

    trainer = GRPOTrainer(
        model=model,
        processing_class=tokenizer,
        train_dataset=dataset,
        reward_funcs=[
            kore_correctness_reward,
            kore_format_reward,
        ],
        args=grpo_config,
    )

    # ── Step 6: Train ──
    print(f"\n{'='*60}")
    print(f"  Starting GRPO training...")
    print(f"  Reward functions: kore_correctness, kore_format")
    print(f"  korec serve throughput: {runner.stats().get('throughput', 0):.0f} prog/sec")
    print(f"{'='*60}\n")

    t_start = time.monotonic()

    try:
        train_result = trainer.train()
    except KeyboardInterrupt:
        print("\n\n  ⚠ Training interrupted by user. Saving checkpoint...")
        trainer.save_model(os.path.join(args.output_dir, "interrupted"))
        tokenizer.save_pretrained(os.path.join(args.output_dir, "interrupted"))
        print(f"  Saved to {args.output_dir}/interrupted")
        sys.exit(0)

    elapsed = time.monotonic() - t_start

    # ── Step 7: Save final adapter ──
    print(f"\nStep 7: Saving final adapter...")
    trainer.save_model(args.output_dir)
    tokenizer.save_pretrained(args.output_dir)

    # Save metrics
    metrics = train_result.metrics
    metrics["training_time_seconds"] = elapsed
    metrics["korec_stats"] = runner.stats()
    metrics_path = os.path.join(args.output_dir, "grpo_metrics.json")
    with open(metrics_path, "w") as f:
        json.dump(metrics, f, indent=2)

    print(f"  Adapter: {args.output_dir}")
    print(f"  Time: {elapsed:.0f}s ({elapsed/60:.1f} min)")

    # ── Step 8: Quick eval ──
    print(f"\nStep 8: Quick evaluation...")
    FastLanguageModel.for_inference(model)

    from data_prep import format_prompt, extract_kore_code
    from curriculum import TASK_POOLS
    from scorer import score

    correct_count = 0
    eval_count = 0

    for level in range(min(args.max_level + 1, 6)):
        pool = TASK_POOLS.get(level, [])[:5]  # First 5 tasks per level
        level_correct = 0

        for task in pool:
            prompt = format_prompt(task)
            messages = [{"role": "user", "content": prompt}]

            input_ids = tokenizer.apply_chat_template(
                messages,
                tokenize=True,
                add_generation_prompt=True,
                return_tensors="pt",
            ).to(model.device)

            outputs = model.generate(
                input_ids,
                max_new_tokens=256,
                temperature=0.6,
                top_p=0.95,
                do_sample=True,
            )

            response = tokenizer.decode(
                outputs[0][input_ids.shape[1]:],
                skip_special_tokens=True,
            )
            kore = extract_kore_code(response)
            result = runner.eval(kore)
            tokens = kore.strip().split()
            s = score(result, task, tokens)

            if s.correct:
                level_correct += 1
                correct_count += 1
            eval_count += 1

        print(f"  Level {level}: {level_correct}/{len(pool)} correct")

    runner.close()

    print(f"\n{'='*60}")
    print(f"  GRPO TRAINING COMPLETE")
    print(f"{'='*60}")
    print(f"  Model: {args.model}")
    print(f"  Adapter: {args.output_dir}")
    print(f"  Training: {args.steps} steps in {elapsed:.0f}s")
    print(f"  Quick eval: {correct_count}/{eval_count} correct")
    print(f"  Korec evals: {runner.stats().get('eval_count', 0):,}")
    print(f"  Next step: python eval_model.py --adapter {args.output_dir}")


if __name__ == "__main__":
    main()
