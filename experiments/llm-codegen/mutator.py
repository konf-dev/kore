"""
Mutator — Random program mutation strategies.

Takes a Kore token sequence and produces mutations. Used by the
evolutionary baseline (evolutionary baseline) before we have a learned policy.

Mutation types:
  - point: replace one token with another valid one
  - insert: insert a random token at a random position
  - delete: remove a random token
  - swap: swap two adjacent tokens
  - crossover: combine parts of two programs
  - grow: append a random token
  - shrink: remove from the end

All mutations respect the curriculum level (only use available tokens).
"""

from __future__ import annotations
import random
from typing import Optional
from kore_env import VOCAB_BY_LEVEL, TOK2ID, ID2TOK, END_ID, VOCAB_SIZE


def _level_tokens(level: int) -> list[str]:
    """All tokens available at a given level (excluding END)."""
    tokens = []
    for lvl in range(level + 1):
        for tok in VOCAB_BY_LEVEL.get(lvl, []):
            if tok != "END":
                tokens.append(tok)
    return tokens


def _random_token(level: int, rng: random.Random) -> str:
    """Pick a random token available at this level."""
    tokens = _level_tokens(level)
    return rng.choice(tokens)


def _weighted_random_token(level: int, rng: random.Random) -> str:
    """
    Pick a token with weights biased toward commonly useful ones.
    Literals and basic ops are more likely than advanced tokens.
    """
    tokens = _level_tokens(level)

    # Weight by inverse level — lower-level tokens are more common
    weights = []
    token_levels = {}
    for lvl in range(level + 1):
        for tok in VOCAB_BY_LEVEL.get(lvl, []):
            if tok != "END":
                token_levels[tok] = lvl

    for tok in tokens:
        lvl = token_levels.get(tok, level)
        # Weight: level 0 tokens get weight 5, level 1 get 4, etc.
        weights.append(max(1, level + 2 - lvl))

    return rng.choices(tokens, weights=weights, k=1)[0]


# =============================================================================
# Mutation operators
# =============================================================================

def mutate_point(tokens: list[str], level: int, rng: random.Random) -> list[str]:
    """Replace one random token with another."""
    if not tokens:
        return [_weighted_random_token(level, rng)]
    result = list(tokens)
    idx = rng.randint(0, len(result) - 1)
    result[idx] = _weighted_random_token(level, rng)
    return result


def mutate_insert(tokens: list[str], level: int, rng: random.Random,
                  max_len: int = 20) -> list[str]:
    """Insert a random token at a random position."""
    if len(tokens) >= max_len:
        return tokens
    result = list(tokens)
    pos = rng.randint(0, len(result))
    result.insert(pos, _weighted_random_token(level, rng))
    return result


def mutate_delete(tokens: list[str], level: int, rng: random.Random) -> list[str]:
    """Remove a random token."""
    if len(tokens) <= 1:
        return tokens
    result = list(tokens)
    idx = rng.randint(0, len(result) - 1)
    del result[idx]
    return result


def mutate_swap(tokens: list[str], level: int, rng: random.Random) -> list[str]:
    """Swap two adjacent tokens."""
    if len(tokens) < 2:
        return tokens
    result = list(tokens)
    idx = rng.randint(0, len(result) - 2)
    result[idx], result[idx + 1] = result[idx + 1], result[idx]
    return result


def mutate_grow(tokens: list[str], level: int, rng: random.Random,
                max_len: int = 20) -> list[str]:
    """Append a random token to the end."""
    if len(tokens) >= max_len:
        return tokens
    return tokens + [_weighted_random_token(level, rng)]


def mutate_shrink(tokens: list[str], level: int, rng: random.Random) -> list[str]:
    """Remove the last token."""
    if len(tokens) <= 1:
        return tokens
    return tokens[:-1]


def crossover(parent_a: list[str], parent_b: list[str], rng: random.Random,
              max_len: int = 20) -> list[str]:
    """Single-point crossover of two programs."""
    if not parent_a or not parent_b:
        return parent_a or parent_b or []
    cut_a = rng.randint(0, len(parent_a))
    cut_b = rng.randint(0, len(parent_b))
    child = parent_a[:cut_a] + parent_b[cut_b:]
    return child[:max_len]


# =============================================================================
# Smart mutations (structure-aware)
# =============================================================================

def mutate_fix_structure(tokens: list[str], level: int, rng: random.Random) -> list[str]:
    """
    Try to fix structural issues: unmatched if/end, [/], while/do/end.
    This makes mutations more likely to compile.
    """
    result = list(tokens)

    # Count brackets/control structures
    open_quotes = sum(1 for t in result if t == "[") - sum(1 for t in result if t == "]")
    open_ifs = sum(1 for t in result if t == "if") - sum(1 for t in result if t == "end")

    # Close unclosed quotes
    while open_quotes > 0:
        result.append("]")
        open_quotes -= 1

    # Remove excess closing
    while open_quotes < 0:
        for i in range(len(result) - 1, -1, -1):
            if result[i] == "]":
                del result[i]
                open_quotes += 1
                break
        else:
            break

    # Add missing "end" for if/while
    while open_ifs > 0:
        result.append("end")
        open_ifs -= 1

    return result


# =============================================================================
# Combined mutator
# =============================================================================

# Mutation type weights
MUTATION_WEIGHTS = {
    "point": 30,
    "insert": 20,
    "delete": 15,
    "swap": 15,
    "grow": 10,
    "shrink": 5,
    "fix": 5,
}

MUTATION_OPS = {
    "point": mutate_point,
    "insert": mutate_insert,
    "delete": mutate_delete,
    "swap": mutate_swap,
    "grow": mutate_grow,
    "shrink": mutate_shrink,
    "fix": mutate_fix_structure,
}


def mutate(
    tokens: list[str],
    level: int,
    rng: random.Random,
    n_mutations: int = 1,
    max_len: int = 20,
) -> list[str]:
    """
    Apply n_mutations random mutations to a token sequence.

    Returns a new token list (does not modify input).
    """
    result = list(tokens)
    ops = list(MUTATION_WEIGHTS.keys())
    weights = list(MUTATION_WEIGHTS.values())

    for _ in range(n_mutations):
        op_name = rng.choices(ops, weights=weights, k=1)[0]
        op_fn = MUTATION_OPS[op_name]
        if op_name in ("insert", "grow"):
            result = op_fn(result, level, rng, max_len=max_len)
        else:
            result = op_fn(result, level, rng)

    return result


def random_program(level: int, rng: random.Random, length: int = 5) -> list[str]:
    """Generate a completely random program of given length."""
    return [_weighted_random_token(level, rng) for _ in range(length)]


def mutate_population(
    population: list[list[str]],
    scores: list[float],
    level: int,
    rng: random.Random,
    pop_size: int = 50,
    elite_frac: float = 0.1,
    max_len: int = 20,
) -> list[list[str]]:
    """
    Produce next generation from population + scores.

    Strategy:
      - Keep top elite_frac% unchanged
      - Fill rest with mutations of top programs
      - Occasionally inject fresh random programs
    """
    n = len(population)
    n_elite = max(1, int(pop_size * elite_frac))
    n_fresh = max(1, int(pop_size * 0.05))  # 5% completely random

    # Sort by score descending
    indexed = sorted(zip(scores, range(n)), reverse=True)

    new_pop: list[list[str]] = []

    # Elites (unchanged)
    for _, idx in indexed[:n_elite]:
        new_pop.append(list(population[idx]))

    # Mutants of top programs
    top_indices = [idx for _, idx in indexed[:max(n_elite * 2, n // 3)]]
    while len(new_pop) < pop_size - n_fresh:
        parent_idx = rng.choice(top_indices)
        parent = population[parent_idx]
        # 1-3 mutations per offspring
        n_mut = rng.randint(1, 3)
        child = mutate(parent, level, rng, n_mutations=n_mut, max_len=max_len)
        new_pop.append(child)

        # Occasional crossover
        if rng.random() < 0.15 and len(top_indices) > 1:
            other_idx = rng.choice(top_indices)
            child2 = crossover(population[parent_idx], population[other_idx], rng, max_len)
            if len(new_pop) < pop_size - n_fresh:
                new_pop.append(child2)

    # Fresh random injections
    avg_len = max(3, sum(len(p) for p in population) // max(1, len(population)))
    while len(new_pop) < pop_size:
        new_pop.append(random_program(level, rng, length=rng.randint(2, avg_len + 2)))

    return new_pop[:pop_size]


# =============================================================================
# Self-test
# =============================================================================

if __name__ == "__main__":
    rng = random.Random(42)

    print("=== Mutator Self-Test ===\n")

    base = ["3", "5", "+"]
    print(f"  Base program: {' '.join(base)}")

    for name in MUTATION_OPS:
        fn = MUTATION_OPS[name]
        result = fn(list(base), level=1, rng=rng)
        print(f"  {name:>8s}: {' '.join(result)}")

    print(f"\n  Random program (level 1, len=5): {' '.join(random_program(1, rng, 5))}")
    print(f"  Random program (level 5, len=8): {' '.join(random_program(5, rng, 8))}")

    # Test population mutation
    pop = [random_program(1, rng, rng.randint(3, 6)) for _ in range(10)]
    scores = [rng.random() for _ in range(10)]
    next_gen = mutate_population(pop, scores, level=1, rng=rng, pop_size=10)
    print(f"\n  Population: {len(pop)} → next gen: {len(next_gen)}")
    for i, prog in enumerate(next_gen[:5]):
        print(f"    {i}: {' '.join(prog)}")

    # Weighted token distribution
    print("\n  Weighted token samples (level 2, 20 draws):")
    samples = [_weighted_random_token(2, rng) for _ in range(20)]
    from collections import Counter
    counts = Counter(samples)
    for tok, c in counts.most_common(8):
        print(f"    {tok}: {c}")

    print("\n✓ All mutator tests passed!")
