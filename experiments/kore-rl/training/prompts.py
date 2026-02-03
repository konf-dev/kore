"""
Kore-RL System Prompts and Task Templates
==========================================

Contains:
1. Kore language documentation for system prompt
2. Philosophy and design principles
3. Task templates for each curriculum phase
4. Few-shot examples
"""

# =============================================================================
# KORE DOCUMENTATION (System Prompt)
# =============================================================================

KORE_SYSTEM_PROMPT = """You are a Kore programming expert. Kore is a minimal, stack-based programming language designed for clarity and composability.

## Kore Philosophy

Kore follows these principles:
1. **Simplicity**: Everything is built from basic operations on a stack
2. **Composability**: Complex behavior emerges from combining simple primitives
3. **Explicitness**: No hidden state, the stack shows everything
4. **Determinism**: Same input always produces same output

## How Kore Works

Kore uses a **stack** - a last-in, first-out data structure. Values are pushed onto the stack, and operations consume values from the top and push results back.

```
3 4 add    # Push 3, push 4, pop both, push 7
           # Stack: [7]
```

## Core Operations

### Stack Manipulation
- `dup`    : Duplicate top element        [a] → [a a]
- `drop`   : Remove top element           [a] → []
- `swap`   : Swap top two elements        [a b] → [b a]
- `over`   : Copy second element to top   [a b] → [a b a]
- `rot`    : Rotate top three             [a b c] → [b c a]

### Arithmetic
- `add`    : Addition                     [a b] → [a+b]
- `sub`    : Subtraction                  [a b] → [a-b]
- `mul`    : Multiplication               [a b] → [a*b]
- `div`    : Integer division             [a b] → [a/b]
- `mod`    : Modulo                       [a b] → [a%b]
- `neg`    : Negate                       [a] → [-a]
- `abs`    : Absolute value               [a] → [|a|]

### Comparison (return true/false)
- `eq`     : Equal                        [a b] → [a==b]
- `neq`    : Not equal                    [a b] → [a!=b]
- `lt`     : Less than                    [a b] → [a<b]
- `gt`     : Greater than                 [a b] → [a>b]
- `le`     : Less or equal                [a b] → [a<=b]
- `ge`     : Greater or equal             [a b] → [a>=b]

### Logic
- `and`    : Logical AND                  [a b] → [a&&b]
- `or`     : Logical OR                   [a b] → [a||b]
- `not`    : Logical NOT                  [a] → [!a]

### Control Flow
- `if`     : Conditional                  [cond then else] → [result]
           : If cond is true, executes 'then', otherwise 'else'
- `times`  : Loop N times                 [n body] → [...]
           : Executes body n times
- `while`  : While loop                   [cond body] → [...]
           : Executes body while cond is true

### Quotations (Code as Data)
- `[ ... ]` : Create a quotation (unevaluated code)
- `call`    : Execute a quotation
- `dip`     : Execute quotation under top element

### Lists
- `[ 1 2 3 ]` : Create a list
- `len`       : Get list length           [list] → [n]
- `first`     : Get first element         [list] → [elem]
- `rest`      : Get all but first         [list] → [list']
- `cons`      : Prepend element           [elem list] → [list']
- `concat`    : Concatenate lists         [list1 list2] → [list']
- `nth`       : Get nth element           [list n] → [elem]
- `map`       : Apply function to each    [list fn] → [list']
- `fold`      : Reduce list               [list init fn] → [result]

## Examples

### Basic Arithmetic
```kore
3 4 add           # Result: 7
10 3 sub          # Result: 7
6 7 mul           # Result: 42
```

### Stack Operations
```kore
5 dup mul         # 5*5 = 25
3 4 swap sub      # 4-3 = 1
1 2 over add add  # 1+2+1 = 4
```

### Conditionals
```kore
# Max of two numbers
5 3 dup2 lt [ swap ] [ ] if drop
# Explanation: Compare, if first < second, swap, then drop the smaller
```

### Loops
```kore
# Sum 1 to 5: put accumulator 0, then add 1,2,3,4,5
0 5 [ 1 add dup rot add swap ] times drop
# Result: 15
```

### Quotations
```kore
[ 2 mul ] call     # Doubles top of stack
3 [ 2 mul ] call   # 3 * 2 = 6
```

## Writing Kore Programs

1. Think about what values you need on the stack
2. Push values in the order operations need them
3. Apply operations left to right
4. The result remains on the stack

Remember: Kore reads left to right. Each word either pushes a value or performs an operation on the stack.
"""


# =============================================================================
# TASK TEMPLATES BY CURRICULUM PHASE
# =============================================================================

PHASE_1_ARITHMETIC = """## Phase 1: Arithmetic

You're learning basic Kore arithmetic. The stack starts empty.

Available operations: add, sub, mul, div, mod, neg, abs

### Task
{task_description}

### Expected Result
The final stack should contain exactly: {expected}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""

PHASE_2_STACK = """## Phase 2: Stack Manipulation

You're learning Kore stack operations combined with arithmetic.

Available operations: dup, drop, swap, over, rot, add, sub, mul, div, mod

### Task
{task_description}

### Expected Result
The final stack should contain exactly: {expected}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""

PHASE_3_CONTROL = """## Phase 3: Control Flow

You're learning Kore conditionals.

Available operations: all arithmetic, all stack ops, comparisons (eq, lt, gt, le, ge), if

### Task
{task_description}

### Expected Result
The final stack should contain exactly: {expected}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""

PHASE_4_LOOPS = """## Phase 4: Loops

You're learning Kore loops.

Available operations: all previous, times, while, quotations [ ]

### Task
{task_description}

### Expected Result
The final stack should contain exactly: {expected}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""

PHASE_5_LISTS = """## Phase 5: Lists

You're learning Kore list operations.

Available operations: all previous, list literals, len, first, rest, cons, concat, nth, map, fold

### Task
{task_description}

### Expected Result
The final stack should contain exactly: {expected}

### Your Program
Write a Kore program (just the code, no explanation):
```kore
"""

PHASE_TEMPLATES = {
    1: PHASE_1_ARITHMETIC,
    2: PHASE_2_STACK,
    3: PHASE_3_CONTROL,
    4: PHASE_4_LOOPS,
    5: PHASE_5_LISTS,
}


# =============================================================================
# FEW-SHOT EXAMPLES
# =============================================================================

FEW_SHOT_EXAMPLES = {
    1: [  # Arithmetic
        {
            "task": "Compute 7 + 8",
            "expected": "[15]",
            "solution": "7 8 add",
        },
        {
            "task": "Compute 20 - 13",
            "expected": "[7]",
            "solution": "20 13 sub",
        },
        {
            "task": "Compute 6 * 9",
            "expected": "[54]",
            "solution": "6 9 mul",
        },
        {
            "task": "Compute (3 + 4) * 2",
            "expected": "[14]",
            "solution": "3 4 add 2 mul",
        },
    ],
    2: [  # Stack
        {
            "task": "Square the number 7 (compute 7 * 7)",
            "expected": "[49]",
            "solution": "7 dup mul",
        },
        {
            "task": "Given 5 and 3 on stack, compute 3 - 5",
            "expected": "[-2]",
            "solution": "5 3 swap sub",
        },
        {
            "task": "Compute a*a + b where a=3, b=5",
            "expected": "[14]",
            "solution": "3 dup mul 5 add",
        },
    ],
    3: [  # Control
        {
            "task": "Return the maximum of 8 and 5",
            "expected": "[8]",
            "solution": "8 5 dup2 lt [ drop ] [ swap drop ] if",
        },
        {
            "task": "Return 1 if 10 > 5, else 0",
            "expected": "[1]",
            "solution": "10 5 gt [ 1 ] [ 0 ] if",
        },
    ],
    4: [  # Loops
        {
            "task": "Compute 5! (factorial of 5)",
            "expected": "[120]",
            "solution": "1 5 [ dup rot mul swap 1 sub dup 0 eq ] [ drop ] while drop",
        },
        {
            "task": "Sum numbers 1 to 4",
            "expected": "[10]",
            "solution": "0 4 [ over add swap 1 add swap ] times drop",
        },
    ],
    5: [  # Lists
        {
            "task": "Get the length of list [1, 2, 3, 4, 5]",
            "expected": "[5]",
            "solution": "[ 1 2 3 4 5 ] len",
        },
        {
            "task": "Sum all elements in list [1, 2, 3]",
            "expected": "[6]",
            "solution": "[ 1 2 3 ] 0 [ add ] fold",
        },
    ],
}


# =============================================================================
# TASK GENERATOR
# =============================================================================

def format_prompt(
    task_description: str,
    expected: str,
    phase: int,
    include_examples: bool = True,
    num_examples: int = 2,
) -> str:
    """
    Format a complete prompt for the LLM.
    
    Args:
        task_description: What the task asks for
        expected: Expected final stack
        phase: Curriculum phase (1-5)
        include_examples: Whether to include few-shot examples
        num_examples: How many examples to include
    
    Returns:
        Complete prompt string
    """
    template = PHASE_TEMPLATES.get(phase, PHASE_1_ARITHMETIC)
    
    prompt_parts = []
    
    # Add few-shot examples
    if include_examples:
        examples = FEW_SHOT_EXAMPLES.get(phase, [])[:num_examples]
        if examples:
            prompt_parts.append("## Examples\n")
            for ex in examples:
                prompt_parts.append(f"Task: {ex['task']}")
                prompt_parts.append(f"Expected: {ex['expected']}")
                prompt_parts.append(f"```kore\n{ex['solution']}\n```\n")
    
    # Add task
    prompt_parts.append(template.format(
        task_description=task_description,
        expected=expected,
    ))
    
    return "\n".join(prompt_parts)


def create_chat_messages(
    task_description: str,
    expected: str,
    phase: int = 1,
    include_system: bool = True,
    include_examples: bool = True,
) -> list:
    """
    Create chat messages for the LLM.
    
    Returns list of {"role": ..., "content": ...} messages.
    """
    messages = []
    
    if include_system:
        messages.append({
            "role": "system",
            "content": KORE_SYSTEM_PROMPT,
        })
    
    user_prompt = format_prompt(
        task_description=task_description,
        expected=expected,
        phase=phase,
        include_examples=include_examples,
    )
    
    messages.append({
        "role": "user",
        "content": user_prompt,
    })
    
    return messages


# =============================================================================
# ENHANCED TASK GENERATION
# =============================================================================

import random

def generate_task(phase: int) -> dict:
    """
    Generate a single task for the given phase.
    
    Returns:
        {
            "description": str,  # Human-readable task
            "expected": Any,     # Expected result (for reward)
            "expected_str": str, # String representation for prompt
            "phase": int,
            "difficulty": int,
        }
    """
    if phase == 1:
        return _generate_arithmetic_task()
    elif phase == 2:
        return _generate_stack_task()
    elif phase == 3:
        return _generate_control_task()
    elif phase == 4:
        return _generate_loop_task()
    elif phase == 5:
        return _generate_list_task()
    else:
        return _generate_arithmetic_task()


def _generate_arithmetic_task() -> dict:
    """Generate Phase 1 arithmetic task"""
    task_type = random.choice(["simple", "compound", "multi"])
    
    if task_type == "simple":
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        op, op_name = random.choice([
            (lambda x, y: x + y, "+"),
            (lambda x, y: x - y, "-"),
            (lambda x, y: x * y, "*"),
        ])
        result = op(a, b)
        description = f"Compute {a} {op_name} {b}"
        difficulty = 1
    
    elif task_type == "compound":
        a = random.randint(1, 20)
        b = random.randint(1, 20)
        c = random.randint(1, 10)
        pattern = random.choice([
            (f"({a} + {b}) * {c}", (a + b) * c),
            (f"({a} * {b}) + {c}", (a * b) + c),
            (f"{a} * ({b} + {c})", a * (b + c)),
            (f"({a} - {b}) * {c}", (a - b) * c),
        ])
        description = f"Compute {pattern[0]}"
        result = pattern[1]
        difficulty = 2
    
    else:  # multi
        nums = [random.randint(1, 20) for _ in range(random.randint(3, 5))]
        result = sum(nums)
        description = f"Compute the sum: {' + '.join(map(str, nums))}"
        difficulty = 2
    
    return {
        "description": description,
        "expected": result,
        "expected_str": f"[{result}]",
        "phase": 1,
        "difficulty": difficulty,
    }


def _generate_stack_task() -> dict:
    """Generate Phase 2 stack task"""
    task_type = random.choice(["square", "swap_sub", "complex"])
    
    if task_type == "square":
        n = random.randint(2, 15)
        description = f"Square the number {n} (compute {n}²)"
        result = n * n
        difficulty = 2
    
    elif task_type == "swap_sub":
        a = random.randint(1, 30)
        b = random.randint(1, 30)
        description = f"Push {a} and {b}, then compute {b} - {a} using swap"
        result = b - a
        difficulty = 2
    
    else:  # complex
        a = random.randint(2, 10)
        b = random.randint(1, 10)
        description = f"Compute {a}² + {b} (square of {a} plus {b})"
        result = a * a + b
        difficulty = 3
    
    return {
        "description": description,
        "expected": result,
        "expected_str": f"[{result}]",
        "phase": 2,
        "difficulty": difficulty,
    }


def _generate_control_task() -> dict:
    """Generate Phase 3 control flow task"""
    task_type = random.choice(["max", "min", "abs", "sign", "clamp"])
    
    if task_type == "max":
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        description = f"Compute max({a}, {b})"
        result = max(a, b)
        difficulty = 3
    
    elif task_type == "min":
        a = random.randint(1, 50)
        b = random.randint(1, 50)
        description = f"Compute min({a}, {b})"
        result = min(a, b)
        difficulty = 3
    
    elif task_type == "abs":
        n = random.randint(-30, 30)
        description = f"Compute the absolute value of {n}"
        result = abs(n)
        difficulty = 3
    
    elif task_type == "sign":
        n = random.randint(-30, 30)
        description = f"Return the sign of {n} (-1, 0, or 1)"
        result = 0 if n == 0 else (1 if n > 0 else -1)
        difficulty = 4
    
    else:  # clamp
        n = random.randint(-20, 40)
        lo, hi = 0, 20
        description = f"Clamp {n} to range [{lo}, {hi}]"
        result = max(lo, min(hi, n))
        difficulty = 4
    
    return {
        "description": description,
        "expected": result,
        "expected_str": f"[{result}]",
        "phase": 3,
        "difficulty": difficulty,
    }


def _generate_loop_task() -> dict:
    """Generate Phase 4 loop task"""
    task_type = random.choice(["sum_n", "factorial", "power", "fib"])
    
    if task_type == "sum_n":
        n = random.randint(5, 15)
        description = f"Compute the sum 1 + 2 + ... + {n}"
        result = n * (n + 1) // 2
        difficulty = 4
    
    elif task_type == "factorial":
        n = random.randint(3, 7)
        result = 1
        for i in range(1, n + 1):
            result *= i
        description = f"Compute {n}! (factorial)"
        difficulty = 5
    
    elif task_type == "power":
        base = random.randint(2, 5)
        exp = random.randint(2, 5)
        result = base ** exp
        description = f"Compute {base}^{exp} (power)"
        difficulty = 4
    
    else:  # fib
        n = random.randint(5, 12)
        fibs = [0, 1]
        for _ in range(n - 1):
            fibs.append(fibs[-1] + fibs[-2])
        result = fibs[n]
        description = f"Compute the {n}th Fibonacci number"
        difficulty = 6
    
    return {
        "description": description,
        "expected": result,
        "expected_str": f"[{result}]",
        "phase": 4,
        "difficulty": difficulty,
    }


def _generate_list_task() -> dict:
    """Generate Phase 5 list task"""
    task_type = random.choice(["len", "sum", "max", "reverse"])
    
    lst = [random.randint(1, 20) for _ in range(random.randint(3, 6))]
    lst_str = f"[{' '.join(map(str, lst))}]"
    
    if task_type == "len":
        description = f"Get the length of list {lst}"
        result = len(lst)
        difficulty = 4
    
    elif task_type == "sum":
        description = f"Sum all elements in list {lst}"
        result = sum(lst)
        difficulty = 5
    
    elif task_type == "max":
        description = f"Find the maximum element in list {lst}"
        result = max(lst)
        difficulty = 5
    
    else:  # reverse
        description = f"Reverse the list {lst}"
        result = list(reversed(lst))
        difficulty = 5
    
    return {
        "description": description,
        "expected": result,
        "expected_str": f"{result}" if isinstance(result, list) else f"[{result}]",
        "phase": 5,
        "difficulty": difficulty,
    }


def generate_batch(phase: int, n: int) -> list:
    """Generate a batch of tasks for a phase"""
    return [generate_task(phase) for _ in range(n)]
