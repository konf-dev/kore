# Mathematical Foundations: Why Kore Enables Novel RL

> **The key insight**: Kore's algebraic structure transforms program synthesis
> from an intractable search problem into a structured optimization problem
> with computable rewards at every step.

## 1. The Fundamental Problem with Code RL

### 1.1 Why Python/JS RL is Hard

For conventional languages, the reward function is:

$$R(p) = \begin{cases} 1 & \text{if } p \text{ produces correct output} \\ 0 & \text{otherwise} \end{cases}$$

Problems:
1. **Sparse reward**: Only final correctness matters
2. **Undecidable properties**: "Does this halt?" is uncomputable
3. **Semantic opacity**: Can't analyze behavior without running
4. **Security risk**: Must sandbox arbitrary code execution

### 1.2 Kore's Advantage

Kore programs have computable static properties:

$$\text{effect}: \text{Program} \to \mathbb{N} \times \mathbb{N} \times \mathcal{P}(\text{IO})$$

This gives us:
- **Dense reward**: Effect similarity at every step
- **Decidable safety**: Bounded stack, declared IO
- **Semantic transparency**: Behavior inferable from effect
- **Safe exploration**: Pure programs can't cause harm

---

## 2. The Effect Algebra

### 2.1 Stack Effects as a Monoid

Stack effects form a monoid $(E, \circ, \epsilon)$:

$$E = \{(c, p) \mid c, p \in \mathbb{N}\}$$

Where:
- $c$ = number of values consumed
- $p$ = number of values produced
- $\epsilon = (0, 0)$ = identity effect

Composition:

$$
(c_1, p_1) \circ (c_2, p_2) = \begin{cases}
(c_1, p_1 - c_2 + p_2) & \text{if } p_1 \geq c_2 \\
(c_1 + c_2 - p_1, p_2) & \text{if } p_1 < c_2
\end{cases}
$$

This is **not commutative** but is **associative**.

### 2.2 Effect Distance Metric

We define a metric on effect space:

$$d((c_1, p_1), (c_2, p_2)) = |c_1 - c_2| + |p_1 - p_2|$$

This satisfies metric axioms and gives us:
- Continuous optimization over discrete programs
- Natural curriculum: start with $d = 0$ targets, increase

### 2.3 IO Effects as a Semilattice

IO effects form a join-semilattice:

$$\text{IO} = \mathcal{P}(\{\texttt{fs}, \texttt{net}, \texttt{io}, \texttt{spawn}, ...\})$$

With:
- Join: $A \vee B = A \cup B$
- Bottom: $\bot = \emptyset$ (pure computation)

Monotonicity: $\text{IO}(A \circ B) = \text{IO}(A) \vee \text{IO}(B)$

---

## 3. Reward Shaping via Effect Potential

### 3.1 Potential-Based Shaping

Given target effect $e^*$, define potential:

$$\Phi(e) = -d(e, e^*)$$

Shaped reward:

$$R'(s, a, s') = R(s, a, s') + \gamma \Phi(s') - \Phi(s)$$

**Theorem (Ng et al., 1999)**: Potential-based shaping preserves optimal policy.

### 3.2 Why This Works for Kore

For each partial program $p_t$, we can compute:
- Current effect: $e_t = \text{effect}(p_t)$
- Potential: $\Phi(e_t) = -d(e_t, e^*)$
- Shaping signal: $\gamma \Phi(e_{t+1}) - \Phi(e_t)$

This gives **per-token reward** instead of end-of-sequence.

### 3.3 Effect-Guided Generation

During generation, we can:
1. Generate candidate token $t$
2. Compute effect if we add $t$: $e' = e_t \circ \text{effect}(t)$
3. Use $\Phi(e')$ to bias sampling

This is like A* search over program space with effect distance as heuristic.

---

## 4. Intrinsic Motivation via Effect Novelty

### 4.1 Effect Space Coverage

Define coverage at time $T$:

$$C_T = \{e_1, e_2, ..., e_T\} \subseteq E$$

Novelty reward:

$$R_{\text{novelty}}(e) = \min_{e' \in C_T} d(e, e')$$

### 4.2 Convergence to Full Coverage

**Theorem**: Under effect-novelty reward with $\epsilon$-greedy exploration:

$$\lim_{T \to \infty} \frac{|C_T|}{|E_{\leq k}|} = 1$$

Where $E_{\leq k}$ is the set of effects achievable by programs of length $\leq k$.

**Proof sketch**: 
- Effect space is finite for bounded stack depth
- Novelty reward is positive for unseen effects
- $\epsilon$-greedy ensures all effects eventually visited
- By coupon collector argument, coverage is $O(|E| \log |E|)$

### 4.3 Why Effect Novelty > Syntax Novelty

Let $P_k$ = programs of length $\leq k$, $E_k$ = their effects.

$$|P_k| = O(|V|^k)$$ where $|V|$ = vocabulary size
$$|E_k| = O(k^2 \cdot 2^{|\text{IO}|})$$ 

The ratio:

$$\frac{|P_k|}{|E_k|} = O\left(\frac{|V|^k}{k^2 \cdot 2^{|\text{IO}|}}\right) \to \infty$$

Effect-space exploration is **exponentially more efficient** than program-space.

---

## 5. Algebraic Equivalence Discovery

### 5.1 Equivalence as Kernel

Define equivalence relation:

$$p_1 \sim p_2 \iff \forall s. \text{exec}(p_1, s) = \text{exec}(p_2, s)$$

The quotient $P / {\sim}$ captures semantic programs.

### 5.2 Testable Equivalence

**Theorem**: For stack programs over finite domains, equivalence is testable:

$$p_1 \sim p_2 \iff \forall s \in S_{\text{test}}. \text{exec}(p_1, s) = \text{exec}(p_2, s)$$

Where $S_{\text{test}}$ is finite test set covering behavior.

For Kore with depth $\leq d$ and values in $[-M, M]$:

$$|S_{\text{test}}| \leq (2M + 1)^d$$

### 5.3 Discovery Reward

For programs $p_1$, $p_2$ with $|p_1| > |p_2|$ and $p_1 \sim p_2$:

$$R_{\text{equiv}} = \frac{|p_1|}{|p_2|} - 1$$

This rewards discovering compression, i.e., algebraic identities.

---

## 6. Linear Types as Resource Games

### 6.1 Linear Logic Interpretation

Linear values in Kore correspond to linear logic:

| Kore | Linear Logic |
|------|--------------|
| `linear-new` | $!A \multimap A$ |
| `linear-unwrap` | $A \multimap !A$ |
| `dup` (fails) | No contraction |
| `drop` (fails) | No weakening |

### 6.2 Resource Game Formulation

State: $(S, L)$ where $S$ = stack, $L$ = multiset of linear resources

Transitions:
- `linear-new`: $(S, L) \to (S \cdot v, L \cup \{v\})$
- `linear-unwrap`: $(S \cdot v, L) \to (S \cdot \text{unwrap}(v), L \setminus \{v\})$

Goal: Reach $(S', \emptyset)$ — all resources consumed.

### 6.3 Reward Shaping for Resources

Potential based on remaining resources:

$$\Phi(L) = -|L|$$

Reward for consuming resource: $+1$
Reward for violating linearity: $-\infty$ (invalid state)

This naturally teaches resource management.

---

## 7. Capability Descent as Lattice Optimization

### 7.1 Capability Lattice

Capabilities form a bounded lattice:

$$\mathcal{C} = (\mathcal{P}(\text{Caps}), \subseteq, \emptyset, \text{All})$$

With:
- Meet: $C_1 \wedge C_2 = C_1 \cap C_2$
- Join: $C_1 \vee C_2 = C_1 \cup C_2$

### 7.2 Minimal Capability Problem

Given task $T$ and program $p$ solving $T$:

$$\text{MinCap}(T) = \bigwedge \{C \mid \exists p. \text{solves}(p, T) \land \text{caps}(p) \leq C\}$$

This is the **minimal capability** needed to solve $T$.

### 7.3 Descent Curriculum

Training schedule:
1. Start with $C_0 = \text{All}$
2. Agent solves tasks under $C_0$
3. Attenuate: $C_{t+1} = C_t \setminus \{c_{\text{unused}}\}$
4. Repeat until $C_t = \text{MinCap}(T)$

Reward:

$$R = \mathbf{1}[\text{solves}(p, T)] \cdot (1 + \alpha |\text{All} \setminus \text{caps}(p)|)$$

Agent is rewarded for solving with fewer capabilities.

---

## 8. Gradient Flow Analysis

### 8.1 Policy Gradient Decomposition

For policy $\pi_\theta$ generating programs $p$:

$$\nabla_\theta J = \mathbb{E}_{p \sim \pi_\theta} \left[ \nabla_\theta \log \pi_\theta(p) \cdot R(p) \right]$$

With Kore's structured rewards:

$$R(p) = R_{\text{task}}(p) + \alpha R_{\text{effect}}(p) + \beta R_{\text{cap}}(p) + \gamma R_{\text{novelty}}(p)$$

### 8.2 Non-Interference Property

**Claim**: The reward components provide independent gradient signals.

**Justification**:
- $R_{\text{task}}$: Depends on execution result
- $R_{\text{effect}}$: Depends on $(c, p)$ signature
- $R_{\text{cap}}$: Depends on IO set
- $R_{\text{novelty}}$: Depends on memory state

These operate on orthogonal aspects of the program, reducing gradient interference.

### 8.3 Variance Reduction

Effect-based rewards have lower variance than execution-based:

$$\text{Var}[R_{\text{effect}}] \ll \text{Var}[R_{\text{task}}]$$

Because:
- Effect is deterministic given program
- Effect space is smaller than output space
- Effect can be computed for invalid programs (partial credit)

---

## 9. Theoretical Guarantees

### 9.1 Sample Complexity

**Theorem**: Under effect-novelty reward, the agent achieves $\epsilon$-coverage of $E_k$ in:

$$T = O\left(\frac{|E_k| \log |E_k|}{\epsilon}\right)$$

samples, compared to $O(|P_k| \log |P_k|)$ for syntax-based exploration.

### 9.2 Regret Bounds

For capability descent with $n$ capability levels:

$$\text{Regret}(T) = O\left(\sqrt{nT \log |A|}\right)$$

where $|A|$ = action space size.

### 9.3 Convergence of Self-Play

For self-play composition with effect targets:

**Theorem**: If the model has sufficient capacity, self-play converges to a policy that can compose programs to achieve any reachable effect.

---

## 10. Summary: What Kore Enables

| Property | Conventional RL | Kore RL |
|----------|-----------------|---------|
| Reward density | Sparse (end-of-episode) | Dense (per-token via effect) |
| Safety | Sandboxing required | Pure programs are safe |
| Novelty metric | Syntactic | Semantic (effect space) |
| Equivalence | Undecidable | Testable |
| Resource tracking | Manual | Linear types |
| Capability control | Ad-hoc | Lattice-theoretic |
| Gradient signal | High variance | Low variance |

**The fundamental insight**: Kore's algebraic structure provides mathematical handles for reward shaping, curriculum design, and exploration that simply don't exist for conventional languages.

---

## References

1. Ng, A. Y., Harada, D., & Russell, S. (1999). Policy invariance under reward transformations: Theory and application to reward shaping.
2. Girard, J. Y. (1987). Linear logic.
3. Wadler, P. (1990). Linear types can change the world!
4. Pathak, D., et al. (2017). Curiosity-driven exploration by self-supervised prediction.
5. Burda, Y., et al. (2018). Exploration by random network distillation.
