# Kore Living Network Experiments

Experiments in neural architecture search and program synthesis using Kore's 
unique stack-based autodiff system.

## Key Innovation

These experiments demonstrate **differentiable program synthesis** in Kore:
- The network doesn't just learn weights, it learns **which operations to compose**
- Uses the new `Scale2` gradient function that allows scalar tensors to receive gradients
- Tools are first-class: define reusable patterns like `make-param`, `sgd`, `mse`

## Experiments

### 1. NAS (Neural Architecture Search)
**File:** `nas.kore`

Learns which activation function to use:
- Model: `s * (a1*relu(x) + a2*sigmoid(5x) + a3*x)`
- Target: `y = 2*relu(x)`
- Result: `a1 → 2.29`, `a2 → 0.08`, `a3 → 0.13`, `s → 1.11`
- **The network discovers that ReLU is the right operation!**

### 2. Program Synthesis  
**File:** `program-synthesis.kore`

Learns both the operation AND its parameters:
- Model: `g1*relu(x+b) + g2*sigmoid(s*x) + g3*x`
- Target: `y = relu(x + 1)`  
- Result: `g1 → 0.91`, `g2 → 0.47`, `g3 → 0.02`, `b → 0.81`
- **The network discovers: use relu with bias ~1!**

## Reusable Tools

The `lib/ml.kore` library provides:
- `make-param`: Create tracked scalar parameter
- `sgd-step`: In-place SGD update with gradient
- `mse-loss`: Mean squared error
- `train-loop`: Training loop with step counter

## Key Kore Patterns

```kore
# Tool composition with memory for state
[ swap 1 list tensor-from-list requires-grad mem-set ] "make-param" def

# SGD using Scale2 for gradient-tracked learning rate
[ 
  "_lr" swap mem-set "_g" swap mem-set
  dup mem-get dup "_g" mem-get grad-get 
  "_lr" mem-get -1.0 mul tensor-scale
  tensor-add requires-grad mem-set 
] "sgd" def

# Forward pass as pure composition
[
  x-plus-b tensor-relu "g1" mem-get tensor-scale
  "x" mem-get "s" mem-get tensor-scale tensor-sigmoid "g2" mem-get tensor-scale
  "x" mem-get "g3" mem-get tensor-scale
  tensor-add tensor-add
] "forward" def
```

## What Makes This Special

1. **Stack-based autodiff**: Gradients flow through tool compositions naturally
2. **Scale2 innovation**: Scalar tensors can now receive gradients via `tensor-scale`
3. **Tool as data**: Forward passes are just tool compositions that can be manipulated
4. **Memory for state, stack for flow**: Clean separation of concerns

## Running

```bash
cd kore
./target/release/kore experiments/living-network/nas.kore
./target/release/kore experiments/living-network/program-synthesis.kore
```
