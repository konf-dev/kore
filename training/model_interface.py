"""
Model Interface for Kore RL Training

Supports multiple backends:
- HuggingFace Transformers
- vLLM (faster inference)
- API-based models
"""

import torch
from typing import List, Dict, Optional, Any
from dataclasses import dataclass
from abc import ABC, abstractmethod
import os


@dataclass
class GenerationConfig:
    """Generation configuration."""
    max_new_tokens: int = 256
    temperature: float = 0.7
    top_p: float = 0.95
    top_k: int = 50
    do_sample: bool = True
    num_return_sequences: int = 1
    stop_sequences: List[str] = None
    
    def __post_init__(self):
        if self.stop_sequences is None:
            self.stop_sequences = ["\n\n", "```", "Task:", "Your program:"]


class BaseModel(ABC):
    """Abstract base class for language models."""
    
    @abstractmethod
    def generate(
        self, 
        prompt: str, 
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> str:
        """Generate a single response."""
        pass
    
    @abstractmethod
    def generate_batch(
        self,
        prompts: List[str],
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> List[str]:
        """Generate responses for multiple prompts."""
        pass
    
    @abstractmethod
    def get_log_probs(
        self,
        prompt: str,
        completion: str
    ) -> float:
        """Get log probability of completion given prompt."""
        pass


class HFModel(BaseModel):
    """HuggingFace Transformers model."""
    
    def __init__(
        self,
        model_name: str = "Qwen/Qwen2.5-Coder-7B-Instruct",
        device: str = "cuda",
        dtype: str = "bfloat16",
        quantization: str = None,
        trust_remote_code: bool = True
    ):
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
        
        self.model_name = model_name
        self.device = device
        
        # Determine dtype
        dtype_map = {
            "float16": torch.float16,
            "bfloat16": torch.bfloat16,
            "float32": torch.float32,
        }
        torch_dtype = dtype_map.get(dtype, torch.bfloat16)
        
        # Quantization config
        bnb_config = None
        if quantization == "4bit":
            bnb_config = BitsAndBytesConfig(
                load_in_4bit=True,
                bnb_4bit_compute_dtype=torch_dtype,
                bnb_4bit_use_double_quant=True,
                bnb_4bit_quant_type="nf4",
            )
        elif quantization == "8bit":
            bnb_config = BitsAndBytesConfig(load_in_8bit=True)
        
        print(f"Loading model: {model_name}")
        print(f"  Device: {device}, Dtype: {dtype}, Quantization: {quantization}")
        
        # Load tokenizer
        self.tokenizer = AutoTokenizer.from_pretrained(
            model_name,
            trust_remote_code=trust_remote_code
        )
        if self.tokenizer.pad_token is None:
            self.tokenizer.pad_token = self.tokenizer.eos_token
        
        # Load model
        self.model = AutoModelForCausalLM.from_pretrained(
            model_name,
            torch_dtype=torch_dtype,
            device_map="auto" if device == "cuda" else None,
            quantization_config=bnb_config,
            trust_remote_code=trust_remote_code,
        )
        self.model.eval()
        
        print(f"  Model loaded: {self.model.device}")
    
    def _format_prompt(self, prompt: str, system_prompt: str = None) -> str:
        """Format prompt for the model's chat template."""
        messages = []
        
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        
        messages.append({"role": "user", "content": prompt})
        
        # Use model's chat template if available
        if hasattr(self.tokenizer, 'apply_chat_template'):
            return self.tokenizer.apply_chat_template(
                messages, 
                tokenize=False, 
                add_generation_prompt=True
            )
        else:
            # Fallback
            formatted = ""
            if system_prompt:
                formatted += f"System: {system_prompt}\n\n"
            formatted += f"User: {prompt}\n\nAssistant:"
            return formatted
    
    def generate(
        self,
        prompt: str,
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> str:
        """Generate a single response."""
        
        if config is None:
            config = GenerationConfig()
        
        formatted = self._format_prompt(prompt, system_prompt)
        
        inputs = self.tokenizer(
            formatted,
            return_tensors="pt",
            truncation=True,
            max_length=4096
        ).to(self.model.device)
        
        with torch.no_grad():
            outputs = self.model.generate(
                **inputs,
                max_new_tokens=config.max_new_tokens,
                temperature=config.temperature if config.do_sample else 1.0,
                top_p=config.top_p if config.do_sample else 1.0,
                top_k=config.top_k if config.do_sample else 0,
                do_sample=config.do_sample,
                pad_token_id=self.tokenizer.pad_token_id,
                eos_token_id=self.tokenizer.eos_token_id,
            )
        
        # Decode only the new tokens
        response = self.tokenizer.decode(
            outputs[0][inputs['input_ids'].shape[1]:],
            skip_special_tokens=True
        )
        
        # Apply stop sequences
        for stop in config.stop_sequences or []:
            if stop in response:
                response = response.split(stop)[0]
        
        return response.strip()
    
    def generate_batch(
        self,
        prompts: List[str],
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> List[str]:
        """Generate responses for multiple prompts."""
        
        # For now, sequential generation
        # TODO: Implement proper batched generation
        return [
            self.generate(p, config, system_prompt)
            for p in prompts
        ]
    
    def get_log_probs(self, prompt: str, completion: str) -> float:
        """Get log probability of completion given prompt."""
        
        full_text = prompt + completion
        inputs = self.tokenizer(full_text, return_tensors="pt").to(self.model.device)
        prompt_len = len(self.tokenizer(prompt)['input_ids'])
        
        with torch.no_grad():
            outputs = self.model(**inputs)
            logits = outputs.logits
        
        # Get log probs for completion tokens
        log_probs = torch.nn.functional.log_softmax(logits, dim=-1)
        
        total_log_prob = 0.0
        for i in range(prompt_len - 1, inputs['input_ids'].shape[1] - 1):
            token_id = inputs['input_ids'][0, i + 1]
            total_log_prob += log_probs[0, i, token_id].item()
        
        return total_log_prob


class VLLMModel(BaseModel):
    """vLLM model for faster inference."""
    
    def __init__(
        self,
        model_name: str = "Qwen/Qwen2.5-Coder-7B-Instruct",
        tensor_parallel_size: int = 1,
        dtype: str = "bfloat16",
        trust_remote_code: bool = True
    ):
        from vllm import LLM, SamplingParams
        from transformers import AutoTokenizer
        
        self.model_name = model_name
        self.SamplingParams = SamplingParams
        
        print(f"Loading vLLM model: {model_name}")
        
        self.tokenizer = AutoTokenizer.from_pretrained(
            model_name,
            trust_remote_code=trust_remote_code
        )
        
        self.model = LLM(
            model=model_name,
            tensor_parallel_size=tensor_parallel_size,
            dtype=dtype,
            trust_remote_code=trust_remote_code,
        )
        
        print("  vLLM model loaded")
    
    def _format_prompt(self, prompt: str, system_prompt: str = None) -> str:
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        messages.append({"role": "user", "content": prompt})
        
        if hasattr(self.tokenizer, 'apply_chat_template'):
            return self.tokenizer.apply_chat_template(
                messages, tokenize=False, add_generation_prompt=True
            )
        return prompt
    
    def generate(
        self,
        prompt: str,
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> str:
        results = self.generate_batch([prompt], config, system_prompt)
        return results[0]
    
    def generate_batch(
        self,
        prompts: List[str],
        config: GenerationConfig = None,
        system_prompt: str = None
    ) -> List[str]:
        
        if config is None:
            config = GenerationConfig()
        
        formatted_prompts = [
            self._format_prompt(p, system_prompt) for p in prompts
        ]
        
        sampling_params = self.SamplingParams(
            max_tokens=config.max_new_tokens,
            temperature=config.temperature,
            top_p=config.top_p,
            top_k=config.top_k,
            stop=config.stop_sequences,
        )
        
        outputs = self.model.generate(formatted_prompts, sampling_params)
        
        return [output.outputs[0].text.strip() for output in outputs]
    
    def get_log_probs(self, prompt: str, completion: str) -> float:
        # vLLM can return log probs with prompt_logprobs parameter
        # For now, return placeholder
        return 0.0


def load_model(config: Dict) -> BaseModel:
    """Load model based on configuration."""
    
    model_config = config.get('model', {})
    model_name = model_config.get('name', 'Qwen/Qwen2.5-Coder-7B-Instruct')
    
    # Try vLLM first if available
    use_vllm = model_config.get('use_vllm', True)
    
    if use_vllm:
        try:
            import vllm
            return VLLMModel(
                model_name=model_name,
                tensor_parallel_size=model_config.get('tensor_parallel_size', 1),
                dtype=model_config.get('dtype', 'bfloat16'),
                trust_remote_code=model_config.get('trust_remote_code', True),
            )
        except ImportError:
            print("vLLM not available, falling back to HuggingFace")
    
    return HFModel(
        model_name=model_name,
        device=model_config.get('device', 'cuda'),
        dtype=model_config.get('dtype', 'bfloat16'),
        quantization=model_config.get('quantization'),
        trust_remote_code=model_config.get('trust_remote_code', True),
    )


# =============================================================================
# Test
# =============================================================================

if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="Qwen/Qwen2.5-Coder-0.5B-Instruct")
    parser.add_argument("--quantization", default=None)
    args = parser.parse_args()
    
    print("Testing Model Interface...")
    
    model = HFModel(
        model_name=args.model,
        quantization=args.quantization
    )
    
    # Test generation
    config = GenerationConfig(max_new_tokens=50, temperature=0.7)
    response = model.generate(
        "Write a Kore program that computes the square of a number on the stack:",
        config=config,
        system_prompt="You are a Kore programming expert."
    )
    
    print(f"\nResponse: {response}")
    print("\n✓ Model test passed!")
