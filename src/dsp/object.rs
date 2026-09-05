//! The eight objects: where their partials land, and what their mode shapes
//! are worth at a point.
//!
//! **The partial series is the whole game.** A beam sounds nothing like a
//! string because its partials are not near-integer multiples of the
//! fundamental, and no amount of filtering turns one into the other.
//! Everything else the engine does — damping, tilt, strike position — colours
//! a series these numbers have already decided.
//!
//! So every series here is solved from its own eigenvalue problem rather than
//! copied out of a book, and `tests.rs` checks each one against published
//! values it was *not* built from. The one exception is the marimba, which is
//! a maker's tuning target rather than the solution of a bare equation, and
//! it is marked as one wherever it appears.
//!
//! | object | series | the published check |
//! |---|---|---|
//! | Beam | `(β_n/β_1)²`, roots of `cos β · cosh β = 1` | Leissa, NASA SP-160 Table 4.23 |
//! | Marimba | the first two overtones are the maker's | Fletcher & Rossing; Woodhouse — they differ, and `bar_third` is that difference |
//! | String | `n·√(1 + B n²)` | Lehtonen et al., DAFx-08 eq. (2) |
//! | Membrane | `√((m/Lx)² + (n/Ly)²)` | Russell, Penn State |
//! | Membrane Round | `j_{m,n}/j_{0,1}` | Abramowitz & Stegun Table 9.5; Russell |
//! | Plate | `(m/Lx)² + (n/Ly)²` | Leissa §4.1, simply supported |
//! | Tine | `(β_n/β_1)²`, roots of `cos β · cosh β = −1` | Leissa, NASA SP-160 Table 4.39 |
//! | Plate Round | `(λ_{m,n}/λ_{0,1})²`, roots of `J_m I_{m+1} + I_m J_{m+1} = 0` | Leissa: `λ² = 10.2158` |
//! | Pipe, Tube | not here — [`crate::dsp::guide`] | |
//!
//! **The plate is the simply supported one, and that is a statement rather
//! than an oversight.** A struck plate is physically free on all four edges,
//! and the free rectangular plate has *no closed form*: Leissa's §4.3.15
//! gives Ritz-method tables and nothing else. A series that has to be
//! tabulated cannot be solved here, so this one solves the case that can be
//! and says which case it is.
//!
//! ## Mode shapes, and why they are mass-normalised
//!
//! A mode's amplitude is its shape at the strike times its shape at the
//! pickup, `a_k ∝ ψ_k(x_e)·ψ_k(x_l)`. Strike a mode on one of its nodal
//! lines and it gets nothing, which is why a string plucked at a twelfth of
//! its length has no twelfth partial. It is a derivation rather than a tone
//! control, and it is the cheapest source of convincing variation in the
//! whole design.
//!
//! Every `ψ` here is normalised so its mean square over the object is
//! **one**. That is the physical normalisation, it makes shapes comparable
//! between modes, and `tests.rs` integrates each family numerically to check
//! it rather than taking my word for it.

use std::sync::OnceLock;

/// The objects, in parameter order. 0…6 are the order Ableton's own device
/// lists them in, so an index never moves under a saved project; 7 is ours,
/// because a drum head is a disc and theirs is a rectangle.
pub const OBJECT_NAMES: [&str; 10] = [
    "Beam",
    "Marimba",
    "String",
    "Membrane",
    "Plate",
    "Pipe",
    "Tube",
    "Membrane Round",
    "Tine",
    "Plate Round",
];

/// Which of the two engines an object needs.
///
/// A solid vibrates and its motion decomposes into normal modes, so it is a
/// mode bank. An air column is only a boundary — what vibrates is the air
/// inside — so it is a waveguide, and it costs the same whatever number of
/// harmonics come out of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    Bank,
    Guide,
}

/// One of the eight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Object {
    Beam,
    Marimba,
    String,
    Membrane,
    Plate,
    Pipe,
    Tube,
    MembraneRound,
    /// A bar clamped at one end and free at the other: a tuning fork's prong,
    /// a music box's tooth, an electric piano's tine.
    Tine,
    /// A disc clamped at its rim, vibrating in flexure rather than under
    /// tension: a cymbal, a gong, a bell plate.
    PlateRound,
}

impl Object {
    /// Every object, in parameter order.
    pub const ALL: [Object; 10] = [
        Object::Beam,
        Object::Marimba,
        Object::String,
        Object::Membrane,
        Object::Plate,
        Object::Pipe,
        Object::Tube,
        Object::MembraneRound,
        Object::Tine,
        Object::PlateRound,
    ];

    /// From the `type` parameter's value, clamped.
    pub fn from_index(i: usize) -> Object {
        Object::ALL[i.min(Object::ALL.len() - 1)]
    }

    /// Which engine renders it.
    pub fn engine(self) -> Engine {
        match self {
            Object::Pipe | Object::Tube => Engine::Guide,
            _ => Engine::Bank,
        }
    }

    /// Whether the object is a surface, so the second contact coordinate
    /// means something and the partials form a two-dimensional lattice.
    pub fn is_2d(self) -> bool {
        matches!(
            self,
            Object::Membrane | Object::Plate | Object::MembraneRound | Object::PlateRound
        )
    }

    /// Whether `ratio` — the rectangle's aspect — applies.
    ///
    /// A disc has none, which is why the round membrane is not the
    /// rectangular one with a knob turned: a rectangle's lattice is
    /// `√(m² + n²)` and a disc's is a set of Bessel zeros, and those are
    /// different functions rather than a reparameterisation.
    pub fn has_aspect(self) -> bool {
        matches!(self, Object::Membrane | Object::Plate)
    }

    /// The exponent `p` in `N(f) ∝ f^p`, the count of partials below `f`.
    ///
    /// It follows from the frequency series and nothing else, and it decides
    /// how much a mode budget hurts: a bar's modal density **falls** with
    /// frequency, a plate's is constant, a membrane's **rises**. The
    /// statistical tail integrates against this to work out how much energy
    /// the bank did not cover.
    pub fn density_exponent(self) -> f32 {
        match self {
            // f ~ n², so N ~ √f.
            Object::Beam | Object::Marimba | Object::Tine => 0.5,
            // f ~ n.
            Object::String | Object::Pipe | Object::Tube => 1.0,
            // Two dimensions and f ~ (m² + n²): the lattice points under a
            // straight line, so N ~ f and the density is constant.
            Object::Plate | Object::PlateRound => 1.0,
            // Two dimensions and f ~ √(m² + n²): Weyl's law, N ~ f².
            Object::Membrane | Object::MembraneRound => 2.0,
        }
    }

    /// Relative cost of examining one candidate partial, used to keep the
    /// incremental rebuild's per-block work bounded whichever object is
    /// selected. The round membrane's shapes need a Bessel recurrence where
    /// every other object needs a sine.
    pub fn candidate_cost(self) -> usize {
        match self {
            // Both discs need a Bessel recurrence per contact where every
            // other object needs a sine; the clamped plate needs two, since
            // its shape is a difference of an ordinary and a modified one.
            Object::MembraneRound => 8,
            Object::PlateRound => 16,
            _ => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// The free–free beam
// ---------------------------------------------------------------------------

/// How many beam eigenvalues are solved.
///
/// A bar tuned to 55 Hz has 28 partials in the whole audible band and its
/// modal density *falls* with frequency, so this is far more than any tuning
/// can reach. The limit is here because `cosh β` overflows long before the
/// table would run out of usefulness.
pub const BEAM_MODES: usize = 192;

/// The eigenvalues `βL` of a free–free uniform beam: the roots of
/// `cos β · cosh β = 1`.
///
/// Solved as `cos β − sech β = 0`, which is the same equation with the
/// overflow taken out — `cosh 45` is 1.7e19, and by the fourth root the
/// product form has lost every digit it had. The roots sit just above
/// `(2n+1)π/2`, which is where Newton starts, and `sech` vanishes fast enough
/// that the high ones land on that asymptote to machine precision.
///
/// The clamped–clamped bar satisfies the same equation and so shares this
/// table; only the shapes differ.
fn beam_eigenvalues() -> &'static [f64; BEAM_MODES] {
    static T: OnceLock<[f64; BEAM_MODES]> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = [0.0f64; BEAM_MODES];
        for (i, slot) in out.iter_mut().enumerate() {
            let n = i + 1;
            let mut x = (2 * n + 1) as f64 * std::f64::consts::FRAC_PI_2;
            for _ in 0..64 {
                let sech = 1.0 / x.cosh();
                let f = x.cos() - sech;
                let df = -x.sin() + sech * x.tanh();
                if df == 0.0 {
                    break;
                }
                let step = f / df;
                x -= step;
                if step.abs() < 1e-15 * x.abs() {
                    break;
                }
            }
            *slot = x;
        }
        out
    })
}

/// The `n`-th free–free beam eigenvalue, one-based.
pub fn beam_eigenvalue(n: usize) -> f64 {
    beam_eigenvalues()[(n.max(1) - 1).min(BEAM_MODES - 1)]
}

/// The eigenvalues `βL` of a **clamped–free** bar — a cantilever — which are
/// the roots of `cos β · cosh β = −1`.
///
/// One sign away from the free–free bar's equation and a different instrument
/// entirely. The free bar's overtones sit at 2.76 and 5.40 times the
/// fundamental; a cantilever's sit at **6.27 and 17.5**, because its first
/// root is 1.875 rather than 4.730 while the rest converge on the same
/// asymptote. That gap is why a tuning fork rings almost pure and a
/// glockenspiel clangs: the cantilever's second partial is two and a half
/// octaves up and its third is more than four, so nothing is left in the
/// range where a listener hears clash.
///
/// Solved as `cos β + sech β = 0`, for the same overflow reason as the free
/// bar's. The first root is below the asymptote rather than above it, so
/// Newton starts from `(2n−1)π/2` instead.
fn tine_eigenvalues() -> &'static [f64; BEAM_MODES] {
    static T: OnceLock<[f64; BEAM_MODES]> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = [0.0f64; BEAM_MODES];
        for (i, slot) in out.iter_mut().enumerate() {
            let n = i + 1;
            let mut x = (2 * n - 1) as f64 * std::f64::consts::FRAC_PI_2;
            for _ in 0..64 {
                let sech = 1.0 / x.cosh();
                let f = x.cos() + sech;
                let df = -x.sin() - sech * x.tanh();
                if df == 0.0 {
                    break;
                }
                let step = f / df;
                x -= step;
                if step.abs() < 1e-15 * x.abs() {
                    break;
                }
            }
            *slot = x;
        }
        out
    })
}

/// The `n`-th clamped–free eigenvalue, one-based.
pub fn tine_eigenvalue(n: usize) -> f64 {
    tine_eigenvalues()[(n.max(1) - 1).min(BEAM_MODES - 1)]
}

/// The clamped–free bar's mode shape at `x ∈ [0,1]`, clamped end at zero,
/// mass-normalised.
///
/// `cosh βx − cos βx − σ(sinh βx − sin βx)` with
/// `σ = (cosh β + cos β)/(sinh β + sin β)`, rearranged the same way and for
/// the same reason as [`beam_shape`]: `σ → 1`, so `1 − σ` is a difference of
/// two numbers agreeing to `e^−β` and is worthless in double precision by the
/// fourth mode, while the `e^{βx}` it multiplies has grown enormous.
///
/// Both clamped conditions fall out of the rearrangement rather than being
/// imposed: `Y(0) = 0` and `Y'(0) = 0` exactly, which is what "clamped"
/// means and what `tests.rs` checks.
pub fn tine_shape(n: usize, x: f64) -> f64 {
    let b = tine_eigenvalue(n);
    let e = (-b).exp();
    let (sb, cb) = b.sin_cos();
    let den = 1.0 - e * e + 2.0 * sb * e;
    let p = (sb - cb - e) / den;
    let q = (1.0 + (sb + cb) * e) / den;
    let sigma = q - p * e;
    let u = b * x;
    let (su, cu) = u.sin_cos();
    p * (b * (x - 1.0)).exp() + q * (-u).exp() - cu + sigma * su
}

/// The free–free beam's mode shape at `x ∈ [0,1]`, mass-normalised.
///
/// The textbook form is `cosh βx + cos βx − σ(sinh βx + sin βx)` with
/// `σ = (cosh β − cos β)/(sinh β − sin β)`, and transcribed like that it is
/// useless above the fourth mode: `σ → 1`, so `1 − σ` is a difference of two
/// numbers agreeing to `e^−β`, and by `β ≈ 36` there is nothing left of it in
/// double precision — while the `e^{βx}` it multiplies has reached 1e15.
///
/// So `1 − σ` is formed analytically instead. With `sinh β − cosh β = −e^−β`,
/// `sinh β + cosh β = e^β`, and `D = 2(sinh β − sin β)·e^−β`:
///
/// ```text
///   p   = (cos β − sin β − e^−β)/D           so (1 − σ)/2 = p·e^−β
///   q   = (1 − (sin β + cos β)e^−β)/D        so (1 + σ)/2 = q
///   σ   = q − p·e^−β
///   Y(x) = p·e^{β(x−1)} + q·e^{−βx} + cos βx − σ·sin βx
/// ```
///
/// Every exponential there is `e` to a non-positive power, so it holds all
/// the way up the table. `∫₀¹ Y² dx = 1` falls out of that normalisation, and
/// `tests.rs` integrates it to check.
pub fn beam_shape(n: usize, x: f64) -> f64 {
    let b = beam_eigenvalue(n);
    let e = (-b).exp();
    let (sb, cb) = b.sin_cos();
    let den = 1.0 - e * e - 2.0 * sb * e;
    let p = (cb - sb - e) / den;
    let q = (1.0 - (sb + cb) * e) / den;
    let sigma = q - p * e;
    let u = b * x;
    let (su, cu) = u.sin_cos();
    p * (b * (x - 1.0)).exp() + q * (-u).exp() + cu - sigma * su
}

// ---------------------------------------------------------------------------
// Bessel functions and their zeros, for the round membrane
// ---------------------------------------------------------------------------

/// `J₀(x)`, by the standard rational and asymptotic approximations of
/// Abramowitz and Stegun §9.4. Accurate to about 1e-8, which is seven orders
/// of magnitude better than a mode gain needs and, through the zero finder,
/// puts a partial's frequency inside a ten-thousandth of a cent.
pub fn bessel_j0(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let p = 57568490574.0
            + y * (-13362590354.0
                + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * -184.9052456))));
        let q = 57568490411.0
            + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y))));
        p / q
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 0.785398164;
        let p = 1.0
            + y * (-0.1098628627e-2
                + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let q = -0.1562499995e-1
            + y * (0.1430488765e-3
                + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * -0.934935152e-7)));
        (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * p - z * xx.sin() * q)
    }
}

/// `J₁(x)`, same source and same accuracy as [`bessel_j0`].
pub fn bessel_j1(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let p = x
            * (72362614232.0
                + y * (-7895059235.0
                    + y * (242396853.1
                        + y * (-2972611.439 + y * (15704.48260 + y * -30.16036606)))));
        let q = 144725228442.0
            + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        // The odd polynomial already carries the sign of `x`.
        return p / q;
    }
    let ans = {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 2.356194491;
        let p = 1.0
            + y * (0.183105e-2
                + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * -0.240337019e-6)));
        let q = 0.04687499995
            + y * (-0.2002690873e-3
                + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        (std::f64::consts::FRAC_2_PI / ax).sqrt() * (xx.cos() * p - z * xx.sin() * q)
    };
    if x < 0.0 { -ans } else { ans }
}

/// `J_m(x)` for integer `m ≥ 0`.
///
/// Two directions, because only one of them is stable at a time. Above the
/// turning point (`x > m`) the upward recurrence
/// `J_{m+1} = (2m/x)J_m − J_{m−1}` is well conditioned and costs `m` steps.
/// Below it the upward direction amplifies whatever admixture of `Y_m`
/// rounding put into the seed, so the downward (Miller) recurrence is used
/// instead, started far enough above the wanted order that its arbitrary
/// seed is forgotten and normalised by the sum rule
/// `J₀ + 2·Σ J_{2k} = 1`.
pub fn bessel_jn(m: usize, x: f64) -> f64 {
    match m {
        0 => return bessel_j0(x),
        1 => return bessel_j1(x),
        _ => {}
    }
    let ax = x.abs();
    if ax == 0.0 {
        return 0.0;
    }
    let sign = if x < 0.0 && m % 2 == 1 { -1.0 } else { 1.0 };
    let tox = 2.0 / ax;
    if ax > m as f64 {
        let mut jm1 = bessel_j0(ax);
        let mut j = bessel_j1(ax);
        for k in 1..m {
            let next = k as f64 * tox * j - jm1;
            jm1 = j;
            j = next;
        }
        return sign * j;
    }
    // Miller: start at an even order well above the wanted one, seed it
    // arbitrarily, and recur down. The seed's error is the growing solution
    // in this direction, so it dies away rather than taking over.
    let start = 2 * ((m + (160.0 * m as f64).sqrt() as usize) / 2) + 8;
    let mut jp1 = 0.0f64;
    let mut j = 1.0f64;
    let mut want = 0.0f64;
    let mut sum = 0.0f64;
    // The sum rule is `J₀ + 2·Σ J_{2k} = 1`, so only every **other** order
    // that comes out of the recurrence belongs in it. `start` is even, so the
    // first value produced is odd and is skipped; getting this parity
    // backwards sums the odd orders instead, which is a normalisation that
    // silently multiplies the answer by whatever it happens to come to.
    let mut in_sum = false;
    for k in (1..=start).rev() {
        let jm1 = k as f64 * tox * j - jp1;
        jp1 = j;
        j = jm1;
        if j.abs() > 1e10 {
            j *= 1e-10;
            jp1 *= 1e-10;
            want *= 1e-10;
            sum *= 1e-10;
        }
        if in_sum {
            sum += j;
        }
        in_sum = !in_sum;
        if k - 1 == m {
            want = j;
        }
    }
    // `j` now holds J₀, which the loop counted twice.
    sum = 2.0 * sum - j;
    sign * want / sum
}

/// `e^{-x}·I_m(x)`, the modified Bessel function of the first kind, scaled.
///
/// Scaled because `I_m` grows like `e^x` and the plate's frequency equation
/// needs it at arguments in the tens, where the unscaled value overflows a
/// `f64` around 700 and is useless long before that. Every place it is used
/// here wants a **ratio** of two of them, and the scaling cancels.
///
/// Miller's downward recurrence again — `I_{m−1} = I_{m+1} + (2m/x)I_m` is the
/// stable direction — normalised by the sum rule `I₀ + 2·Σ I_k = e^x`, which in
/// scaled terms is simply **1**.
pub fn bessel_i_scaled(m: usize, x: f64) -> f64 {
    let ax = x.abs();
    if ax < 1e-12 {
        return if m == 0 { 1.0 } else { 0.0 };
    }
    let tox = 2.0 / ax;
    let start = 2 * ((m + (1.5 * ax + 40.0) as usize) / 2) + 8;
    let mut ip1 = 0.0f64;
    let mut i = 1.0f64;
    let mut want = 0.0f64;
    let mut sum = 0.0f64;
    for k in (1..=start).rev() {
        let im1 = ip1 + k as f64 * tox * i;
        ip1 = i;
        i = im1;
        if i.abs() > 1e100 {
            i *= 1e-100;
            ip1 *= 1e-100;
            want *= 1e-100;
            sum *= 1e-100;
        }
        // Unlike the J sum rule, every order counts here and none is doubled
        // except through the two-sided sum, so the total is I₀ + 2ΣI_k.
        sum += i;
        if k - 1 == m {
            want = i;
        }
    }
    // `i` now holds I₀; the loop added it once along with everything else.
    let total = 2.0 * sum - i;
    want / total
}

/// `I_{m+1}(x) / I_m(x)`, which is what the clamped plate's frequency
/// equation actually needs and which sits safely in `(0, 1)` for every
/// positive argument.
pub fn bessel_i_ratio(m: usize, x: f64) -> f64 {
    let a = bessel_i_scaled(m, x);
    if a.abs() < 1e-300 {
        return 0.0;
    }
    bessel_i_scaled(m + 1, x) / a
}

/// Orders kept in the round membrane's zero table.
pub const CIRCLE_ORDERS: usize = 128;
/// Zeros per order kept in the round membrane's zero table.
pub const CIRCLE_ZEROS: usize = 128;

/// `j_{m,n}` for `m < CIRCLE_ORDERS` and `1 ≤ n ≤ CIRCLE_ZEROS`.
///
/// Built once per process rather than once per instance: the zeros of a
/// Bessel function are universal constants, and a plug-in that recomputes
/// them for every voice is paying for arithmetic that cannot come out
/// differently.
///
/// Order zero starts from McMahon's expansion, where it is at its best, and
/// is refined by Newton with `J₀' = −J₁`. Every higher order is **bracketed**
/// by the interlacing property `j_{m,n} < j_{m+1,n} < j_{m,n+1}`, so the
/// previous order's table brackets the next one exactly and bisection cannot
/// wander out of it.
fn circle_zeros() -> &'static Vec<f64> {
    static T: OnceLock<Vec<f64>> = OnceLock::new();
    T.get_or_init(|| {
        let mut z = vec![0.0f64; CIRCLE_ORDERS * CIRCLE_ZEROS];
        for n in 1..=CIRCLE_ZEROS {
            let b = (n as f64 - 0.25) * std::f64::consts::PI;
            let mut x = b + 0.125 / b - 0.0807 / (b * b * b);
            for _ in 0..40 {
                let f = bessel_j0(x);
                let df = -bessel_j1(x);
                if df == 0.0 {
                    break;
                }
                let step = f / df;
                x -= step;
                if step.abs() < 1e-14 * x {
                    break;
                }
            }
            z[n - 1] = x;
        }
        for m in 1..CIRCLE_ORDERS {
            for n in 1..=CIRCLE_ZEROS {
                let lo = z[(m - 1) * CIRCLE_ZEROS + (n - 1)];
                // The previous order's next zero brackets from above. The
                // last column has nothing above it in the table, and
                // consecutive zeros approach π apart, so extrapolate.
                let hi = if n < CIRCLE_ZEROS {
                    z[(m - 1) * CIRCLE_ZEROS + n]
                } else {
                    lo + std::f64::consts::PI + 1.0
                };
                z[m * CIRCLE_ZEROS + (n - 1)] = bisect_bessel(m, lo, hi);
            }
        }
        z
    })
}

/// The single zero of `J_m` inside `(lo, hi)`, by bisection.
///
/// Bisection rather than Newton because the bracket is guaranteed by the
/// interlacing property and a bracketed method cannot leave it; sixty
/// halvings of a bracket a few units wide land far inside the accuracy of
/// the `J_m` underneath.
fn bisect_bessel(m: usize, lo: f64, hi: f64) -> f64 {
    let mut a = lo;
    let mut b = hi;
    let fa_pos = bessel_jn(m, a) > 0.0;
    for _ in 0..60 {
        let mid = 0.5 * (a + b);
        if mid <= a || mid >= b {
            break;
        }
        let fm = bessel_jn(m, mid);
        if fm == 0.0 {
            return mid;
        }
        if (fm > 0.0) == fa_pos {
            a = mid;
        } else {
            b = mid;
        }
    }
    0.5 * (a + b)
}

/// `j_{m,n}` from the table, `n` one-based; `0` outside it.
pub fn bessel_zero(m: usize, n: usize) -> f64 {
    if m >= CIRCLE_ORDERS || n == 0 || n > CIRCLE_ZEROS {
        return 0.0;
    }
    circle_zeros()[m * CIRCLE_ZEROS + (n - 1)]
}

// ---------------------------------------------------------------------------
// The clamped circular plate
// ---------------------------------------------------------------------------

/// Orders kept in the clamped disc's eigenvalue table.
pub const DISC_ORDERS: usize = 40;
/// Roots per order.
pub const DISC_ROOTS: usize = 40;

/// The frequency equation of a circular plate clamped at its rim.
///
/// Clamped means the plate cannot move **and** cannot tilt at the edge, which
/// is two conditions rather than a membrane's one, and it is why this is a
/// different object rather than a stiffer drum head. Written out, `W = 0` and
/// `dW/dr = 0` at the rim give
///
/// ```text
///   J_m(λ)·I_{m+1}(λ) + I_m(λ)·J_{m+1}(λ) = 0
/// ```
///
/// which is divided through by `I_m(λ)` here so that nothing enormous is ever
/// formed: the surviving ratio lies in `(0, 1)` and the rest is ordinary.
fn disc_equation(m: usize, lambda: f64) -> f64 {
    bessel_jn(m, lambda) * bessel_i_ratio(m, lambda) + bessel_jn(m + 1, lambda)
}

/// `λ_{m,n}` for the clamped disc, `n` one-based.
///
/// A plate is flexural, so frequency goes as `λ²` where a membrane's goes as
/// `λ`. That single square is the whole difference between a drum head and a
/// cymbal: the membrane's partials crowd together as they rise and the
/// plate's spread apart.
fn disc_roots() -> &'static Vec<f64> {
    static T: OnceLock<Vec<f64>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = vec![0.0f64; DISC_ORDERS * DISC_ROOTS];
        for m in 0..DISC_ORDERS {
            let mut found = 0usize;
            // The first root of each order sits a little above `m`; step
            // finely enough that no sign change is stepped over, and bisect
            // every one that appears.
            let step = 0.05;
            let mut x = (m as f64).max(0.5);
            let mut prev = disc_equation(m, x);
            while found < DISC_ROOTS && x < m as f64 + DISC_ROOTS as f64 * 4.0 + 40.0 {
                let next = x + step;
                let cur = disc_equation(m, next);
                if (prev > 0.0) != (cur > 0.0) {
                    let (mut a, mut b) = (x, next);
                    let fa_pos = prev > 0.0;
                    for _ in 0..80 {
                        let mid = 0.5 * (a + b);
                        if mid <= a || mid >= b {
                            break;
                        }
                        if (disc_equation(m, mid) > 0.0) == fa_pos {
                            a = mid;
                        } else {
                            b = mid;
                        }
                    }
                    out[m * DISC_ROOTS + found] = 0.5 * (a + b);
                    found += 1;
                }
                x = next;
                prev = cur;
            }
        }
        out
    })
}

/// `λ_{m,n}` from the table, `n` one-based; `0` outside it.
pub fn disc_root(m: usize, n: usize) -> f64 {
    if m >= DISC_ORDERS || n == 0 || n > DISC_ROOTS {
        return 0.0;
    }
    disc_roots()[m * DISC_ROOTS + (n - 1)]
}

/// The clamped disc's radial shape at `r ∈ [0,1]`, before normalisation.
///
/// ```text
///   W(r) = J_m(λr) − [J_m(λ)/I_m(λ)]·I_m(λr)
/// ```
///
/// The bracket is a ratio of two modified Bessel functions, which at these
/// arguments are enormous individually and perfectly ordinary as a quotient.
/// Written with the scaled `e^{-x}I_m`, the whole term carries a factor
/// `e^{λ(r−1)}` that is at most one, so nothing large is ever formed.
///
/// `W(1) = 0` exactly, by construction rather than by arithmetic — which is
/// half of what "clamped" means, and the half a membrane also has. The other
/// half, `W'(1) = 0`, is what the eigenvalue was solved for.
fn disc_shape_raw(m: usize, lambda: f64, r: f64) -> f64 {
    if lambda <= 0.0 {
        return 0.0;
    }
    let denom = bessel_i_scaled(m, lambda);
    if denom.abs() < 1e-300 {
        return 0.0;
    }
    let scale = (lambda * (r - 1.0)).exp() * bessel_i_scaled(m, lambda * r) / denom;
    bessel_jn(m, lambda * r) - bessel_jn(m, lambda) * scale
}

/// The normalisation that makes each clamped-disc mode's mean square one over
/// the disc, computed once with the roots.
///
/// Every other family here has a closed form for this; a clamped plate's
/// shape is a difference of two Bessel functions and does not, so it is
/// integrated. That is a one-time cost in a static table rather than
/// per-block work, and `tests.rs` checks the result the same way it checks
/// the families that do have one.
fn disc_norms() -> &'static Vec<f64> {
    static T: OnceLock<Vec<f64>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = vec![0.0f64; DISC_ORDERS * DISC_ROOTS];
        const STEPS: usize = 512;
        for m in 0..DISC_ORDERS {
            for n in 1..=DISC_ROOTS {
                let lambda = disc_root(m, n);
                if lambda <= 0.0 {
                    continue;
                }
                // (1/area) ∫ W² cos²(mθ) dA over the unit disc, which is
                // 2∫₀¹ W² r dr for m = 0 and half that for m ≥ 1.
                let mut acc = 0.0f64;
                for i in 0..=STEPS {
                    let r = i as f64 / STEPS as f64;
                    let w = if i == 0 || i == STEPS { 0.5 } else { 1.0 };
                    let v = disc_shape_raw(m, lambda, r);
                    acc += w * v * v * r;
                }
                let mean = acc / STEPS as f64 * if m == 0 { 2.0 } else { 1.0 };
                out[m * DISC_ROOTS + (n - 1)] = if mean > 1e-30 { 1.0 / mean.sqrt() } else { 0.0 };
            }
        }
        out
    })
}

/// The clamped disc's radial shape, mass-normalised.
pub fn disc_shape(m: usize, n: usize, r: f64) -> f64 {
    if m >= DISC_ORDERS || n == 0 || n > DISC_ROOTS {
        return 0.0;
    }
    disc_shape_raw(m, disc_root(m, n), r) * disc_norms()[m * DISC_ROOTS + (n - 1)]
}

// ---------------------------------------------------------------------------
// The marimba's tuning targets
// ---------------------------------------------------------------------------

/// What the maker tunes the first overtone to. A marimba bar is arch-cut
/// until its first overtone lands two octaves above the fundamental; a
/// xylophone bar is cut to a twelfth.
pub const BAR_TUNING_NAMES: [&str; 2] = ["Marimba 4:1", "Xylophone 3:1"];

/// Where the second tuned overtone lands.
///
/// **The sources disagree, and this control is the disagreement.**
/// Woodhouse's *Euphonics* §3.3 gives about 9.2×; Fletcher and Rossing quote
/// 10×. That is close to a whole tone at that partial — a real choice a
/// builder makes, not a discrepancy to be averaged away — and no panel that
/// generates its modes from nine global knobs can offer it.
pub const BAR_THIRD_NAMES: [&str; 2] = ["9.2x", "10x"];

/// The two tuned overtone ratios for a `(bar_tuning, bar_third)` pair.
///
/// The 4:1 and 3:1 first overtones and the 9.2× and 10× second overtones are
/// the literature's, for marimba bars. **The xylophone's second tuned
/// overtone is modelling and not a source**: a bar cut to a twelfth is a
/// shallower arch than one cut to two octaves and cannot put its third
/// partial where the marimba's sits, so it is scaled by the same factor the
/// first overtone moved. I could not reach a published figure for it.
pub fn bar_targets(tuning: usize, third: usize) -> (f64, f64) {
    let published_third = if third == 0 { 9.2 } else { 10.0 };
    if tuning == 0 {
        (4.0, published_third)
    } else {
        (3.0, published_third * 3.0 / 4.0)
    }
}

// ---------------------------------------------------------------------------
// Enumerating an object's partials
// ---------------------------------------------------------------------------

/// One candidate partial: where it sits, and which mode it is.
///
/// `i` and `j` are the mode's own indices — `n` for a one-dimensional object,
/// `(m, n)` for a surface — and they are what a per-mode edit addresses, so
/// they keep their meaning when the selection changes underneath.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Partial {
    /// `f_k / f_1`.
    pub ratio: f32,
    pub i: u16,
    pub j: u16,
}

/// Everything an enumeration needs beyond the object itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub object: Object,
    /// `Lx / Ly` for the rectangular surfaces; ignored otherwise.
    pub aspect: f32,
    /// Stiff-string inharmonicity `B`, signed: positive stretches the
    /// partials the way a real string's stiffness does, negative compresses
    /// them, which no string does and which the docs call synthetic.
    pub inharm_b: f32,
    pub bar_tuning: usize,
    pub bar_third: usize,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            object: Object::String,
            aspect: 1.0,
            inharm_b: 0.0,
            bar_tuning: 0,
            bar_third: 0,
        }
    }
}

impl Shape {
    /// The ratio of partial `(i, j)` to the fundamental, before
    /// inharmonicity.
    pub fn base_ratio(&self, i: u16, j: u16) -> f64 {
        let a = self.aspect.clamp(0.05, 20.0) as f64;
        match self.object {
            Object::Beam => {
                let b = beam_eigenvalue(i as usize) / beam_eigenvalue(1);
                b * b
            }
            Object::Marimba => {
                let (t2, t3) = bar_targets(self.bar_tuning, self.bar_third);
                match i {
                    0 => 0.0,
                    1 => 1.0,
                    2 => t2,
                    3 => t3,
                    _ => {
                        // Above the second tuned overtone the arch profile no
                        // longer controls the ratio and I have no source for
                        // what it becomes. Continuing on the free bar's series
                        // scaled by the factor the third partial needed keeps
                        // the sequence monotonic and the interval structure
                        // intact; it is modelling, and the module docs say so.
                        let free3 = {
                            let b = beam_eigenvalue(3) / beam_eigenvalue(1);
                            b * b
                        };
                        let b = beam_eigenvalue(i as usize) / beam_eigenvalue(1);
                        b * b * (t3 / free3)
                    }
                }
            }
            Object::Tine => {
                let b = tine_eigenvalue(i as usize) / tine_eigenvalue(1);
                b * b
            }
            Object::String | Object::Pipe | Object::Tube => i as f64,
            Object::Membrane => {
                // Unit area, so the aspect changes the shape and not the
                // size: Lx = √a and Ly = 1/√a.
                let (m, n) = (i as f64, j as f64);
                ((m * m / a + n * n * a) / (1.0 / a + a)).sqrt()
            }
            Object::Plate => {
                let (m, n) = (i as f64, j as f64);
                (m * m / a + n * n * a) / (1.0 / a + a)
            }
            Object::MembraneRound => {
                let z = bessel_zero(i as usize, j as usize);
                if z == 0.0 { 0.0 } else { z / bessel_zero(0, 1) }
            }
            Object::PlateRound => {
                // Flexural, so frequency goes as the **square** of the
                // eigenvalue where a membrane's goes as the eigenvalue
                // itself. That square is the whole difference between a drum
                // head and a cymbal.
                let l = disc_root(i as usize, j as usize);
                if l == 0.0 {
                    0.0
                } else {
                    let base = disc_root(0, 1);
                    (l * l) / (base * base)
                }
            }
        }
    }

    /// The ratio with inharmonicity applied.
    ///
    /// The physical half is Fletcher's stiff string exactly as Lehtonen and
    /// colleagues write it, `f_k = k·f₀·√(1 + B k²)`, one-signed because a
    /// stiff string's partials are stretched and never compressed. The
    /// negative half is its reciprocal, which compresses them; that is how a
    /// string model is made to sound like a gong, it is a legitimate
    /// synthetic extension, and it is not a stiff string. The panel prints
    /// `B` on both halves so nobody has to guess which one they are on.
    ///
    /// It is applied to the partial's own **ratio** rather than to an integer
    /// index, so it means something for the objects whose series is not `n` —
    /// where it is a stretch of a series rather than a derivation from
    /// stiffness, which is also stated.
    pub fn ratio(&self, i: u16, j: u16) -> f64 {
        let r = self.base_ratio(i, j);
        let b = self.inharm_b as f64;
        if b == 0.0 || r == 0.0 {
            return r;
        }
        let s = (1.0 + b.abs() * r * r).sqrt();
        if b > 0.0 { r * s } else { r / s }
    }

    /// How many partials the object has at or below `max_ratio`, counted
    /// without enumerating them one at a time.
    ///
    /// A rectangular membrane tuned to 55 Hz has 219,541 partials below
    /// 20 kHz, so the readout that says how many exist cannot be a loop over
    /// all of them. Each family's count collapses to a walk over the first
    /// index with the second solved in closed form, which is `O(√N)`.
    ///
    /// Inharmonicity is monotonic in the ratio, so the ceiling can simply be
    /// pulled back through it rather than applied per partial.
    pub fn available(&self, max_ratio: f64) -> usize {
        if max_ratio < 1.0 {
            return 0;
        }
        let cap = self.uninharm(max_ratio);
        let a = self.aspect.clamp(0.05, 20.0) as f64;
        match self.object {
            Object::Beam | Object::Marimba | Object::Tine => {
                let mut n = 0usize;
                while n < BEAM_MODES && self.base_ratio(n as u16 + 1, 0) <= cap {
                    n += 1;
                }
                n
            }
            Object::String | Object::Pipe | Object::Tube => cap.floor().max(0.0) as usize,
            Object::Membrane | Object::Plate => {
                let s = 1.0 / a + a;
                // Membrane: (m²/a + n²a)/s ≤ cap². Plate: the same without
                // the square, because a plate is flexural and a membrane is
                // not.
                let rhs = if self.object == Object::Membrane {
                    cap * cap * s
                } else {
                    cap * s
                };
                let mut total = 0usize;
                let mut m = 1u32;
                loop {
                    let rem = rhs - (m * m) as f64 / a;
                    if rem < a {
                        break;
                    }
                    total += (rem / a).sqrt().floor() as usize;
                    m += 1;
                    if m > 1 << 20 {
                        break;
                    }
                }
                total
            }
            Object::MembraneRound => {
                let z_cap = cap * bessel_zero(0, 1);
                let mut total = 0usize;
                for m in 0..CIRCLE_ORDERS {
                    if bessel_zero(m, 1) > z_cap {
                        break;
                    }
                    let mut n = CIRCLE_ZEROS;
                    while n > 0 && bessel_zero(m, n) > z_cap {
                        n -= 1;
                    }
                    total += n;
                }
                total
            }
            Object::PlateRound => {
                let l_cap = cap.max(0.0).sqrt() * disc_root(0, 1);
                let mut total = 0usize;
                for m in 0..DISC_ORDERS {
                    if disc_root(m, 1) > l_cap {
                        break;
                    }
                    let mut n = DISC_ROOTS;
                    while n > 0 && disc_root(m, n) > l_cap {
                        n -= 1;
                    }
                    total += n;
                }
                total
            }
        }
    }

    /// The base ratio whose inharmonic image is `r`; the inverse of the
    /// stretch, used to pull a frequency ceiling back through it.
    fn uninharm(&self, r: f64) -> f64 {
        let b = self.inharm_b as f64;
        if b == 0.0 {
            return r;
        }
        // r = x·√(1 + |b|x²) for a stretch, r = x/√(1 + |b|x²) for a
        // compression; both are quadratics in x².
        let ab = b.abs();
        if b > 0.0 {
            // ab·x⁴ + x² − r² = 0.
            let d = (1.0 + 4.0 * ab * r * r).sqrt();
            (((d - 1.0) / (2.0 * ab)).max(0.0)).sqrt()
        } else {
            // x² = r²/(1 − ab·r²); above the pole every partial fits.
            let den = 1.0 - ab * r * r;
            if den <= 0.0 {
                f64::INFINITY
            } else {
                (r * r / den).sqrt()
            }
        }
    }
}

/// A lazy walk over every partial of an object at or below a ratio ceiling.
///
/// Lazy because a membrane has hundreds of thousands of them and the
/// selection only ever keeps a few thousand. Materialising the lattice in
/// order to sort it would cost megabytes and a millisecond; walking it into a
/// bounded heap costs neither.
pub struct Walk {
    shape: Shape,
    max_ratio: f64,
    i: u16,
    j: u16,
    done: bool,
}

impl Walk {
    pub fn new(shape: Shape, max_ratio: f64) -> Walk {
        // **Both discs number their angular index from zero**, because the
        // axisymmetric modes — no nodal diameter at all — are real modes and
        // are the ones a strike at the centre excites. Starting at one drops
        // that whole family silently: the object still rings, with its
        // fundamental missing.
        let polar = matches!(shape.object, Object::MembraneRound | Object::PlateRound);
        Walk {
            shape,
            max_ratio,
            i: if polar { 0 } else { 1 },
            j: if shape.object.is_2d() { 1 } else { 0 },
            done: max_ratio < 1.0,
        }
    }
}

impl Iterator for Walk {
    type Item = Partial;

    fn next(&mut self) -> Option<Partial> {
        if self.done {
            return None;
        }
        let object = self.shape.object;
        if !object.is_2d() {
            let i = self.i;
            let limit = match object {
                Object::Beam | Object::Marimba | Object::Tine => BEAM_MODES as u16,
                _ => u16::MAX - 1,
            };
            if i > limit {
                self.done = true;
                return None;
            }
            let r = self.shape.ratio(i, 0);
            if r > self.max_ratio {
                // Every one-dimensional series here increases with `i`, so
                // the first partial past the ceiling ends the walk.
                self.done = true;
                return None;
            }
            self.i += 1;
            return Some(Partial {
                ratio: r as f32,
                i,
                j: 0,
            });
        }

        let round = object == Object::MembraneRound;
        let disc = object == Object::PlateRound;
        let i_limit = if round {
            CIRCLE_ORDERS as u16
        } else if disc {
            DISC_ORDERS as u16
        } else {
            u16::MAX - 1
        };
        let j_limit = if round {
            CIRCLE_ZEROS as u16
        } else if disc {
            DISC_ROOTS as u16
        } else {
            u16::MAX - 1
        };
        loop {
            if self.i >= i_limit {
                self.done = true;
                return None;
            }
            let (i, j) = (self.i, self.j);
            let r = if j > j_limit {
                f64::INFINITY
            } else {
                self.shape.ratio(i, j)
            };
            if r > self.max_ratio || r == 0.0 {
                // Past the ceiling up this column, so start the next one. A
                // column that is already past it at `j = 1` ends the walk,
                // because every series here rises with `i` as well.
                let empty = j <= 1;
                self.i += 1;
                self.j = 1;
                if empty {
                    self.done = true;
                    return None;
                }
                continue;
            }
            self.j += 1;
            return Some(Partial {
                ratio: r as f32,
                i,
                j,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Mode shapes at a point
// ---------------------------------------------------------------------------

/// A contact point on the object, in the panel's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }

    /// The contact point on a disc — a membrane or a plate: **X is the
    /// distance from the centre and Y is the angle round it.**
    ///
    /// Not a square mapped into a circle, which is the obvious thing and is
    /// wrong twice. It wastes the corners, and worse, it clamps them to the
    /// rim — where a clamped membrane's every mode is exactly zero, so a
    /// strike in the corner of the control excites nothing at all. The first
    /// version of this did that, and the benchmark's series row read five
    /// hundred cents of nonsense because there was no signal to measure.
    ///
    /// Radius and angle also use both coordinates for something, which the
    /// square mapping does not: the angle is what decides which orientation
    /// of a degenerate pair the strike lines up with, and it is audible.
    fn polar(self) -> (f64, f64) {
        (
            (self.x as f64).clamp(0.0, 1.0),
            std::f64::consts::TAU * self.y as f64,
        )
    }
}

/// The mode shapes of one object evaluated at the strike and at both pickups,
/// mass-normalised.
///
/// The three are computed together because they are always wanted together,
/// and because the round membrane's angular part is measured **from the
/// strike**. A point strike excites only the orientation of a degenerate pair
/// that lines up with it, so working in that frame halves the mode count
/// without approximating anything.
pub struct Contacts {
    shape: Shape,
    exc: Point,
    left: Point,
    right: Point,
    exc_polar: (f64, f64),
    left_polar: (f64, f64),
    right_polar: (f64, f64),
}

impl Contacts {
    pub fn new(shape: Shape, exc: Point, left: Point, right: Point) -> Contacts {
        Contacts {
            shape,
            exc,
            left,
            right,
            exc_polar: exc.polar(),
            left_polar: left.polar(),
            right_polar: right.polar(),
        }
    }

    /// `(ψ(strike), ψ(left), ψ(right))` for partial `(i, j)`.
    pub fn psi(&self, i: u16, j: u16) -> (f32, f32, f32) {
        match self.shape.object {
            Object::Beam | Object::Marimba => {
                let n = i as usize;
                (
                    beam_shape(n, self.exc.x as f64) as f32,
                    beam_shape(n, self.left.x as f64) as f32,
                    beam_shape(n, self.right.x as f64) as f32,
                )
            }
            Object::Tine => {
                let n = i as usize;
                (
                    tine_shape(n, self.exc.x as f64) as f32,
                    tine_shape(n, self.left.x as f64) as f32,
                    tine_shape(n, self.right.x as f64) as f32,
                )
            }
            Object::String | Object::Pipe | Object::Tube => {
                // √2·sin(nπx) has mean square 1 over the length.
                let k = i as f64 * std::f64::consts::PI;
                let r2 = std::f64::consts::SQRT_2;
                (
                    (r2 * (k * self.exc.x as f64).sin()) as f32,
                    (r2 * (k * self.left.x as f64).sin()) as f32,
                    (r2 * (k * self.right.x as f64).sin()) as f32,
                )
            }
            Object::Membrane | Object::Plate => {
                let (m, n) = (i as f64, j as f64);
                let pi = std::f64::consts::PI;
                let f = |p: Point| 2.0 * (m * pi * p.x as f64).sin() * (n * pi * p.y as f64).sin();
                (
                    f(self.exc) as f32,
                    f(self.left) as f32,
                    f(self.right) as f32,
                )
            }
            Object::PlateRound => {
                let m = i as usize;
                if disc_root(m, j as usize) <= 0.0 {
                    return (0.0, 0.0, 0.0);
                }
                let th0 = self.exc_polar.1;
                let at = |p: (f64, f64), angular: bool| {
                    let radial = disc_shape(m, j as usize, p.0);
                    if m == 0 || !angular {
                        radial
                    } else {
                        radial * (m as f64 * (p.1 - th0)).cos()
                    }
                };
                (
                    at(self.exc_polar, false) as f32,
                    at(self.left_polar, true) as f32,
                    at(self.right_polar, true) as f32,
                )
            }
            Object::MembraneRound => {
                let m = i as usize;
                let z = bessel_zero(m, j as usize);
                if z == 0.0 {
                    return (0.0, 0.0, 0.0);
                }
                // Mass normalisation over the unit disc, from
                // ∫₀¹ J_m(jr)²·r dr = J_{m+1}(j)²/2.
                let jm1 = bessel_jn(m + 1, z).abs();
                if jm1 < 1e-12 {
                    return (0.0, 0.0, 0.0);
                }
                let norm = if m == 0 {
                    1.0 / jm1
                } else {
                    std::f64::consts::SQRT_2 / jm1
                };
                let th0 = self.exc_polar.1;
                let at = |p: (f64, f64), angular: bool| {
                    let radial = norm * bessel_jn(m, z * p.0);
                    if m == 0 || !angular {
                        radial
                    } else {
                        radial * (m as f64 * (p.1 - th0)).cos()
                    }
                };
                (
                    at(self.exc_polar, false) as f32,
                    at(self.left_polar, true) as f32,
                    at(self.right_polar, true) as f32,
                )
            }
        }
    }
}
