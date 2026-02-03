#!/usr/bin/env python3
"""
Minimal Kore-RL Training Run

Test training with a small model to verify everything works.
For full training, use the main trainer.py with config.yaml.
"""

import os
import sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "training"))

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from docker_executor import KoreLocalExecutor
from trainer import generate_arithmetic_tasks, extract_program

# Change to kore root
os.chdir(os.path.dirname(os.path.abspath(__file__)) + "/../..")


def main():
    # Use a tiny model for testing
    # model_name = "Qwen/Qwen2-0.5B-Instruct"  # Very small, fast
    model_name = "HuggingFaceTB/SmolLM-135M-Instruct"  # Even smaller
    
    print(f"Loading model: {model_name}")
    
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        torch_dtype=torch.float16,
        device_map="auto",
    )
    model.eval()
    
    print(f"Model loaded on {model.device}")
    
    # Setup executor
    executor = KoreLocalExecutor(
        kore_binary="./target/release/kore-train",
        max_workers=8,
    )
    
    # Generate tasks
    tasks = generate_arithmetic_tasks(10)
    print(f"\nGenerated {len(tasks)} tasks")
    
    # Test generation + execution loop
    num_samples = 4
    temperature = 0.7
    max_new_tokens = 50
    
    print("\n" + "=" * 60)
    print("Testing Generation → Execution Loop")
    print("=" * 60)
    
    for task in tasks[:5]:
        prompt = task["prompt"]
        target = task["target"]
        
        print(f"\nPrompt: {prompt[:60]}...")
        print(f"Target: {target}")
        
        # Generate samples
        inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
        
        with torch.no_grad():
            outputs = model.generate(
                **inputs,
                max_new_tokens=max_new_tokens,
                num_return_sequences=num_samples,
                do_sample=True,
                temperature=temperature,
                pad_token_id=tokenizer.pad_token_id,
            )
        
        # Decode and extract programs
        programs = []
        for output in outputs:
            text = tokenizer.decode(output[inputs.input_ids.shape[1]:], skip_special_tokens=True)
            program = extract_program(text)
            programs.append(program)
        
        # Execute programs
        results = executor.execute_batch(programs, trace=False)
        
        # Compute rewards
        print("Samples:")
        for i, (program, result) in enumerate(zip(programs, results)):
            if not result.success:
                reward = -1.0
            elif len(result.final_stack) == 1 and result.final_stack[0] == target:
                reward = 1.0
            elif len(result.final_stack) == 1:
                got = result.final_stack[0]
                if isinstance(got, (int, float)):
                    reward = 0.5 / (1 + abs(got - target) * 0.1)
                else:
                    reward = -0.3
            else:
                reward = -0.5
            
            status = "✓" if reward > 0.9 else "✗"
            print(f"  [{i}] {status} {program!r:40} -> {result.final_stack} (r={reward:.2f})")
    
    executor.close()
    
    print("\n" + "=" * 60)
    print("Minimal training loop test complete!")
    print("=" * 60)
    print("\nTo run full training:")
    print("  cd experiments/kore-rl")
    print("  python training/trainer.py --config training/config.yaml")
    

if __name__ == "__main__":
    main()
