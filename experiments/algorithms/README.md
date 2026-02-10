# Algorithms

Iterative algorithms proving that `while` loops + basic stack operations = Turing-complete computation.
No recursion needed. P3: composition = concatenation.

## Files

| File | Algorithm | Result |
|------|-----------|--------|
| `iterative.kore` | Fibonacci F(20), GCD(252,105), isqrt(144), 2^10 | `[6765, 21, 12, 1024]` |

## Stack Patterns

Each algorithm uses a 3-element stack window `[a b counter]` with `rot`/`swap`/`over` to simulate local variables:

- **Fibonacci**: `swap over +` advances `(a,b) → (b, a+b)`
- **GCD (Euclid)**: `swap over %` computes `(a,b) → (b, a%b)`
- **Newton isqrt**: `over over / + 2 /` computes `(guess+n/guess)/2`
- **Power**: `swap over *` accumulates `result *= base`
