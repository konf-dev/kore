#!/usr/bin/env python3
"""
Kore Simulator with Full Tracing

A pure Python implementation of Kore semantics that provides:
1. Token-by-token incremental execution
2. Full execution trace (stack states, tool calls)
3. Rich error information for training signals

Architecture:
    LLM → tokens → KoreSimulator.step() → (stack, trace, error) → loss

This enables:
- Early error detection during generation
- Partial credit based on stack progress
- Learning from execution traces, not just final results
"""

from dataclasses import dataclass, field
from typing import List, Dict, Tuple, Optional, Any, Callable, Union
from enum import Enum
import json
import re


# ============================================================================
# Value Types (matches Kore's 10 types)
# ============================================================================

class ValueType(Enum):
    NULL = "null"
    BOOL = "bool"
    INT = "int"
    FLOAT = "float"
    TEXT = "text"
    LIST = "list"
    MAP = "map"
    QUOTE = "quote"
    HANDLE = "handle"
    ERROR = "error"


@dataclass
class Value:
    """Kore value - one of 10 types."""
    type: ValueType
    data: Any
    
    @staticmethod
    def null() -> "Value":
        return Value(ValueType.NULL, None)
    
    @staticmethod
    def bool_(b: bool) -> "Value":
        return Value(ValueType.BOOL, b)
    
    @staticmethod
    def int_(n: int) -> "Value":
        return Value(ValueType.INT, n)
    
    @staticmethod
    def float_(f: float) -> "Value":
        return Value(ValueType.FLOAT, f)
    
    @staticmethod
    def text(s: str) -> "Value":
        return Value(ValueType.TEXT, s)
    
    @staticmethod
    def list_(items: List["Value"]) -> "Value":
        return Value(ValueType.LIST, items)
    
    @staticmethod
    def map_(d: Dict[str, "Value"]) -> "Value":
        return Value(ValueType.MAP, d)
    
    @staticmethod
    def quote(ops: List["Op"]) -> "Value":
        return Value(ValueType.QUOTE, ops)
    
    @staticmethod
    def error(code: str, msg: str) -> "Value":
        return Value(ValueType.ERROR, {"code": code, "message": msg})
    
    def is_truthy(self) -> bool:
        if self.type == ValueType.NULL:
            return False
        if self.type == ValueType.BOOL:
            return self.data
        if self.type == ValueType.INT:
            return self.data != 0
        if self.type == ValueType.FLOAT:
            return self.data != 0.0
        if self.type == ValueType.TEXT:
            return len(self.data) > 0
        if self.type == ValueType.LIST:
            return len(self.data) > 0
        if self.type == ValueType.ERROR:
            return False
        return True
    
    def to_python(self) -> Any:
        """Convert to Python native type for JSON serialization."""
        if self.type == ValueType.NULL:
            return None
        if self.type == ValueType.BOOL:
            return self.data
        if self.type == ValueType.INT:
            return self.data
        if self.type == ValueType.FLOAT:
            return self.data
        if self.type == ValueType.TEXT:
            return self.data
        if self.type == ValueType.LIST:
            return [v.to_python() for v in self.data]
        if self.type == ValueType.MAP:
            return {k: v.to_python() for k, v in self.data.items()}
        if self.type == ValueType.QUOTE:
            return f"[{' '.join(str(op) for op in self.data)}]"
        if self.type == ValueType.ERROR:
            return {"error": self.data}
        return str(self.data)
    
    def __repr__(self):
        if self.type == ValueType.QUOTE:
            return f"[{' '.join(str(op) for op in self.data)}]"
        return repr(self.to_python())


# ============================================================================
# Operations (only 2: Push and Call)
# ============================================================================

@dataclass
class Op:
    """A Kore operation - either Push or Call."""
    kind: str  # "push" or "call"
    value: Any  # Value for push, tool name (str) for call
    
    @staticmethod
    def push(v: Value) -> "Op":
        return Op("push", v)
    
    @staticmethod
    def call(name: str) -> "Op":
        return Op("call", name)
    
    def __repr__(self):
        if self.kind == "push":
            return repr(self.value)
        return self.value


# ============================================================================
# Trace Entry
# ============================================================================

@dataclass
class TraceEntry:
    """A single step in the execution trace."""
    step: int
    op: str                           # The operation that was executed
    stack_before: List[Any]           # Stack state before
    stack_after: List[Any]            # Stack state after
    success: bool                     # Did it succeed?
    error: Optional[str] = None       # Error message if failed
    effect: Tuple[int, int] = (0, 0)  # (consumed, produced)
    
    def to_dict(self) -> dict:
        return {
            "step": self.step,
            "op": self.op,
            "stack_before": self.stack_before,
            "stack_after": self.stack_after,
            "success": self.success,
            "error": self.error,
            "effect": list(self.effect),
        }


@dataclass
class ExecutionResult:
    """Complete result of executing a Kore program."""
    success: bool
    final_stack: List[Any]
    trace: List[TraceEntry]
    error: Optional[str] = None
    error_at_step: Optional[int] = None
    tokens_executed: int = 0
    
    def to_dict(self) -> dict:
        return {
            "success": self.success,
            "final_stack": self.final_stack,
            "trace": [t.to_dict() for t in self.trace],
            "error": self.error,
            "error_at_step": self.error_at_step,
            "tokens_executed": self.tokens_executed,
        }


# ============================================================================
# Stack Effect Signatures
# ============================================================================

# (consumes, produces) for each built-in tool
EFFECTS: Dict[str, Tuple[int, int]] = {
    # Stack manipulation
    "dup": (1, 2),
    "drop": (1, 0),
    "swap": (2, 2),
    "over": (2, 3),
    "rot": (3, 3),
    "nip": (2, 1),
    "tuck": (2, 3),
    "pick": (1, 1),  # n -- value (depends on n)
    
    # Arithmetic
    "add": (2, 1),
    "sub": (2, 1),
    "mul": (2, 1),
    "div": (2, 1),
    "mod": (2, 1),
    "neg": (1, 1),
    "abs": (1, 1),
    "min": (2, 1),
    "max": (2, 1),
    
    # Comparison
    "eq": (2, 1),
    "neq": (2, 1),
    "lt": (2, 1),
    "gt": (2, 1),
    "le": (2, 1),
    "ge": (2, 1),
    
    # Logic
    "and": (2, 1),
    "or": (2, 1),
    "not": (1, 1),
    
    # Control flow
    "call": (1, 0),  # Approximate - depends on quote
    "if": (3, 0),    # Approximate - depends on branches
    "times": (2, 0), # Approximate
    "while": (2, 0), # Approximate
    "dip": (2, 1),   # (a q -- a) but runs q
    
    # List operations
    "list-len": (1, 1),
    "list-get": (2, 1),
    "list-set": (3, 1),
    "list-push": (2, 1),
    "list-pop": (1, 2),
    "list-wrap": (1, 1),  # a -- [a]
    "unlist": (1, 0),     # Pushes variable number
    
    # Map operations
    "map-get": (2, 1),
    "map-set": (3, 1),
    "map-has": (2, 1),
    "map-keys": (1, 1),
    "map-vals": (1, 1),
    
    # String operations
    "str-len": (1, 1),
    "str-concat": (2, 1),
    "str-split": (2, 1),
    
    # Type checks
    "type": (1, 1),
    "is-int": (1, 1),
    "is-text": (1, 1),
    "is-list": (1, 1),
    "is-error": (1, 1),
    
    # Error handling
    "try": (1, 1),
    "fail": (1, 0),
    "unwrap": (1, 1),
}


# ============================================================================
# Kore Simulator
# ============================================================================

class KoreSimulator:
    """
    Simulates Kore execution with full tracing.
    
    Designed for incremental execution during LLM generation.
    """
    
    def __init__(self):
        self.stack: List[Value] = []
        self.trace: List[TraceEntry] = []
        self.step_count: int = 0
        self.error: Optional[str] = None
        
        # User-defined tools (from `def`)
        self.definitions: Dict[str, List[Op]] = {}
    
    def reset(self):
        """Reset simulator state."""
        self.stack = []
        self.trace = []
        self.step_count = 0
        self.error = None
        self.definitions = {}
    
    def get_stack_python(self) -> List[Any]:
        """Get stack as Python values."""
        return [v.to_python() for v in self.stack]
    
    def effect_of(self, token: str) -> Tuple[int, int]:
        """Get stack effect for a token."""
        if token in EFFECTS:
            return EFFECTS[token]
        if token in self.definitions:
            # Could analyze definition, but approximate
            return (0, 1)
        # Literals push 1
        try:
            int(token)
            return (0, 1)
        except:
            pass
        try:
            float(token)
            return (0, 1)
        except:
            pass
        if token in ("true", "false", "null"):
            return (0, 1)
        if token.startswith('"') or token.startswith("'"):
            return (0, 1)
        # Unknown tool - conservative
        return (0, 1)
    
    def can_execute(self, token: str) -> Tuple[bool, Optional[str]]:
        """Check if token can be executed with current stack."""
        consumes, _ = self.effect_of(token)
        if len(self.stack) < consumes:
            return False, f"Stack underflow: '{token}' needs {consumes} values, have {len(self.stack)}"
        return True, None
    
    def step(self, token: str) -> Tuple[bool, Optional[str]]:
        """
        Execute a single token.
        
        Returns: (success, error_message)
        
        This is the key method for incremental execution during LLM generation.
        """
        if self.error:
            return False, f"Previous error: {self.error}"
        
        stack_before = self.get_stack_python()
        
        try:
            success = self._execute_token(token)
            stack_after = self.get_stack_python()
            
            consumes, produces = self.effect_of(token)
            
            self.trace.append(TraceEntry(
                step=self.step_count,
                op=token,
                stack_before=stack_before,
                stack_after=stack_after,
                success=success,
                error=None if success else self.error,
                effect=(consumes, produces),
            ))
            
            self.step_count += 1
            
            if success:
                return True, None
            else:
                return False, self.error
                
        except Exception as e:
            self.error = str(e)
            self.trace.append(TraceEntry(
                step=self.step_count,
                op=token,
                stack_before=stack_before,
                stack_after=self.get_stack_python(),
                success=False,
                error=str(e),
            ))
            return False, str(e)
    
    def _execute_token(self, token: str) -> bool:
        """Execute a single token. Returns success."""
        
        # === Literals ===
        
        # Integer
        try:
            n = int(token)
            self.stack.append(Value.int_(n))
            return True
        except ValueError:
            pass
        
        # Float
        try:
            f = float(token)
            self.stack.append(Value.float_(f))
            return True
        except ValueError:
            pass
        
        # Boolean
        if token == "true":
            self.stack.append(Value.bool_(True))
            return True
        if token == "false":
            self.stack.append(Value.bool_(False))
            return True
        
        # Null
        if token == "null":
            self.stack.append(Value.null())
            return True
        
        # String (simple handling)
        if (token.startswith('"') and token.endswith('"')) or \
           (token.startswith("'") and token.endswith("'")):
            s = token[1:-1]
            self.stack.append(Value.text(s))
            return True
        
        # === User definitions ===
        if token in self.definitions:
            # Execute the definition's ops
            for op in self.definitions[token]:
                if op.kind == "push":
                    self.stack.append(op.value)
                else:
                    success = self._execute_token(op.value)
                    if not success:
                        return False
            return True
        
        # === Built-in tools ===
        return self._execute_builtin(token)
    
    def _execute_builtin(self, name: str) -> bool:
        """Execute a built-in tool."""
        
        # Stack manipulation
        if name == "dup":
            if len(self.stack) < 1:
                self.error = "Stack underflow: dup needs 1 value"
                return False
            self.stack.append(self.stack[-1])
            return True
        
        if name == "drop":
            if len(self.stack) < 1:
                self.error = "Stack underflow: drop needs 1 value"
                return False
            self.stack.pop()
            return True
        
        if name == "swap":
            if len(self.stack) < 2:
                self.error = "Stack underflow: swap needs 2 values"
                return False
            self.stack[-1], self.stack[-2] = self.stack[-2], self.stack[-1]
            return True
        
        if name == "over":
            if len(self.stack) < 2:
                self.error = "Stack underflow: over needs 2 values"
                return False
            self.stack.append(self.stack[-2])
            return True
        
        if name == "rot":
            if len(self.stack) < 3:
                self.error = "Stack underflow: rot needs 3 values"
                return False
            a = self.stack.pop()
            b = self.stack.pop()
            c = self.stack.pop()
            self.stack.extend([b, a, c])
            return True
        
        if name == "nip":
            if len(self.stack) < 2:
                self.error = "Stack underflow: nip needs 2 values"
                return False
            del self.stack[-2]
            return True
        
        # Arithmetic
        if name == "add":
            if len(self.stack) < 2:
                self.error = "Stack underflow: add needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            if a.type == ValueType.INT and b.type == ValueType.INT:
                self.stack.append(Value.int_(a.data + b.data))
            elif a.type in (ValueType.INT, ValueType.FLOAT) and \
                 b.type in (ValueType.INT, ValueType.FLOAT):
                self.stack.append(Value.float_(float(a.data) + float(b.data)))
            elif a.type == ValueType.TEXT and b.type == ValueType.TEXT:
                self.stack.append(Value.text(a.data + b.data))
            else:
                self.error = f"Type error: cannot add {a.type.value} and {b.type.value}"
                return False
            return True
        
        if name == "sub":
            if len(self.stack) < 2:
                self.error = "Stack underflow: sub needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            if a.type == ValueType.INT and b.type == ValueType.INT:
                self.stack.append(Value.int_(a.data - b.data))
            else:
                self.stack.append(Value.float_(float(a.data) - float(b.data)))
            return True
        
        if name == "mul":
            if len(self.stack) < 2:
                self.error = "Stack underflow: mul needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            if a.type == ValueType.INT and b.type == ValueType.INT:
                self.stack.append(Value.int_(a.data * b.data))
            else:
                self.stack.append(Value.float_(float(a.data) * float(b.data)))
            return True
        
        if name == "div":
            if len(self.stack) < 2:
                self.error = "Stack underflow: div needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            if b.data == 0:
                self.error = "Division by zero"
                return False
            if a.type == ValueType.INT and b.type == ValueType.INT:
                self.stack.append(Value.int_(a.data // b.data))
            else:
                self.stack.append(Value.float_(float(a.data) / float(b.data)))
            return True
        
        if name == "mod":
            if len(self.stack) < 2:
                self.error = "Stack underflow: mod needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            if b.data == 0:
                self.error = "Modulo by zero"
                return False
            self.stack.append(Value.int_(a.data % b.data))
            return True
        
        if name == "neg":
            if len(self.stack) < 1:
                self.error = "Stack underflow: neg needs 1 value"
                return False
            a = self.stack.pop()
            if a.type == ValueType.INT:
                self.stack.append(Value.int_(-a.data))
            else:
                self.stack.append(Value.float_(-float(a.data)))
            return True
        
        # Comparison
        if name == "eq":
            if len(self.stack) < 2:
                self.error = "Stack underflow: eq needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data == b.data))
            return True
        
        if name == "neq":
            if len(self.stack) < 2:
                self.error = "Stack underflow: neq needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data != b.data))
            return True
        
        if name == "lt":
            if len(self.stack) < 2:
                self.error = "Stack underflow: lt needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data < b.data))
            return True
        
        if name == "gt":
            if len(self.stack) < 2:
                self.error = "Stack underflow: gt needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data > b.data))
            return True
        
        if name == "le":
            if len(self.stack) < 2:
                self.error = "Stack underflow: le needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data <= b.data))
            return True
        
        if name == "ge":
            if len(self.stack) < 2:
                self.error = "Stack underflow: ge needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.data >= b.data))
            return True
        
        # Logic
        if name == "and":
            if len(self.stack) < 2:
                self.error = "Stack underflow: and needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.is_truthy() and b.is_truthy()))
            return True
        
        if name == "or":
            if len(self.stack) < 2:
                self.error = "Stack underflow: or needs 2 values"
                return False
            b = self.stack.pop()
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.is_truthy() or b.is_truthy()))
            return True
        
        if name == "not":
            if len(self.stack) < 1:
                self.error = "Stack underflow: not needs 1 value"
                return False
            a = self.stack.pop()
            self.stack.append(Value.bool_(not a.is_truthy()))
            return True
        
        # Type operations
        if name == "type":
            if len(self.stack) < 1:
                self.error = "Stack underflow: type needs 1 value"
                return False
            a = self.stack.pop()
            self.stack.append(Value.text(a.type.value))
            return True
        
        if name == "is-int":
            if len(self.stack) < 1:
                self.error = "Stack underflow"
                return False
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.type == ValueType.INT))
            return True
        
        if name == "is-error":
            if len(self.stack) < 1:
                self.error = "Stack underflow"
                return False
            a = self.stack.pop()
            self.stack.append(Value.bool_(a.type == ValueType.ERROR))
            return True
        
        # List operations
        if name == "list-len":
            if len(self.stack) < 1:
                self.error = "Stack underflow: list-len needs 1 value"
                return False
            a = self.stack.pop()
            if a.type != ValueType.LIST:
                self.error = f"Type error: list-len expects list, got {a.type.value}"
                return False
            self.stack.append(Value.int_(len(a.data)))
            return True
        
        if name == "list-wrap":
            if len(self.stack) < 1:
                self.error = "Stack underflow: list-wrap needs 1 value"
                return False
            a = self.stack.pop()
            self.stack.append(Value.list_([a]))
            return True
        
        # Unknown tool
        self.error = f"Unknown tool: {name}"
        return False
    
    def execute(self, program: str) -> ExecutionResult:
        """
        Execute a complete program.
        
        Returns full execution result with trace.
        """
        self.reset()
        
        tokens = self._tokenize(program)
        
        i = 0
        while i < len(tokens):
            token = tokens[i]
            
            # Handle quote literals: [ ... ]
            if token == "[":
                depth = 1
                end = i + 1
                while end < len(tokens) and depth > 0:
                    if tokens[end] == "[":
                        depth += 1
                    elif tokens[end] == "]":
                        depth -= 1
                    end += 1
                
                # Parse inner tokens into ops
                inner_tokens = tokens[i+1:end-1]
                inner_ops = self._tokens_to_ops(inner_tokens)
                
                stack_before = self.get_stack_python()
                self.stack.append(Value.quote(inner_ops))
                
                self.trace.append(TraceEntry(
                    step=self.step_count,
                    op=f"[{' '.join(inner_tokens)}]",
                    stack_before=stack_before,
                    stack_after=self.get_stack_python(),
                    success=True,
                    effect=(0, 1),
                ))
                self.step_count += 1
                
                i = end
                continue
            
            # Handle `def`: value "name" def
            if token == "def" and len(self.stack) >= 2:
                name_val = self.stack.pop()
                value = self.stack.pop()
                
                if name_val.type != ValueType.TEXT:
                    self.error = f"def: name must be text, got {name_val.type.value}"
                    return ExecutionResult(
                        success=False,
                        final_stack=self.get_stack_python(),
                        trace=self.trace,
                        error=self.error,
                        error_at_step=self.step_count,
                        tokens_executed=i,
                    )
                
                name = name_val.data
                
                if value.type == ValueType.QUOTE:
                    self.definitions[name] = value.data
                else:
                    # Define as constant
                    self.definitions[name] = [Op.push(value)]
                
                self.trace.append(TraceEntry(
                    step=self.step_count,
                    op=f"def:{name}",
                    stack_before=self.get_stack_python(),
                    stack_after=self.get_stack_python(),
                    success=True,
                    effect=(2, 0),
                ))
                self.step_count += 1
                i += 1
                continue
            
            # Handle `call`: execute quote on stack
            if token == "call":
                if len(self.stack) < 1:
                    self.error = "Stack underflow: call needs 1 value"
                    return ExecutionResult(
                        success=False,
                        final_stack=self.get_stack_python(),
                        trace=self.trace,
                        error=self.error,
                        error_at_step=self.step_count,
                        tokens_executed=i,
                    )
                
                quote = self.stack.pop()
                if quote.type != ValueType.QUOTE:
                    self.error = f"call: expected quote, got {quote.type.value}"
                    return ExecutionResult(
                        success=False,
                        final_stack=self.get_stack_python(),
                        trace=self.trace,
                        error=self.error,
                        error_at_step=self.step_count,
                        tokens_executed=i,
                    )
                
                # Execute quote ops
                for op in quote.data:
                    if op.kind == "push":
                        self.stack.append(op.value)
                    else:
                        success, err = self.step(op.value)
                        if not success:
                            return ExecutionResult(
                                success=False,
                                final_stack=self.get_stack_python(),
                                trace=self.trace,
                                error=err,
                                error_at_step=self.step_count,
                                tokens_executed=i,
                            )
                
                i += 1
                continue
            
            # Handle `if`: (cond then else -- result)
            if token == "if":
                if len(self.stack) < 3:
                    self.error = "Stack underflow: if needs 3 values"
                    return ExecutionResult(
                        success=False,
                        final_stack=self.get_stack_python(),
                        trace=self.trace,
                        error=self.error,
                        error_at_step=self.step_count,
                        tokens_executed=i,
                    )
                
                else_q = self.stack.pop()
                then_q = self.stack.pop()
                cond = self.stack.pop()
                
                chosen = then_q if cond.is_truthy() else else_q
                
                if chosen.type != ValueType.QUOTE:
                    self.error = f"if: branches must be quotes"
                    return ExecutionResult(
                        success=False,
                        final_stack=self.get_stack_python(),
                        trace=self.trace,
                        error=self.error,
                        error_at_step=self.step_count,
                        tokens_executed=i,
                    )
                
                # Execute chosen branch
                for op in chosen.data:
                    if op.kind == "push":
                        self.stack.append(op.value)
                    else:
                        success, err = self.step(op.value)
                        if not success:
                            return ExecutionResult(
                                success=False,
                                final_stack=self.get_stack_python(),
                                trace=self.trace,
                                error=err,
                                error_at_step=self.step_count,
                                tokens_executed=i,
                            )
                
                self.trace.append(TraceEntry(
                    step=self.step_count,
                    op=f"if:{cond.is_truthy()}",
                    stack_before=[],
                    stack_after=self.get_stack_python(),
                    success=True,
                ))
                self.step_count += 1
                i += 1
                continue
            
            # Regular token
            success, err = self.step(token)
            if not success:
                return ExecutionResult(
                    success=False,
                    final_stack=self.get_stack_python(),
                    trace=self.trace,
                    error=err,
                    error_at_step=self.step_count,
                    tokens_executed=i,
                )
            
            i += 1
        
        return ExecutionResult(
            success=True,
            final_stack=self.get_stack_python(),
            trace=self.trace,
            tokens_executed=len(tokens),
        )
    
    def _tokenize(self, program: str) -> List[str]:
        """Tokenize a Kore program."""
        tokens = []
        current = ""
        in_string = False
        string_char = None
        
        i = 0
        while i < len(program):
            c = program[i]
            
            # Handle strings
            if c in ('"', "'") and not in_string:
                if current:
                    tokens.append(current)
                    current = ""
                in_string = True
                string_char = c
                current = c
                i += 1
                continue
            
            if in_string:
                current += c
                if c == string_char:
                    tokens.append(current)
                    current = ""
                    in_string = False
                i += 1
                continue
            
            # Comments
            if c == '#':
                if current:
                    tokens.append(current)
                    current = ""
                while i < len(program) and program[i] != '\n':
                    i += 1
                continue
            
            # Whitespace
            if c in ' \t\n\r':
                if current:
                    tokens.append(current)
                    current = ""
                i += 1
                continue
            
            # Brackets (separate tokens)
            if c in '[]':
                if current:
                    tokens.append(current)
                    current = ""
                tokens.append(c)
                i += 1
                continue
            
            current += c
            i += 1
        
        if current:
            tokens.append(current)
        
        return tokens
    
    def _tokens_to_ops(self, tokens: List[str]) -> List[Op]:
        """Convert tokens to Op objects (for quotes)."""
        ops = []
        i = 0
        
        while i < len(tokens):
            token = tokens[i]
            
            # Nested quote
            if token == "[":
                depth = 1
                end = i + 1
                while end < len(tokens) and depth > 0:
                    if tokens[end] == "[":
                        depth += 1
                    elif tokens[end] == "]":
                        depth -= 1
                    end += 1
                
                inner = self._tokens_to_ops(tokens[i+1:end-1])
                ops.append(Op.push(Value.quote(inner)))
                i = end
                continue
            
            # Literals
            try:
                n = int(token)
                ops.append(Op.push(Value.int_(n)))
                i += 1
                continue
            except:
                pass
            
            try:
                f = float(token)
                ops.append(Op.push(Value.float_(f)))
                i += 1
                continue
            except:
                pass
            
            if token == "true":
                ops.append(Op.push(Value.bool_(True)))
            elif token == "false":
                ops.append(Op.push(Value.bool_(False)))
            elif token == "null":
                ops.append(Op.push(Value.null()))
            elif token.startswith('"') or token.startswith("'"):
                ops.append(Op.push(Value.text(token[1:-1])))
            else:
                ops.append(Op.call(token))
            
            i += 1
        
        return ops


# ============================================================================
# Incremental Executor (for LLM integration)
# ============================================================================

class IncrementalExecutor:
    """
    Executes Kore incrementally during LLM generation.
    
    Usage:
        exec = IncrementalExecutor()
        
        for token in llm.generate():
            result = exec.step(token)
            if result.error:
                # Can influence next token or stop
                break
            # Use result.stack for conditioning
    """
    
    def __init__(self):
        self.sim = KoreSimulator()
        self.tokens: List[str] = []
        self.pending_quote: List[str] = []
        self.quote_depth: int = 0
    
    def reset(self):
        """Reset for new program."""
        self.sim.reset()
        self.tokens = []
        self.pending_quote = []
        self.quote_depth = 0
    
    def step(self, token: str) -> ExecutionResult:
        """
        Process a single token during generation.
        
        Handles quote buffering - tokens inside [...] are buffered
        until the quote is complete.
        """
        self.tokens.append(token)
        
        # Track quote depth
        if token == "[":
            self.quote_depth += 1
            self.pending_quote.append(token)
            return ExecutionResult(
                success=True,
                final_stack=self.sim.get_stack_python(),
                trace=self.sim.trace,
                tokens_executed=len(self.tokens),
            )
        
        if token == "]":
            self.quote_depth -= 1
            self.pending_quote.append(token)
            
            if self.quote_depth == 0:
                # Quote complete - parse and push
                quote_str = " ".join(self.pending_quote)
                self.pending_quote = []
                return self.sim.execute(quote_str)
            else:
                return ExecutionResult(
                    success=True,
                    final_stack=self.sim.get_stack_python(),
                    trace=self.sim.trace,
                    tokens_executed=len(self.tokens),
                )
        
        if self.quote_depth > 0:
            # Inside a quote - buffer
            self.pending_quote.append(token)
            return ExecutionResult(
                success=True,
                final_stack=self.sim.get_stack_python(),
                trace=self.sim.trace,
                tokens_executed=len(self.tokens),
            )
        
        # Regular token - execute immediately
        success, error = self.sim.step(token)
        
        return ExecutionResult(
            success=success,
            final_stack=self.sim.get_stack_python(),
            trace=self.sim.trace,
            error=error,
            error_at_step=self.sim.step_count if not success else None,
            tokens_executed=len(self.tokens),
        )
    
    def get_state(self) -> dict:
        """Get current execution state for LLM conditioning."""
        return {
            "stack": self.sim.get_stack_python(),
            "tokens": self.tokens,
            "in_quote": self.quote_depth > 0,
            "quote_depth": self.quote_depth,
            "error": self.sim.error,
            "step_count": self.sim.step_count,
        }


# ============================================================================
# Main / Demo
# ============================================================================

if __name__ == "__main__":
    # Demo execution
    sim = KoreSimulator()
    
    # Test basic program
    print("=" * 60)
    print("Test: 3 4 add 2 mul")
    result = sim.execute("3 4 add 2 mul")
    print(f"Success: {result.success}")
    print(f"Stack: {result.final_stack}")
    print(f"Trace:")
    for t in result.trace:
        print(f"  {t.step}: {t.op} | {t.stack_before} → {t.stack_after}")
    
    # Test error
    print("\n" + "=" * 60)
    print("Test: 3 add (stack underflow)")
    result = sim.execute("3 add")
    print(f"Success: {result.success}")
    print(f"Error: {result.error}")
    print(f"Error at step: {result.error_at_step}")
    
    # Test incremental execution
    print("\n" + "=" * 60)
    print("Test: Incremental execution")
    inc = IncrementalExecutor()
    
    tokens = ["5", "3", "add", "2", "mul"]
    for token in tokens:
        result = inc.step(token)
        state = inc.get_state()
        print(f"  Token '{token}' → Stack: {state['stack']}, Error: {state['error']}")
    
    # Test quote handling
    print("\n" + "=" * 60)
    print("Test: Quote handling [ 1 2 add ] call")
    inc.reset()
    
    tokens = ["[", "1", "2", "add", "]", "call"]
    for token in tokens:
        result = inc.step(token)
        state = inc.get_state()
        print(f"  Token '{token}' → Stack: {state['stack']}, In quote: {state['in_quote']}")
    
    # Test definition
    print("\n" + "=" * 60)
    print("Test: [ dup mul ] 'square' def  5 square")
    result = sim.execute("[ dup mul ] \"square\" def  5 square")
    print(f"Stack: {result.final_stack}")
