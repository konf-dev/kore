# Kore: Formal Mathematical Foundations

A rigorous mathematical treatment of Kore's computational model, establishing its algebraic structure, type theory, and connections to existing mathematical frameworks.

---

## Table of Contents

1. [Syntactic Structure](#1-syntactic-structure)
2. [Algebraic Semantics](#2-algebraic-semantics)
3. [Operational Semantics](#3-operational-semantics)
4. [Denotational Semantics](#4-denotational-semantics)
5. [Capability Logic](#5-capability-logic)
6. [Resource Algebra](#6-resource-algebra)
7. [Trace Semantics](#7-trace-semantics)
8. [Type Theory](#8-type-theory)
9. [Categorical Semantics](#9-categorical-semantics)
10. [Connections to Established Theories](#10-connections-to-established-theories)
11. [Theorems and Proofs](#11-theorems-and-proofs)
12. [Open Questions](#12-open-questions)
13. [References](#13-references)

---

## 1. Syntactic Structure

### 1.1 Abstract Syntax

**Definition 1.1 (Kore Terms)**

The set of Kore terms $\mathcal{T}$ is defined inductively:

$$
\begin{aligned}
t ::= &\ v                    && \text{(value)} \\
    | &\ w                    && \text{(word/primitive)} \\
    | &\ [t_1 \cdots t_n]     && \text{(quotation)} \\
    | &\ t_1\ t_2             && \text{(concatenation)}
\end{aligned}
$$

**Definition 1.2 (Values)**

The set of values $\mathcal{V}$ consists of:

$$
\mathcal{V} = \mathbb{Z} \cup \mathbb{R} \cup \mathbb{B} \cup \mathcal{S} \cup \mathcal{Q} \cup \mathcal{L} \cup \mathcal{M} \cup \mathcal{E} \cup \mathcal{C} \cup \{\mathbf{nil}\}
$$

Where:
- $\mathbb{Z}$ = integers
- $\mathbb{R}$ = floating point numbers
- $\mathbb{B} = \{\mathbf{true}, \mathbf{false}\}$ = booleans
- $\mathcal{S}$ = strings (sequences over Unicode)
- $\mathcal{Q}$ = quotations (suspended computations)
- $\mathcal{L}$ = lists
- $\mathcal{M}$ = maps (finite partial functions)
- $\mathcal{E}$ = errors
- $\mathcal{C}$ = capabilities
- $\mathbf{nil}$ = unit/null value

**Definition 1.3 (Programs)**

A Kore program $P$ is a finite sequence of terms:

$$
P = t_1\ t_2\ \cdots\ t_n \in \mathcal{T}^*
$$

### 1.2 Concrete Syntax

The concrete syntax maps to abstract syntax via a parsing function:

$$
\mathsf{parse} : \Sigma^* \rightharpoonup \mathcal{T}^*
$$

Where $\Sigma$ is the character alphabet. The function is partial because not all strings are valid programs.

**Key Syntactic Property (LLM-Native):**

For any prefix $s_1$ of a valid program string $s$:
$$
\mathsf{parse}(s_1) = \mathsf{error} \implies \exists s_2.\ \mathsf{parse}(s_1 \cdot s_2) \neq \mathsf{error}
$$

This means partial parses can always be completed—essential for streaming LLM output.

---

## 2. Algebraic Semantics

### 2.1 The Concatenative Monoid

**Theorem 2.1 (Kore Programs Form a Monoid)**

The set of Kore programs $\mathcal{T}^*$ with concatenation $(\cdot)$ and the empty program $\epsilon$ forms a monoid:

$$
(\mathcal{T}^*, \cdot, \epsilon)
$$

*Proof:*
1. **Closure**: $P_1 \cdot P_2 \in \mathcal{T}^*$ for all $P_1, P_2 \in \mathcal{T}^*$
2. **Associativity**: $(P_1 \cdot P_2) \cdot P_3 = P_1 \cdot (P_2 \cdot P_3)$
3. **Identity**: $\epsilon \cdot P = P \cdot \epsilon = P$

$\square$

**Definition 2.2 (Stack Transformation)**

A stack is a finite sequence of values:
$$
\mathcal{S}tack = \mathcal{V}^*
$$

A stack transformation is a partial function:
$$
f : \mathcal{S}tack \rightharpoonup \mathcal{S}tack
$$

**Theorem 2.2 (Stack Transformations Form a Monoid)**

Stack transformations with composition and identity form a monoid:
$$
(\mathcal{S}tack \rightharpoonup \mathcal{S}tack, \circ, \mathsf{id})
$$

### 2.2 The Semantic Homomorphism

**Definition 2.3 (Denotation Function)**

The meaning of a program is a homomorphism from programs to stack transformations:

$$
\llbracket \cdot \rrbracket : \mathcal{T}^* \to (\mathcal{S}tack \rightharpoonup \mathcal{S}tack)
$$

Satisfying:
$$
\begin{aligned}
\llbracket \epsilon \rrbracket &= \mathsf{id} \\
\llbracket P_1 \cdot P_2 \rrbracket &= \llbracket P_2 \rrbracket \circ \llbracket P_1 \rrbracket
\end{aligned}
$$

Note: The order reversal ($P_2 \circ P_1$) reflects that $P_1$ executes first, then $P_2$.

**Corollary 2.3 (Compositionality)**

The meaning of a program is determined entirely by the meanings of its parts:
$$
\llbracket P_1 \cdot P_2 \rrbracket(s) = \llbracket P_2 \rrbracket(\llbracket P_1 \rrbracket(s))
$$

This is the **fundamental theorem of concatenative languages**. [1]

### 2.3 Quotations as Higher-Order Structure

**Definition 2.4 (Quotation Semantics)**

A quotation $[P]$ pushes a suspended computation onto the stack:

$$
\llbracket [P] \rrbracket(s) = s \cdot \langle P \rangle
$$

Where $\langle P \rangle \in \mathcal{Q}$ is the quotation value containing $P$.

**Definition 2.5 (Apply/Call Semantics)**

The `call` primitive applies a quotation:

$$
\llbracket \mathsf{call} \rrbracket(s \cdot \langle P \rangle) = \llbracket P \rrbracket(s)
$$

**Theorem 2.4 (Quotations Enable Abstraction)**

For any program $P$, there exists a quotation $[P]$ such that:
$$
\llbracket [P]\ \mathsf{call} \rrbracket = \llbracket P \rrbracket
$$

This gives Kore the power of higher-order functions.

---

## 3. Operational Semantics

### 3.1 Small-Step Semantics

**Definition 3.1 (Configuration)**

A configuration is a tuple:
$$
\gamma = (P, s, C, R, \tau) \in \mathcal{T}^* \times \mathcal{S}tack \times \mathcal{C}ap \times \mathcal{R}es \times \mathcal{T}race
$$

Where:
- $P$ = program (remaining code)
- $s$ = stack
- $C$ = capability set
- $R$ = resource state
- $\tau$ = trace (execution history)

**Definition 3.2 (Transition Relation)**

The small-step transition relation $\to$ is defined by rules:

**Push Rule:**
$$
\frac{v \in \mathcal{V}}{(v \cdot P,\ s,\ C,\ R,\ \tau) \to (P,\ s \cdot v,\ C,\ R,\ \tau \cdot \mathsf{push}(v))}
$$

**Call Rule:**
$$
\frac{w \in \mathcal{W} \quad \mathsf{requires}(w) \subseteq C \quad \mathsf{cost}(w) \leq R}{(w \cdot P,\ s,\ C,\ R,\ \tau) \to (P,\ \delta_w(s),\ C,\ R - \mathsf{cost}(w),\ \tau \cdot \mathsf{call}(w))}
$$

Where $\delta_w$ is the primitive's stack effect and $\mathcal{W}$ is the set of primitives.

**Quote Rule:**
$$
\frac{}{([P'] \cdot P,\ s,\ C,\ R,\ \tau) \to (P,\ s \cdot \langle P' \rangle,\ C,\ R,\ \tau \cdot \mathsf{quote}(P'))}
$$

**Apply Rule:**
$$
\frac{}{(\mathsf{call} \cdot P,\ s \cdot \langle P' \rangle,\ C,\ R,\ \tau) \to (P' \cdot P,\ s,\ C,\ R,\ \tau \cdot \mathsf{apply})}
$$

**If-True Rule:**
$$
\frac{}{(\mathsf{if} \cdot P,\ s \cdot \mathbf{true} \cdot \langle P_t \rangle \cdot \langle P_f \rangle,\ C,\ R,\ \tau) \to (P_t \cdot P,\ s,\ C,\ R,\ \tau \cdot \mathsf{if\text{-}true})}
$$

**If-False Rule:**
$$
\frac{}{(\mathsf{if} \cdot P,\ s \cdot \mathbf{false} \cdot \langle P_t \rangle \cdot \langle P_f \rangle,\ C,\ R,\ \tau) \to (P_f \cdot P,\ s,\ C,\ R,\ \tau \cdot \mathsf{if\text{-}false})}
$$

### 3.2 Termination and Results

**Definition 3.3 (Terminal Configuration)**

A configuration $(\epsilon, s, C, R, \tau)$ is terminal (program exhausted).

**Definition 3.4 (Execution)**

An execution is a (possibly infinite) sequence of transitions:
$$
\gamma_0 \to \gamma_1 \to \gamma_2 \to \cdots
$$

**Definition 3.5 (Successful Execution)**

Execution is successful if it reaches a terminal configuration:
$$
\gamma_0 \to^* (\epsilon, s_{final}, C, R_{final}, \tau_{final})
$$

---

## 4. Denotational Semantics

### 4.1 Domain Structure

**Definition 4.1 (Semantic Domains)**

$$
\begin{aligned}
\mathbf{Val} &= \mathbb{Z}_\bot + \mathbb{R}_\bot + \mathbb{B}_\bot + \mathbf{Str}_\bot + \mathbf{Quot}_\bot + \mathbf{List}_\bot + \mathbf{Map}_\bot + \mathbf{Err}_\bot + \mathbf{Cap}_\bot + \{\mathbf{nil}\}_\bot \\
\mathbf{Stack} &= \mathbf{Val}^* \\
\mathbf{Conf} &= \mathbf{Stack} \times \mathcal{P}(\mathbf{Cap}) \times \mathbf{Res} \times \mathbf{Trace}
\end{aligned}
$$

The $\bot$ subscript denotes lifting with a bottom element for non-termination.

**Definition 4.2 (Continuation Semantics)**

Using continuation-passing style for proper handling of control flow:

$$
\llbracket P \rrbracket : \mathbf{Conf} \to (\mathbf{Conf} \to \mathbf{Answer}) \to \mathbf{Answer}
$$

Where $\mathbf{Answer}$ is the final result domain.

### 4.2 Semantic Equations

$$
\begin{aligned}
\llbracket \epsilon \rrbracket\ \sigma\ k &= k\ \sigma \\
\llbracket v \cdot P \rrbracket\ (s, C, R, \tau)\ k &= \llbracket P \rrbracket\ (s \cdot v, C, R, \tau \cdot \mathsf{push}(v))\ k \\
\llbracket w \cdot P \rrbracket\ (s, C, R, \tau)\ k &= 
  \begin{cases}
    \llbracket P \rrbracket\ (\delta_w(s), C, R', \tau')\ k & \text{if } \mathsf{check}(w, C, R) \\
    \mathbf{error} & \text{otherwise}
  \end{cases}
\end{aligned}
$$

---

## 5. Capability Logic

### 5.1 Capability Algebra

**Definition 5.1 (Capability)**

A capability is a pair:
$$
c = (\mathsf{kind}, \mathsf{scope}) \in \mathcal{K} \times \mathcal{S}cope
$$

Where:
- $\mathcal{K} = \{\mathsf{fs}, \mathsf{net}, \mathsf{exec}, \mathsf{spawn}, \mathsf{env}, \ldots\}$
- $\mathcal{S}cope$ varies by kind (paths, addresses, etc.)

**Definition 5.2 (Capability Set)**

A capability set is:
$$
C \in \mathcal{P}(\mathcal{C}ap)
$$

**Definition 5.3 (Capability Ordering)**

Capabilities form a partial order under attenuation:
$$
c_1 \leq c_2 \iff \mathsf{scope}(c_1) \subseteq \mathsf{scope}(c_2) \land \mathsf{kind}(c_1) = \mathsf{kind}(c_2)
$$

A more restricted capability is "smaller."

**Theorem 5.1 (Capability Lattice)**

$(\mathcal{P}(\mathcal{C}ap), \subseteq, \cap, \cup, \emptyset, \mathcal{C}ap)$ forms a bounded lattice.

### 5.2 Capability Logic

**Definition 5.4 (Capability Assertions)**

We define a logic for reasoning about capabilities:

$$
\begin{aligned}
\phi ::= &\ c \in C         && \text{(has capability)} \\
      | &\ \phi_1 \land \phi_2  && \text{(conjunction)} \\
      | &\ \phi_1 \lor \phi_2   && \text{(disjunction)} \\
      | &\ \neg \phi            && \text{(negation)} \\
      | &\ C_1 \subseteq C_2    && \text{(subset)} \\
      | &\ \mathsf{requires}(w, C) && \text{(operation requires)}
\end{aligned}
$$

**Axiom 5.1 (Capability Monotonicity)**

If $C_1 \subseteq C_2$, then:
$$
\mathsf{executable}(P, C_1) \implies \mathsf{executable}(P, C_2)
$$

More capabilities never prevent execution.

**Axiom 5.2 (Spawn Attenuation)**

When spawning with capabilities $C'$ from capability set $C$:
$$
C' \subseteq C
$$

A spawned agent cannot have more capabilities than its creator.

**Theorem 5.2 (Capability Safety)**

For any execution starting with capabilities $C$:
$$
\forall \tau_i \in \tau.\ \mathsf{caps\text{-}used}(\tau_i) \subseteq C
$$

All capabilities used are contained in the granted set.

*Proof sketch:* By induction on execution steps. Each transition rule checks $\mathsf{requires}(w) \subseteq C$. $\square$

### 5.3 Connection to Object-Capability Model

Kore's capability system corresponds to the object-capability model [2]:

| Object-Capability Principle | Kore Implementation |
|---------------------------|---------------------|
| No ambient authority | Empty default capability set |
| Capabilities are unforgeable | Runtime-enforced, not in language |
| Capabilities travel with references | Explicit capability passing |
| Attenuation | $C' \subseteq C$ on spawn |

---

## 6. Resource Algebra

### 6.1 Resource Structure

**Definition 6.1 (Resource Quota)**

A resource quota is a tuple:
$$
R = (m, r, c, n) \in \mathbb{N}^4
$$

Where:
- $m$ = memory quota
- $r$ = ROM/storage quota  
- $c$ = compute quota
- $n$ = network quota

**Definition 6.2 (Resource Monoid)**

Resources form a commutative monoid under addition:
$$
(\mathbb{N}^4, +, \mathbf{0})
$$

With component-wise operations.

**Definition 6.3 (Resource Ordering)**

$$
R_1 \leq R_2 \iff \forall i.\ R_1[i] \leq R_2[i]
$$

### 6.2 Resource Consumption

**Definition 6.4 (Cost Function)**

Each primitive $w$ has a cost:
$$
\mathsf{cost} : \mathcal{W} \times \mathcal{S}tack \to \mathbb{N}^4
$$

Cost may depend on stack contents (e.g., string length for operations on strings).

**Definition 6.5 (Resource Transition)**

A transition consumes resources:
$$
\frac{\mathsf{cost}(w, s) \leq R}{(w \cdot P, s, C, R, \tau) \to (P, \delta_w(s), C, R - \mathsf{cost}(w, s), \tau')}
$$

**Theorem 6.1 (Resource Monotonicity)**

Resources decrease monotonically during execution:
$$
\gamma_0 \to^* \gamma_n \implies R_n \leq R_0
$$

*Proof:* Each transition subtracts non-negative cost. $\square$

**Theorem 6.2 (Termination under Bounded Resources)**

If $R_0$ is finite and all primitive costs are positive:
$$
\exists n.\ \gamma_0 \to^n \gamma_n \land \gamma_n \text{ is terminal or stuck}
$$

Execution cannot continue indefinitely.

### 6.3 Resource as Linear Logic

Resources connect to linear logic [3]:

| Linear Logic | Kore Resources |
|--------------|----------------|
| Linear proposition | Single-use resource |
| ! (of course) | Unlimited capability |
| ⊗ (tensor) | Resource combination |
| ⊸ (lollipop) | Resource transformation |

**Proposition 6.1:** Kore's resource system can be viewed as a fragment of linear logic where:
- Resources are linear propositions
- Operations consume and produce resources
- Capabilities are exponential (!C means unlimited use of C)

---

## 7. Trace Semantics

### 7.1 Trace Structure

**Definition 7.1 (Trace Event)**

A trace event is:
$$
e \in \mathcal{E}vent = \{\mathsf{push}(v), \mathsf{call}(w), \mathsf{quote}(P), \mathsf{apply}, \mathsf{if\text{-}true}, \mathsf{if\text{-}false}, \ldots\}
$$

**Definition 7.2 (Trace)**

A trace is a finite sequence of events:
$$
\tau \in \mathcal{T}race = \mathcal{E}vent^*
$$

**Definition 7.3 (Trace Monoid)**

Traces form a monoid under concatenation:
$$
(\mathcal{E}vent^*, \cdot, \epsilon)
$$

### 7.2 Trace Properties

**Definition 7.4 (Trace Projection)**

Project trace to capability usage:
$$
\pi_C(\tau) = \{c \mid \exists e \in \tau.\ c \in \mathsf{caps}(e)\}
$$

Project trace to resource consumption:
$$
\pi_R(\tau) = \sum_{e \in \tau} \mathsf{cost}(e)
$$

**Theorem 7.1 (Trace Completeness)**

The trace determines the execution:
$$
\tau_1 = \tau_2 \implies \gamma_1^{final} = \gamma_2^{final}
$$

(Given same initial configuration)

**Theorem 7.2 (Trace Verifiability)**

Given a trace $\tau$ and initial configuration $\gamma_0$, verification is decidable:
$$
\mathsf{verify}(\tau, \gamma_0) = 
\begin{cases}
  \mathbf{true} & \text{if } \gamma_0 \to^* \gamma_n \text{ produces } \tau \\
  \mathbf{false} & \text{otherwise}
\end{cases}
$$

Verification is linear in trace length.

### 7.3 Trace Equivalence

**Definition 7.5 (Trace Equivalence)**

Programs are trace-equivalent if they produce the same traces:
$$
P_1 \approx_\tau P_2 \iff \forall \gamma_0.\ \mathsf{trace}(P_1, \gamma_0) = \mathsf{trace}(P_2, \gamma_0)
$$

**Proposition 7.1:** Trace equivalence is finer than observational equivalence—trace-equivalent programs are observationally equivalent, but not vice versa.

---

## 8. Type Theory

### 8.1 Stack Effect Types

**Definition 8.1 (Stack Effect)**

A stack effect describes input/output stack shapes:
$$
\sigma = (A \to B)
$$

Where $A, B$ are stack type patterns.

**Definition 8.2 (Stack Type)**

$$
\begin{aligned}
S ::= &\ \epsilon           && \text{(empty stack)} \\
    | &\ S \cdot T          && \text{(stack with top)} \\
    | &\ \rho               && \text{(stack variable)}
\end{aligned}
$$

**Definition 8.3 (Value Type)**

$$
\begin{aligned}
T ::= &\ \mathsf{int} \mid \mathsf{float} \mid \mathsf{bool} \mid \mathsf{string} \\
    | &\ \mathsf{quot}(\sigma) && \text{(quotation with effect)} \\
    | &\ \mathsf{list}(T)      && \text{(list of T)} \\
    | &\ \mathsf{map}(K, V)    && \text{(map from K to V)} \\
    | &\ \mathsf{cap}(\kappa)  && \text{(capability of kind κ)} \\
    | &\ \alpha               && \text{(type variable)}
\end{aligned}
$$

### 8.2 Typing Rules

**Judgment Form:**
$$
\Gamma; C; R \vdash P : \sigma
$$

"Under context $\Gamma$, capabilities $C$, resources $R$, program $P$ has effect $\sigma$."

**Value Rule:**
$$
\frac{v : T}{\Gamma; C; R \vdash v : (\rho \to \rho \cdot T)}
$$

**Primitive Rule:**
$$
\frac{w : \sigma \quad \mathsf{requires}(w) \subseteq C \quad \mathsf{cost}(w) \leq R}{\Gamma; C; R \vdash w : \sigma}
$$

**Concatenation Rule:**
$$
\frac{\Gamma; C; R_1 \vdash P_1 : (A \to B) \quad \Gamma; C; R_2 \vdash P_2 : (B \to C)}{\Gamma; C; R_1 + R_2 \vdash P_1\ P_2 : (A \to C)}
$$

**Quotation Rule:**
$$
\frac{\Gamma; C; R \vdash P : \sigma}{\Gamma; C; 0 \vdash [P] : (\rho \to \rho \cdot \mathsf{quot}(\sigma))}
$$

(Quotation captures effect but costs nothing until called)

### 8.3 Type Safety

**Theorem 8.1 (Type Preservation)**

If $\Gamma; C; R \vdash P : \sigma$ and $(P, s, C, R, \tau) \to (P', s', C, R', \tau')$, then:
$$
\exists \sigma'.\ \Gamma; C; R' \vdash P' : \sigma'
$$

**Theorem 8.2 (Progress)**

If $\Gamma; C; R \vdash P : \sigma$ and $P \neq \epsilon$, then either:
1. $(P, s, C, R, \tau) \to (P', s', C, R', \tau')$ for some $P', s', R', \tau'$
2. $\mathsf{requires}(P) \not\subseteq C$ (capability error)
3. $\mathsf{cost}(P) > R$ (resource exhaustion)

---

## 9. Categorical Semantics

### 9.1 The Category of Kore Computations

**Definition 9.1 (Kore Category)**

Define category $\mathbf{Kore}$:
- **Objects:** Stack types
- **Morphisms:** Programs with stack effects
- **Identity:** Empty program $\epsilon$
- **Composition:** Program concatenation

**Theorem 9.1:** $\mathbf{Kore}$ is a category.

*Proof:* Follows from the monoid structure (Theorem 2.1). $\square$

### 9.2 Monoidal Structure

**Definition 9.2 (Tensor Product)**

The tensor product on stack types:
$$
S_1 \otimes S_2 = S_1 \cdot S_2
$$

**Theorem 9.2:** $(\mathbf{Kore}, \otimes, \epsilon)$ is a monoidal category.

### 9.3 Closed Structure

**Definition 9.3 (Internal Hom)**

The internal hom (function space):
$$
[A, B] = \mathsf{quot}(A \to B)
$$

**Theorem 9.3:** $\mathbf{Kore}$ is a closed monoidal category.

This means quotations provide exponential objects, giving full higher-order power.

### 9.4 Kleisli Category for Effects

**Definition 9.4 (Configuration Monad)**

Define monad $\mathbf{Conf}$ for configuration threading:
$$
\mathbf{Conf}(A) = \mathcal{C}ap \times \mathcal{R}es \times \mathcal{T}race \to A \times \mathcal{C}ap \times \mathcal{R}es \times \mathcal{T}race
$$

**Theorem 9.4:** Kore computations form the Kleisli category $\mathbf{Kore}_{\mathbf{Conf}}$.

---

## 10. Connections to Established Theories

### 10.1 Connection to Linear Logic

| Linear Logic [3] | Kore |
|-----------------|------|
| $A \otimes B$ | Stack with $A$ below $B$ |
| $A \multimap B$ | Quotation $(A \to B)$ |
| $!A$ | Capability (unlimited use) |
| Cut elimination | Program execution |

**Conjecture 10.1:** There exists a faithful functor from a fragment of linear logic to $\mathbf{Kore}$.

### 10.2 Connection to Process Calculi

| π-Calculus [4] | Kore |
|---------------|------|
| Process | Agent |
| Channel | Message queue |
| Name passing | Capability passing |
| Restriction | Spawn with attenuated capabilities |

**Proposition 10.1:** Kore's agent model can simulate a fragment of the π-calculus.

### 10.3 Connection to Separation Logic

| Separation Logic [5] | Kore |
|---------------------|------|
| $P * Q$ (separating conjunction) | Disjoint resource regions |
| $\mathsf{emp}$ | No resources |
| Frame rule | Resource addition preserves validity |

**Proposition 10.2:** Kore's resource system satisfies a form of the frame rule:
$$
\frac{\{R\}\ P\ \{R'\}}{\{R + R_0\}\ P\ \{R' + R_0\}}
$$

### 10.4 Connection to Effect Systems

| Algebraic Effects [6] | Kore |
|----------------------|------|
| Effect signature | Capability kind |
| Handler | Host environment |
| Operation | Primitive with capability |
| Effect row | Capability set |

---

## 11. Theorems and Proofs

### 11.1 Fundamental Theorems

**Theorem 11.1 (Capability Soundness)**

No execution can use a capability it wasn't granted:
$$
\forall (P, s_0, C, R, \epsilon) \to^* (P', s', C, R', \tau).\ \pi_C(\tau) \subseteq C
$$

*Proof:* By induction on derivation length. Base case: empty trace, $\pi_C(\epsilon) = \emptyset \subseteq C$. Inductive case: each rule checks $\mathsf{requires}(w) \subseteq C$ before adding to trace. $\square$

**Theorem 11.2 (Resource Boundedness)**

Execution is bounded by initial resources:
$$
\forall (P, s_0, C, R_0, \epsilon) \to^* (P', s', C, R', \tau).\ \pi_R(\tau) \leq R_0
$$

*Proof:* Each transition subtracts cost from resources and adds to trace. Total trace cost equals total consumption. Resources cannot go negative. $\square$

**Theorem 11.3 (Trace Determinism)**

Given the same initial configuration and the same program:
$$
\gamma_0 \to^* \gamma_n \land \gamma_0 \to^* \gamma'_n \implies \tau_n = \tau'_n
$$

*Proof:* Kore is deterministic—each configuration has at most one successor. $\square$

**Theorem 11.4 (Compositional Capability Analysis)**

Capability requirements compose:
$$
\mathsf{requires}(P_1 \cdot P_2) = \mathsf{requires}(P_1) \cup \mathsf{requires}(P_2)
$$

*Proof:* The requirements of a sequence are the union of component requirements. $\square$

### 11.2 Safety Theorems

**Theorem 11.5 (Type Safety)**

Well-typed programs don't go wrong (modulo capability/resource errors):
$$
\Gamma; C; R \vdash P : \sigma \implies P \text{ either terminates, exhausts resources, or needs more capabilities}
$$

**Theorem 11.6 (Spawn Safety)**

Spawned agents cannot exceed parent capabilities:
$$
\mathsf{spawn}(P, C') \text{ valid} \implies C' \subseteq C_{parent}
$$

**Theorem 11.7 (Trace Integrity)**

Traces cannot be forged—they are produced only by the runtime:
$$
\mathsf{valid\text{-}trace}(\tau) \iff \exists \gamma_0.\ \gamma_0 \to^* \gamma_n \text{ produces } \tau
$$

---

## 12. Open Questions

### 12.1 Theoretical Questions

1. **Full Abstraction:** Is the denotational semantics fully abstract with respect to observational equivalence?

2. **Decidability:** Is type inference decidable for the full type system with capability and resource effects?

3. **Completeness:** Can all computable functions be expressed in Kore? (Answer: yes, it's Turing-complete via quotations and recursion)

4. **Expressive Power:** What is the exact relationship between Kore and linear logic?

### 12.2 Practical Questions

1. **Optimal Resource Accounting:** Can we compute tighter resource bounds statically?

2. **Capability Inference:** Can minimal required capabilities be inferred automatically?

3. **Trace Compression:** What is the optimal representation for traces?

4. **Distributed Traces:** How should traces compose across agents?

### 12.3 Future Directions

1. **Dependent Types:** Adding types that depend on values for stronger guarantees.

2. **Session Types:** Formalizing communication protocols between agents.

3. **Temporal Logic:** Adding temporal operators for trace properties.

4. **Game Semantics:** Modeling agent interactions as games.

### 12.4 Deliberate Non-Goals

1. **Streaming/Lazy Evaluation:** Kore's formal model requires complete values:
   
   $$⟦P⟧ : Stack → Stack$$
   
   Every tool takes a complete stack state and produces a complete stack state. This means:
   - No partial values or lazy evaluation
   - No suspended computation mid-tool
   - No chunked/streaming primitives
   
   This is a deliberate trade-off: **formal simplicity over unbounded data support**. For streaming workloads, use external tools (Unix pipes, message queues) and integrate via `exec`.

2. **Callback Patterns:** Primitives like "iterate-and-apply" bundle two responsibilities and don't compose with stack semantics. Each primitive does exactly ONE thing.

---

## 13. References

[1] Brent Kerby. "The Theory of Concatenative Combinators." 2002.
http://tunes.org/~iepos/joy.html

[2] Mark S. Miller. "Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control." PhD Thesis, Johns Hopkins University, 2006.
http://www.erights.org/talks/thesis/

[3] Jean-Yves Girard. "Linear Logic." Theoretical Computer Science, 50:1-102, 1987.
https://doi.org/10.1016/0304-3975(87)90045-4

[4] Robin Milner. "Communicating and Mobile Systems: The π-Calculus." Cambridge University Press, 1999.
ISBN: 978-0521658690

[5] John Reynolds. "Separation Logic: A Logic for Shared Mutable Data Structures." LICS 2002.
https://doi.org/10.1109/LICS.2002.1029817

[6] Andrej Bauer and Matija Pretnar. "Programming with Algebraic Effects and Handlers." Journal of Logical and Algebraic Methods in Programming, 2015.
https://doi.org/10.1016/j.jlamp.2014.02.001

[7] Gul Agha. "Actors: A Model of Concurrent Computation in Distributed Systems." MIT Press, 1986.
ISBN: 978-0262010924

[8] Manfred Broy. "Compositional Refinement of Interactive Systems." Journal of the ACM, 1997.
https://doi.org/10.1145/263867.263872

[9] Martín Abadi and Luca Cardelli. "A Theory of Objects." Springer, 1996.
ISBN: 978-0387947754

[10] Benjamin Pierce. "Types and Programming Languages." MIT Press, 2002.
ISBN: 978-0262162098

[11] Gordon Plotkin. "A Structural Approach to Operational Semantics." DAIMI Report, Aarhus University, 1981.

[12] Eugenio Moggi. "Notions of Computation and Monads." Information and Computation, 1991.
https://doi.org/10.1016/0890-5401(91)90052-4

[13] Dennis and Van Horn. "Programming Semantics for Multiprogrammed Computations." Communications of the ACM, 1966.
https://doi.org/10.1145/365230.365252

---

## Appendix A: Notation Summary

| Symbol | Meaning |
|--------|---------|
| $\mathcal{T}$ | Set of terms |
| $\mathcal{V}$ | Set of values |
| $\mathcal{W}$ | Set of primitives |
| $\mathcal{S}tack$ | Stack domain |
| $\mathcal{C}ap$ | Capability domain |
| $\mathcal{R}es$ | Resource domain |
| $\mathcal{T}race$ | Trace domain |
| $\llbracket \cdot \rrbracket$ | Semantic function |
| $\to$ | Small-step transition |
| $\to^*$ | Reflexive transitive closure |
| $\vdash$ | Typing judgment |
| $\gamma$ | Configuration |
| $\sigma$ | Stack effect type |
| $\tau$ | Trace |
| $\otimes$ | Tensor/conjunction |
| $\multimap$ | Linear implication |

---

## Appendix B: Primitive Stack Effects

Selected primitives with their formal stack effects:

| Primitive | Stack Effect | Cost | Capabilities |
|-----------|-------------|------|--------------|
| `+` | $(\rho \cdot \mathsf{int} \cdot \mathsf{int} \to \rho \cdot \mathsf{int})$ | $(0,0,1,0)$ | $\emptyset$ |
| `dup` | $(\rho \cdot \alpha \to \rho \cdot \alpha \cdot \alpha)$ | $(0,0,1,0)$ | $\emptyset$ |
| `drop` | $(\rho \cdot \alpha \to \rho)$ | $(0,0,1,0)$ | $\emptyset$ |
| `swap` | $(\rho \cdot \alpha \cdot \beta \to \rho \cdot \beta \cdot \alpha)$ | $(0,0,1,0)$ | $\emptyset$ |
| `call` | $(\rho \cdot \mathsf{quot}(A \to B) \to B)$ where stack matches $A$ | varies | $\emptyset$ |
| `if` | $(\rho \cdot Q \cdot Q \cdot \mathsf{bool} \to \rho')$ | varies | $\emptyset$ |
| `fs-read` | $(\rho \cdot \mathsf{string} \to \rho \cdot \mathsf{string})$ | $(0,0,1,n)$ | $\{\mathsf{fs:read}\}$ |
| `spawn` | $(\rho \cdot Q \cdot C \cdot R \to \rho \cdot \mathsf{pid})$ | $(0,0,1,0)$ | $\{\mathsf{spawn}\}$ |

---

*This document establishes the mathematical foundations of Kore. The formal framework enables rigorous reasoning about program behavior, capability safety, resource bounds, and trace properties. Future work will extend these foundations with dependent types, session types, and mechanized proofs.*
