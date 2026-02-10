#!/usr/bin/env python3
"""
SFT Training — Stage 1: Teach the model Kore syntax basics.

Uses Unsloth for efficient QLoRA fine-tuning on curriculum hint data.
This bootstraps the model so GRPO has non-garbage starting generations.

Usage:
  python train_sft.py                              # Default config
  python train_sft.py --epochs 5 --lr 1e-4         # Custom
  python train_sft.py --max-level 3                 # Only basic levels
  python train_sft.py --resume checkpoints/sft-v1   # Resume from checkpoint
"""

from __future__ import annotations
import argparse
import os
import sys
import json
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def main():
    parser = argparse.ArgumentParser(
        description="Stage 1: Supervised Fine-Tuning on Kore curriculum",
    )
    # Model
    parser.add_argument("--model", type=str,
                        default="deepseek-ai/DeepSeek-R1-Distill-Qwen-14B",
                        help="Base model name or path")
    parser.add_argument("--resume", type=str, default=None,
                        help="Resume from adapter checkpoint")
    parser.add_argument("--adapter", type=str, default=None,
                        help="Path to existing LoRA adapter to continue training from")

    # Data
    parser.add_argument("--data", type=str, default=None,
                        help="Path to SFT JSONL dataset (auto-generates if not provided)")
    parser.add_argument("--max-level", type=int, default=9,
                        help="Max curriculum level for data generation")
    parser.add_argument("--sft-random-per-level", type=int, default=50,
                        help="Random samples per level for SFT data")

    # LoRA
    parser.add_argument("--lora-r", type=int, default=32,
                        help="LoRA rank (default: 32)")
    parser.add_argument("--lora-alpha", type=int, default=32,
                        help="LoRA alpha (default: 32)")
    parser.add_argument("--lora-dropout", type=float, default=0.05)

    # Training
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=2,
                        help="Per-device batch size")
    parser.add_argument("--grad-accum", type=int, default=4,
                        help="Gradient accumulation steps")
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--max-seq-len", type=int, default=2048)
    parser.add_argument("--warmup-ratio", type=float, default=0.1)
    parser.add_argument("--seed", type=int, default=42)

    # Output
    parser.add_argument("--output-dir", type=str, default="checkpoints/sft-v1")
    parser.add_argument("--logging-steps", type=int, default=10)

    # Hardware
    parser.add_argument("--gpu", type=int, default=0,
                        help="GPU index (default: 0 = first GPU)")

    args = parser.parse_args()

    # ── Set GPU ──
    os.environ["CUDA_VISIBLE_DEVICES"] = str(args.gpu)

    print("╔══════════════════════════════════════════════════╗")
    print("║  Stage 1 — Supervised Fine-Tuning       ║")
    print("╚══════════════════════════════════════════════════╝\n")

    # ── Step 1: Prepare data ──
    print("Step 1: Preparing dataset...")
    if args.data and os.path.exists(args.data):
        from data_prep import load_dataset
        sft_data = load_dataset(args.data)
        print(f"  Loaded {len(sft_data)} examples from {args.data}")
    else:
        from data_prep import generate_sft_dataset, save_dataset
        sft_data = generate_sft_dataset(
            max_level=args.max_level,
            samples_per_random_level=args.sft_random_per_level,
            seed=args.seed,
        )
        data_path = os.path.join("data", "sft_train.jsonl")
        save_dataset(sft_data, data_path)
        print(f"  Generated {len(sft_data)} examples")

    # Count per level
    level_counts = {}
    for item in sft_data:
        lvl = item.get("level", 0)
        level_counts[lvl] = level_counts.get(lvl, 0) + 1
    print(f"  Per level: {dict(sorted(level_counts.items()))}")

    # ── Step 2: Load model with Unsloth ──
    print(f"\nStep 2: Loading model...")

    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("\n  ⚠ Unsloth not installed. Install with:")
        print("    pip install 'unsloth[colab-new]'")
        print("  Or: pip install unsloth")
        sys.exit(1)

    if args.adapter:
        # Load from existing adapter (Unsloth reads adapter_config.json)
        load_name = args.adapter
        print(f"  Loading from adapter: {load_name}")
    else:
        load_name = args.model
        print(f"  Model: {load_name}")

    print(f"  LoRA: r={args.lora_r}, alpha={args.lora_alpha}")

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=load_name,
        max_seq_length=args.max_seq_len,
        dtype=None,  # auto-detect (bfloat16 on Ampere+)
        load_in_4bit=True,
    )

    print(f"  Model loaded: {model.config.num_hidden_layers} layers, "
          f"hidden={model.config.hidden_size}")
    print(f"  Tokenizer: vocab_size={tokenizer.vocab_size}")

    # ── Step 3: Add LoRA adapters ──
    print(f"\nStep 3: Configuring LoRA adapters...")

    if args.adapter:
        # Continue training existing LoRA — read config for passthrough
        import json as _json
        _acfg_path = os.path.join(args.adapter, "adapter_config.json")
        with open(_acfg_path) as _f:
            _acfg = _json.load(_f)
        sft_r = _acfg.get("r", args.lora_r)
        sft_alpha = _acfg.get("lora_alpha", args.lora_alpha)
        sft_dropout = _acfg.get("lora_dropout", args.lora_dropout)
        sft_targets = list(_acfg.get("target_modules", [
            "q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj",
        ]))
        print(f"  Continuing LoRA: r={sft_r}, alpha={sft_alpha}, dropout={sft_dropout}")
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
            use_gradient_checkpointing="unsloth",  # 30% less VRAM
            random_state=args.seed,
        )

    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total = sum(p.numel() for p in model.parameters())
    print(f"  Trainable: {trainable:,} / {total:,} ({100*trainable/total:.2f}%)")

    # ── Step 4: Format dataset ──
    print(f"\nStep 4: Formatting dataset for training...")

    from datasets import Dataset

    def format_for_training(example):
        """Apply chat template to messages."""
        messages = example["messages"]
        text = tokenizer.apply_chat_template(
            messages,
            tokenize=False,
            add_generation_prompt=False,
        )
        return {"text": text}

    # Remove any extra columns that aren't "messages" before building Dataset
    # (stdlib tasks may have "task_name" etc.)
    clean_data = []
    for item in sft_data:
        clean_data.append({"messages": item["messages"], "level": item.get("level", 19)})
    dataset = Dataset.from_list(clean_data)
    dataset = dataset.map(format_for_training, remove_columns=["level"])

    print(f"  Dataset size: {len(dataset)} examples")
    print(f"  Example (truncated): {dataset[0]['text'][:200]}...")

    # ── Step 5: Train ──
    print(f"\nStep 5: Training...")
    print(f"  Epochs: {args.epochs}")
    print(f"  Batch: {args.batch_size} × {args.grad_accum} = {args.batch_size * args.grad_accum}")
    print(f"  LR: {args.lr}")
    print(f"  Max seq len: {args.max_seq_len}")
    print(f"  Output: {args.output_dir}")

    from trl import SFTTrainer, SFTConfig

    training_args = SFTConfig(
        output_dir=args.output_dir,
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        learning_rate=args.lr,
        lr_scheduler_type="cosine",
        warmup_ratio=args.warmup_ratio,
        weight_decay=0.01,
        fp16=False,
        bf16=True,
        logging_steps=args.logging_steps,
        save_strategy="epoch",
        save_total_limit=3,
        optim="adamw_8bit",
        seed=args.seed,
        max_length=args.max_seq_len,
        packing=True,  # Pack short examples together for efficiency
        dataset_text_field="text",
        report_to="none",  # Disable wandb/tensorboard for now
    )

    trainer = SFTTrainer(
        model=model,
        processing_class=tokenizer,
        train_dataset=dataset,
        args=training_args,
    )

    print(f"\n  Starting training...\n")
    train_result = trainer.train(
        resume_from_checkpoint=args.resume,
    )

    # ── Step 6: Save ──
    print(f"\nStep 6: Saving adapter...")
    trainer.save_model(args.output_dir)
    tokenizer.save_pretrained(args.output_dir)

    # Save training metrics
    metrics = train_result.metrics
    metrics_path = os.path.join(args.output_dir, "train_metrics.json")
    with open(metrics_path, "w") as f:
        json.dump(metrics, f, indent=2)

    print(f"  Adapter saved to: {args.output_dir}")
    print(f"  Metrics: {metrics}")

    # ── Step 7: Quick sanity check ──
    print(f"\nStep 7: Sanity check (generating one completion)...")
    FastLanguageModel.for_inference(model)

    from data_prep import format_prompt
    from curriculum import TASK_POOLS

    task = TASK_POOLS[1][0]  # First arithmetic task
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

    response = tokenizer.decode(outputs[0][input_ids.shape[1]:], skip_special_tokens=True)
    print(f"  Task: {task.description}")
    print(f"  Expected: {task.expected}")
    print(f"  Model output: {response[:200]}")

    from data_prep import extract_kore_code
    kore = extract_kore_code(response)
    print(f"  Extracted Kore: {kore}")

    # Try to eval with korec
    try:
        from experiment_runner import ServeRunner
        runner = ServeRunner()
        result = runner.eval(kore)
        print(f"  korec result: ok={result.ok}, value={result.value}, "
              f"stage={result.stage}")
        runner.close()
    except Exception as e:
        print(f"  korec eval failed: {e}")

    print(f"\n{'='*60}")
    print(f"  SFT TRAINING COMPLETE")
    print(f"{'='*60}")
    print(f"  Model: {args.model}")
    print(f"  Adapter: {args.output_dir}")
    print(f"  Dataset: {len(sft_data)} examples, levels 0-{args.max_level}")
    print(f"  Next step: python train_grpo.py --adapter {args.output_dir}")


if __name__ == "__main__":
    main()
