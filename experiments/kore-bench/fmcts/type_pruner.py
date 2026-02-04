"""
Type-based action pruning for FMCTS.

Uses Kore's static type information to filter out invalid actions BEFORE
execution. This reduces the branching factor significantly.

Kore Advantage:
- We know stack effects statically (consumes, produces)
- We can prevent stack underflow without executing
- We can prevent type errors for type-checked ops
"""

from typing import List, Dict, Tuple, Optional, Set
from dataclasses import dataclass


@dataclass
class TokenRequirement:
    """Requirements for a token to be valid."""
    min_stack_size: int
    required_types: Optional[List[str]] = None  # Type requirements for top items
    produces: int = 1
    description: str = ""


# Token requirements based on Kore semantics
TOKEN_REQUIREMENTS: Dict[str, TokenRequirement] = {
    # Stack manipulation
    "dup": TokenRequirement(1, None, 2, "Duplicate top"),
    "drop": TokenRequirement(1, None, 0, "Drop top"),
    "swap": TokenRequirement(2, None, 2, "Swap top two"),
    "over": TokenRequirement(2, None, 3, "Copy second to top"),
    "rot": TokenRequirement(3, None, 3, "Rotate top three"),
    "nip": TokenRequirement(2, None, 1, "Drop second"),
    "tuck": TokenRequirement(2, None, 3, "Copy top under second"),
    
    # Binary arithmetic (need 2 numbers)
    "add": TokenRequirement(2, ["number", "number"], 1, "Add"),
    "sub": TokenRequirement(2, ["number", "number"], 1, "Subtract"),
    "mul": TokenRequirement(2, ["number", "number"], 1, "Multiply"),
    "div": TokenRequirement(2, ["number", "number"], 1, "Divide"),
    "mod": TokenRequirement(2, ["number", "number"], 1, "Modulo"),
    "min": TokenRequirement(2, ["number", "number"], 1, "Minimum"),
    "max": TokenRequirement(2, ["number", "number"], 1, "Maximum"),
    
    # Unary arithmetic
    "neg": TokenRequirement(1, ["number"], 1, "Negate"),
    "abs": TokenRequirement(1, ["number"], 1, "Absolute value"),
    
    # Comparison (need 2 comparable values)
    "eq": TokenRequirement(2, None, 1, "Equal"),
    "neq": TokenRequirement(2, None, 1, "Not equal"),
    "lt": TokenRequirement(2, ["number", "number"], 1, "Less than"),
    "gt": TokenRequirement(2, ["number", "number"], 1, "Greater than"),
    "le": TokenRequirement(2, ["number", "number"], 1, "Less or equal"),
    "ge": TokenRequirement(2, ["number", "number"], 1, "Greater or equal"),
    
    # Logic
    "and": TokenRequirement(2, None, 1, "Logical and"),
    "or": TokenRequirement(2, None, 1, "Logical or"),
    "not": TokenRequirement(1, None, 1, "Logical not"),
    
    # Control flow
    "call": TokenRequirement(1, ["quote"], 0, "Call quotation"),
    "if": TokenRequirement(3, [None, "quote", "quote"], 0, "Conditional"),
    "times": TokenRequirement(2, ["number", "quote"], 0, "Repeat n times"),
    "while": TokenRequirement(2, ["quote", "quote"], 0, "While loop"),
    "dip": TokenRequirement(2, [None, "quote"], 1, "Dip under top"),
    
    # List operations
    "list-len": TokenRequirement(1, ["list"], 1, "List length"),
    "list-get": TokenRequirement(2, ["list", "number"], 1, "Get item"),
    "list-set": TokenRequirement(3, ["list", "number", None], 1, "Set item"),
    "list-push": TokenRequirement(2, ["list", None], 1, "Push to list"),
    "list-pop": TokenRequirement(1, ["list"], 2, "Pop from list"),
    "list-wrap": TokenRequirement(1, None, 1, "Wrap in list"),
    
    # String operations
    "str-len": TokenRequirement(1, ["text"], 1, "String length"),
    "str-concat": TokenRequirement(2, ["text", "text"], 1, "Concatenate"),
    
    # Type checks (always valid if stack has 1 item)
    "type": TokenRequirement(1, None, 1, "Get type"),
    "is-int": TokenRequirement(1, None, 1, "Check if int"),
    "is-text": TokenRequirement(1, None, 1, "Check if text"),
    "is-list": TokenRequirement(1, None, 1, "Check if list"),
    "is-error": TokenRequirement(1, None, 1, "Check if error"),
}

# Literal patterns
LITERAL_PATTERNS = {
    "int": r"^-?\d+$",
    "float": r"^-?\d+\.\d+$",
    "bool": r"^(true|false)$",
    "null": r"^null$",
}


def get_value_type(value) -> str:
    """Get the type category of a Kore value."""
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str):
        return "text"
    if isinstance(value, list):
        # Check if it's a quotation (list of ops) or a data list
        if len(value) > 0 and isinstance(value[0], str):
            return "quote"  # Simplified check
        return "list"
    if isinstance(value, dict):
        return "map"
    return "unknown"


def is_number_type(value) -> bool:
    """Check if value is a number (int or float)."""
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def type_matches(value, required_type: Optional[str]) -> bool:
    """Check if a value matches the required type."""
    if required_type is None:
        return True
    
    actual_type = get_value_type(value)
    
    if required_type == "number":
        return actual_type in ("int", "float")
    
    return actual_type == required_type


def get_token_requirements(token: str) -> TokenRequirement:
    """Get requirements for a token."""
    if token in TOKEN_REQUIREMENTS:
        return TOKEN_REQUIREMENTS[token]
    
    # Literals - always valid (push to stack)
    import re
    for type_name, pattern in LITERAL_PATTERNS.items():
        if re.match(pattern, token):
            return TokenRequirement(0, None, 1, f"Literal {type_name}")
    
    # Quoted string
    if (token.startswith('"') and token.endswith('"')) or \
       (token.startswith("'") and token.endswith("'")):
        return TokenRequirement(0, None, 1, "String literal")
    
    # Quotation start/end (special handling)
    if token in ("[", "]", "{", "}"):
        return TokenRequirement(0, None, 0, "Bracket")
    
    # Unknown token - assume it's a user-defined word that needs 0 and produces 1
    return TokenRequirement(0, None, 1, "Unknown/user-defined")


def valid_tokens(
    stack: List,
    vocab: List[str],
    strict_types: bool = False
) -> List[str]:
    """
    Filter vocabulary to only type-valid tokens.
    
    This is the KEY function for Kore-native MCTS - we prune the action
    space BEFORE execution using static type information.
    
    Args:
        stack: Current stack state (as Python values)
        vocab: Full vocabulary of possible tokens
        strict_types: If True, also check type compatibility (slower)
    
    Returns:
        List of valid tokens that won't cause immediate failure
    """
    valid = []
    stack_size = len(stack)
    
    for token in vocab:
        req = get_token_requirements(token)
        
        # Check 1: Stack size requirement
        if stack_size < req.min_stack_size:
            continue
        
        # Check 2: Type requirements (if enabled and specified)
        if strict_types and req.required_types:
            types_ok = True
            for i, required_type in enumerate(req.required_types):
                if required_type is None:
                    continue
                # Stack grows left-to-right, top is at the end
                stack_idx = -(len(req.required_types) - i)
                if stack_size + stack_idx < 0:
                    types_ok = False
                    break
                if not type_matches(stack[stack_idx], required_type):
                    types_ok = False
                    break
            if not types_ok:
                continue
        
        # Check 3: Division by zero
        if token == "div" and stack_size >= 2:
            divisor = stack[-1]
            if isinstance(divisor, (int, float)) and divisor == 0:
                continue
        
        valid.append(token)
    
    return valid


def get_pruning_stats(stack: List, vocab: List[str]) -> Dict[str, int]:
    """Get statistics about pruning effectiveness."""
    total = len(vocab)
    valid = len(valid_tokens(stack, vocab, strict_types=False))
    strict_valid = len(valid_tokens(stack, vocab, strict_types=True))
    
    return {
        "total_vocab": total,
        "stack_valid": valid,
        "type_valid": strict_valid,
        "pruned_by_stack": total - valid,
        "pruned_by_type": valid - strict_valid,
        "prune_ratio": 1.0 - (strict_valid / total) if total > 0 else 0.0,
    }


# Default vocabulary for Kore synthesis
KORE_VOCAB = [
    # Literals (represented as patterns, actual numbers generated separately)
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10",
    "-1", "-2",
    "true", "false", "null",
    
    # Stack manipulation
    "dup", "drop", "swap", "over", "rot", "nip", "tuck",
    
    # Arithmetic
    "add", "sub", "mul", "div", "mod", "neg", "abs", "min", "max",
    
    # Comparison
    "eq", "neq", "lt", "gt", "le", "ge",
    
    # Logic
    "and", "or", "not",
    
    # Control flow (simplified - quotations handled specially)
    # "call", "if", "times", "while", "dip",
    
    # List operations
    "list-len", "list-get", "list-push", "list-wrap",
    
    # Type checks
    "type", "is-int", "is-list",
]


# Extended vocabulary including control flow
KORE_VOCAB_EXTENDED = KORE_VOCAB + [
    "[", "]",  # Quotation delimiters
    "call", "if", "times", "while", "dip",
]
