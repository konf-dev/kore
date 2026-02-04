# Kore mem-set/rom-set API Change

## Summary

The `mem-set` and `rom-set` tools have been updated to use `(value key --)` signature
to match `def`'s `(body name --)` pattern. This follows Kore's principle of consistency.

## Old Syntax (DEPRECATED)
```kore
"x" 42 mem-set       # key then value
"config" {...} rom-set
```

## New Syntax (CURRENT)
```kore
42 "x" mem-set       # value then key (like def)
{...} "config" rom-set
```

## Why This Change?

1. **Consistency with def**: `[dup mul] "square" def` puts body first, name second
2. **Mathematical soundness**: Tools follow uniform (value, name) convention
3. **Stack intuition**: The thing you're naming stays on stack until you name it

## Migration Patterns

### Simple assignment
```kore
# Old: "x" 42 mem-set
# New: 42 "x" mem-set
```

### Increment pattern
```kore
# Old: "x" "x" mem-get 1 add mem-set
# New: "x" mem-get 1 add "x" mem-set
```

### Swap pattern (function result)
```kore
# Old: "result" swap mem-set  
# New: swap "result" mem-set   # or just: "result" mem-set if only one value
```

## Files Requiring Migration

The following experiment and example files use old syntax:
- experiments/kore-synth/benchmark.kore (132 occurrences)
- experiments/kore-synth/continuous-learning.kore (122 occurrences)
- experiments/kore-synth/self-improving.kore (104 occurrences)
- experiments/kore-synth/real-world-challenges.kore (82 occurrences)
- experiments/kore-synth/discover.kore (61 occurrences)
- experiments/kore-synth/typed-search.kore (53 occurrences)
- examples/algorithms.kore (16 occurrences)
- And others...

## Already Migrated
- experiments/kore-synth/persistent-learning.kore ✓
- All documentation files ✓
