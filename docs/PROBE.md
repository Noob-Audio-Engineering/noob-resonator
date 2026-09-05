# The out-of-tree probe

`cargo run --release --bin benchmark -- --dump series` writes one line per
partial — `id,object,i,j,ratio` behind a header row — for every object, the two
air columns included. A separate program, about four hundred lines of Python
written from the published formulae and sharing no line with this repository,
computes the same series for itself and diffs the two.

It lives at `tools/physics_probe.py`, and is run as:

```sh
cargo run --release --bin benchmark -- --dump series > series.csv
python tools/physics_probe.py --compare series.csv
```

**The dump's rows are sorted by ratio and complete up to the last of them**,
and each object in the probe states the range it is checked over. Both of those
are recent and both were bugs first; see *What it caught* below.

**Its independence is in the implementation, not the directory.** It began in a
session scratchpad on the reasoning that being outside the crate made it
independent, which confused location with dependence: sharing no line of code
is what makes it a second opinion, and a scratchpad only makes it a second
opinion nobody has tomorrow. It is in the repository now and just as
independent.

The reason for keeping it separate at all is on this project's record rather
than a matter of taste: an audit found **nine tests across five plug-ins** that had been
written to assert a model's own output instead of the figure they existed to
check, one of which compared an estimate with itself. A number published about a
plug-in's own partial series has to be reproducible by something that could have
disagreed with it.

## What it shares with this crate: nothing

It does not import, link against or read any Rust. Every quantity is computed
from a different algorithm, so a mistake would have to be made twice, in two
languages, in two different ways:

| quantity | this crate | the probe |
|---|---|---|
| `J_m(x)` | Abramowitz and Stegun's rational and asymptotic approximations for `J₀` and `J₁`, then a recurrence — upward above the turning point, Miller's downward below it | the integral representation `J_m(x) = (1/π)∫₀^π cos(mτ − x sin τ)dτ`, by the trapezoid rule with the panel count scaled to the argument |
| Bessel zeros | bisection inside the bracket the interlacing property `j_{m,n} < j_{m+1,n} < j_{m,n+1}` guarantees | a linear scan for sign changes, then bisection |
| beam eigenvalues | Newton on `cos β − sech β`, from the asymptote | bisection on the same function, from a bracket round the asymptote |
| the beam's mode shape | the textbook form algebraically rearranged so `1 − σ` never has to be computed as a difference of two numbers that agree to `e^−β` | the textbook form written out directly, which is exact at the low mode numbers it is used at |
| membrane and plate lattices | a lazy walk with early exits | a full enumeration and a sort |

## It checks itself first

Before it is used on anything of ours, it reproduces every published value the
series rest on. All of these pass:

| | published | the probe |
|---|---|---|
| beam eigenvalues β₁…β₆ | Leissa, NASA SP-160 Table 4.23 | agree to 5 × 10⁻⁷ |
| `j₀,₁` | Abramowitz & Stegun Table 9.5, 2.404825558 | 3 × 10⁻¹⁰ |
| `j₁,₁` | same, 3.831705970 | 2 × 10⁻¹⁰ |
| circular membrane ratios 1.593, 2.135, 2.295, 2.917, 3.598 | Russell, Penn State | 6 × 10⁻⁴ |
| free bar ratios 1 : 2.7565 : 5.4039 : 8.9330 : 13.3443 | `MODAL.md` §2.3 | 6 × 10⁻⁵ |
| a piano C4's partials 8, 16 and 32 at `B = 3 × 10⁻⁴`: +16.5, +64.1, +231.9 cents | Lehtonen et al., DAFx-08 eq. (2) | 0.05 cents |
| square membrane and square plate ratios | elementary | 3 × 10⁻⁸ |

It also integrates each mode shape's mean square over its own object and gets
**1.000000** for the beam, the string, the rectangular membrane and the disc,
which is the mass normalisation the mode gains depend on.

## And then it does not disagree

Across every object and every partial the two both cover — the beam, the tine,
the string, both membranes and both plates, **3,960 partials** — the worst
disagreement is **0.0001 cents**. A cent is roughly the threshold of pitch
discrimination, so that is four orders of magnitude inside anything audible and
is the accuracy of the probe's own quadrature rather than of this engine.

The **air columns** are checked too, and they are the one place a disagreement
is expected rather than feared. A delay loop with a third-order Lagrange
fractional delay disperses, so its agreement with an ideal `n` or `2n − 1`
column is a band-limited claim: it is held to the same sixteen partials and the
same one cent `docs/BENCHMARK.md` publishes — worst **0.53 cents** for the
stopped column — and the drift above that, 2.7 cents by the fifty-third
partial, is printed beside it as a measurement rather than asserted as a pass.

**Each object also states its range, and a partial inside that range which the
probe cannot produce fails the run.** The two discs are why: every zero there is
bisected on an integral rather than looked up, so they are swept to a ratio
bound — 30 for the round membrane, 200 for the clamped plate — and are complete
up to it, which is a claim the probe can make about a set rather than about a
count.

The one series it cannot check is the **marimba's**, and that is stated rather
than glossed: an arch-cut bar's tuned overtones are a maker's targets from the
percussion-acoustics literature and not the solution of a bare equation, so
there is nothing for a second implementation to solve. `src/dsp/tests.rs`
asserts the published targets themselves instead, and the two sources that
disagree about the second one are a control on the panel.

## What it caught

Four things, and all of them were real.

**A dump that was not a series.** `--dump` took the first 2,000 partials the
walk yielded, and the walk is index-major: on a membrane the whole `j` sweep
for `i = 1` runs to 565 partials before `i = 2` begins, so 2,000 rows held four
families and **nothing with `i` of 5 or more, from the seventeenth partial
upward** — both halves of degenerate pairs among them. Nothing downstream could
have repaired it, because the smallest missing ratio belonged to a family that
was not in the file at all, and a checker comparing against that table would
either report a false disagreement or agree with it for the wrong reason. Found
by the panel agent, from the tell that exactly two objects stopped at exactly
the cap. The rows are sorted by ratio before the cap now, so the cap means what
every reader already assumed it meant.

**A probe that skipped what it could not compute.** The mirror of the same
fault, on the other side of the comparison: the discs' reference series were
generated over a *box* of indices, `m < 12` and `n <= 12`, and a box misses low
partials at high angular order, since `j(30,1)` sits below `j(5,8)`. Those rows
were counted as "not in the probe's own range" and passed over, which made a
partial the probe could not produce look exactly like a partial it had checked.
Both discs are now swept to a ratio bound and are complete up to it, and a hole
inside the bound fails the run — which took the round membrane from 144
scattered modes checked to 643, and the clamped plate from 64 to 242. The worst
disagreement on the plate is at `(13, 1)`: a mode the old box could not see.

**A parity error in Miller's recurrence.** The sum rule that normalises the
downward recurrence is `J₀ + 2·Σ J_{2k} = 1`, and only every *other* value that
comes out of the recurrence belongs in that sum. Getting the parity backwards
sums the odd orders instead, which is a normalisation that silently multiplies
the answer by whatever it happens to come to. It did not move the Bessel
**zeros**, because bisection only reads signs — so the frequencies were right
and the probe's series comparison passed. What it wrecked was every
round-membrane **mode shape**, by a factor that varied with the order. The
orthonormality check in `src/dsp/tests.rs` found it: the identity
`∫₀¹ J_m(jr)²·r dr = J_{m+1}(j)²/2` came out as 39.8 instead of 0.0368.

**A contact mapping that excited nothing.** The disc's Hit and pickup controls
originally mapped the unit square into the unit disc, which put the control's
corners on the **rim** — where a clamped membrane's every mode is exactly zero.
The benchmark's series row read five hundred cents of nonsense because there was
no signal in the tail to measure. The controls are a radius and an angle now,
which also gives the second coordinate something real to do.
