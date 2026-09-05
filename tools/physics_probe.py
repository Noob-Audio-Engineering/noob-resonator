"""Out-of-tree probe: the partial series of every object, from the published
formulae, touching none of the plug-in's code.

Nothing here imports, links or reads the Rust. Bessel functions come from the
integral representation, beam eigenvalues from bisection on the frequency
equation, and every result is checked against a value somebody else published
before it is used to check anything of ours.

    python p1_physics.py                 # the tables, and the published checks
    python p1_physics.py --compare FILE  # ... and diff against a dump from the engine

The dump format is what `benchmark --dump series` writes: lines of
`object,i,j,ratio`.
"""

import math
import sys

# ---------------------------------------------------------------------------
# Bessel functions from the integral representation, Abramowitz & Stegun 9.1.21
#   J_m(x) = (1/pi) * INT_0^pi cos(m*tau - x*sin tau) d tau
# Integrated by the trapezoid rule, which is spectrally accurate for a smooth
# periodic integrand; the panel count scales with x so the oscillation is
# always resolved.
# ---------------------------------------------------------------------------


def bessel_j(m, x):
    if x == 0.0:
        return 1.0 if m == 0 else 0.0
    n = max(256, int(40 + 12 * abs(x)))
    h = math.pi / n
    s = 0.5 * (math.cos(0.0) + math.cos(m * math.pi))
    for k in range(1, n):
        tau = k * h
        s += math.cos(m * tau - x * math.sin(tau))
    return s * h / math.pi


def bessel_i(m, x):
    """I_m(x) from its own integral representation, A&S 9.6.18:
        I_m(x) = (1/pi) INT_0^pi e^{x cos t} cos(m t) dt
    A different algorithm from the engine's scaled downward recurrence, which
    is the point. Only used at small arguments, where e^x is ordinary."""
    n = max(256, int(60 + 20 * abs(x)))
    h = math.pi / n
    s = 0.5 * (math.exp(x) + math.exp(-x) * math.cos(m * math.pi))
    for k in range(1, n):
        t = k * h
        s += math.exp(x * math.cos(t)) * math.cos(m * t)
    return s * h / math.pi


def disc_equation(m, lam):
    """A circular plate clamped at its rim:
        J_m(l) I_{m+1}(l) + I_m(l) J_{m+1}(l) = 0
    Formed here with both functions in full rather than as a ratio, which is
    only possible because the probe stays at small arguments."""
    return bessel_j(m, lam) * bessel_i(m + 1, lam) + bessel_i(m, lam) * bessel_j(m + 1, lam)


def disc_roots(m, count):
    out = []
    x = max(0.5, float(m))
    step = 0.05
    prev = disc_equation(m, x)
    while len(out) < count and x < m + 4 * count + 40:
        x2 = x + step
        cur = disc_equation(m, x2)
        if (prev > 0.0) != (cur > 0.0):
            a, b, fa = x, x2, prev
            for _ in range(80):
                mid = 0.5 * (a + b)
                fm = disc_equation(m, mid)
                if (fm > 0.0) == (fa > 0.0):
                    a, fa = mid, fm
                else:
                    b = mid
            out.append(0.5 * (a + b))
        x, prev = x2, cur
    return out


def plate_round(count):
    zs = []
    for m in range(0, 8):
        for n, l in enumerate(disc_roots(m, 8), start=1):
            zs.append((l * l, m, n))
    zs.sort()
    base = zs[0][0]
    return [(v / base, m, n) for v, m, n in zs[:count]]


def bessel_zeros(m, count):
    """The first `count` positive zeros of J_m, by scanning for sign changes
    and bisecting. Slow and obviously correct, which is what a probe wants."""
    out = []
    # The first zero of J_m is above m; zeros are spaced about pi apart.
    x = max(1e-6, m * 1.0)
    step = 0.05
    prev = bessel_j(m, x)
    while len(out) < count:
        x2 = x + step
        cur = bessel_j(m, x2)
        if prev == 0.0:
            out.append(x)
        elif (prev > 0.0) != (cur > 0.0):
            a, b = x, x2
            fa = prev
            for _ in range(80):
                mid = 0.5 * (a + b)
                fm = bessel_j(m, mid)
                if (fm > 0.0) == (fa > 0.0):
                    a, fa = mid, fm
                else:
                    b = mid
            out.append(0.5 * (a + b))
        x, prev = x2, cur
        if x > 20 + m * 1.2 + count * 4:
            break
    return out


# ---------------------------------------------------------------------------
# Free-free beam: roots of cos(b) cosh(b) = 1, written as cos b = sech b so
# that cosh does not overflow.
# ---------------------------------------------------------------------------


def beam_eigenvalues(count):
    out = []
    for n in range(1, count + 1):
        lo = (2 * n + 1) * math.pi / 2 - 0.5
        hi = (2 * n + 1) * math.pi / 2 + 0.5
        f = lambda b: math.cos(b) - 1.0 / math.cosh(b)
        a, b = lo, hi
        fa = f(a)
        if (fa > 0) == (f(b) > 0):
            # Widen once; the roots are always within half a unit of the
            # asymptote after the first.
            a, b = lo - 1.0, hi + 1.0
            fa = f(a)
        for _ in range(200):
            mid = 0.5 * (a + b)
            fm = f(mid)
            if (fm > 0) == (fa > 0):
                a, fa = mid, fm
            else:
                b = mid
        out.append(0.5 * (a + b))
    return out


def tine_eigenvalues(count):
    """Roots of cos(b) cosh(b) = -1, the clamped-free bar. Written as
    cos b + sech b = 0 for the same overflow reason as the free-free case;
    the first root is below the asymptote rather than above it."""
    out = []
    f = lambda b: math.cos(b) + 1.0 / math.cosh(b)
    for n in range(1, count + 1):
        centre = (2 * n - 1) * math.pi / 2
        a, b = centre - 1.0, centre + 1.0
        fa = f(a)
        for _ in range(200):
            mid = 0.5 * (a + b)
            fm = f(mid)
            if (fm > 0) == (fa > 0):
                a, fa = mid, fm
            else:
                b = mid
        out.append(0.5 * (a + b))
    return out


def tine_ratios(count):
    e = tine_eigenvalues(count)
    return [(b / e[0]) ** 2 for b in e]


def beam_shape(beta, x):
    """cosh(bx)+cos(bx) - sigma (sinh(bx)+sin(bx)), evaluated the slow, direct
    way in Python's floats. Only used at low mode numbers, where the direct
    form is still exact -- which is the point of checking the engine's
    rearranged version against it."""
    sigma = (math.cosh(beta) - math.cos(beta)) / (math.sinh(beta) - math.sin(beta))
    u = beta * x
    return math.cosh(u) + math.cos(u) - sigma * (math.sinh(u) + math.sin(u))


def integrate(f, a, b, n=20001):
    h = (b - a) / (n - 1)
    s = 0.0
    for k in range(n):
        w = 1.0 if 0 < k < n - 1 else 0.5
        s += w * f(a + k * h)
    return s * h


# ---------------------------------------------------------------------------
# The series
# ---------------------------------------------------------------------------


def beam_ratios(count):
    e = beam_eigenvalues(count)
    return [(b / e[0]) ** 2 for b in e]


def string_ratios(count, B=0.0):
    return [k * math.sqrt(1.0 + B * k * k) for k in range(1, count + 1)]


def membrane_rect(aspect, count):
    """f ~ sqrt((m/Lx)^2 + (n/Ly)^2) with Lx*Ly = 1."""
    a = aspect
    f1 = math.sqrt(1.0 / a + a)
    out = []
    for m in range(1, 200):
        for n in range(1, 200):
            out.append((math.sqrt(m * m / a + n * n * a) / f1, m, n))
    out.sort()
    return out[:count]


def plate_rect(aspect, count):
    a = aspect
    f1 = 1.0 / a + a
    out = []
    for m in range(1, 200):
        for n in range(1, 200):
            out.append(((m * m / a + n * n * a) / f1, m, n))
    out.sort()
    return out[:count]


def membrane_round(count):
    zs = []
    for m in range(0, 12):
        for n, z in enumerate(bessel_zeros(m, 12), start=1):
            zs.append((z, m, n))
    zs.sort()
    j01 = zs[0][0]
    return [(z / j01, m, n) for z, m, n in zs[:count]]


def cents(a, b):
    return 1200.0 * math.log2(a / b)


# ---------------------------------------------------------------------------
# The published checks, run before anything here is used on anything of ours
# ---------------------------------------------------------------------------


def published_checks():
    ok = True

    def check(name, got, want, tol, unit=""):
        nonlocal ok
        d = abs(got - want)
        good = d <= tol
        ok = ok and good
        print(
            f"  {'ok ' if good else 'FAIL'} {name:<44} {got:>14.9f}  vs {want:<14.9f}"
            f"  d={d:.3e}{unit}"
        )

    print("Published checks")
    # Leissa, NASA SP-160 Table 4.23 (free-free / clamped-clamped beam).
    leissa = [4.730041, 7.853205, 10.995608, 14.137165, 17.278760, 20.420352]
    e = beam_eigenvalues(6)
    for k, (g, w) in enumerate(zip(e, leissa), start=1):
        check(f"beam eigenvalue beta_{k} (Leissa T4.23)", g, w, 5e-7)

    # Abramowitz & Stegun Table 9.5.
    j01 = bessel_zeros(0, 1)[0]
    check("j_0,1 (A&S 9.5)", j01, 2.404825558, 1e-7)
    check("j_1,1 (A&S 9.5)", bessel_zeros(1, 1)[0], 3.831705970, 1e-7)

    # Russell, Penn State: circular membrane ratios.
    russell = [1.000, 1.593, 2.135, 2.295, 2.917, 3.598]
    got = [r for r, _, _ in membrane_round(40)]
    # Russell lists the (0,1) (1,1) (2,1) (0,2) (1,2) (0,3) modes; pick them
    # out by index rather than by position in the sorted list.
    want_modes = [(0, 1), (1, 1), (2, 1), (0, 2), (1, 2), (0, 3)]
    table = {(m, n): r for r, m, n in membrane_round(60)}
    for (m, n), w in zip(want_modes, russell):
        check(f"circular membrane ({m},{n}) (Russell)", table[(m, n)], w, 6e-4)

    # The free-free beam's own ratios, as MODAL.md prints them.
    modal = [1.0000, 2.7565, 5.4039, 8.9330, 13.3443]
    for k, (g, w) in enumerate(zip(beam_ratios(5), modal), start=1):
        check(f"free-free beam ratio {k} (MODAL 2.3)", g, w, 6e-5)

    # Leissa, NASA SP-160 Table 4.39 (clamped-free beam), which MODAL.md 2.3
    # also prints. One sign away from the free-free equation.
    leissa_cf = [1.875104, 4.694091, 7.854757, 10.995541, 14.137168]
    for k, (g, w) in enumerate(zip(tine_eigenvalues(5), leissa_cf), start=1):
        check(f"cantilever eigenvalue beta_{k} (Leissa T4.39)", g, w, 5e-7)
    for k, w in enumerate([1.0, 6.267, 17.55, 34.39, 56.84]):
        check(f"cantilever ratio {k + 1} (CORPUS 4.2)", tine_ratios(5)[k], w, 0.01)

    # A&S Table 9.8, through the integral representation.
    for x, w in [(1.0, 0.4657596), (2.0, 0.3085083), (5.0, 0.1835408)]:
        check(f"e^-x I0({x}) (A&S 9.8)", bessel_i(0, x) * math.exp(-x), w, 1e-6)

    # The clamped circular plate, Leissa: lambda^2 = 10.2158 and 21.26.
    l01 = disc_roots(0, 1)[0]
    check("clamped disc lambda^2 (0,1) (Leissa)", l01 * l01, 10.2158, 0.01)
    l11 = disc_roots(1, 1)[0]
    check("clamped disc lambda^2 (1,1) (Leissa)", l11 * l11, 21.26, 0.01)

    # Lehtonen et al. DAFx-08: a piano C4 with B = 3.0e-4.
    for n, w in [(8, 16.5), (16, 64.1), (32, 231.9)]:
        r = string_ratios(n, 3.0e-4)[n - 1]
        check(f"stiff string partial {n} sharp by (DAFx-08)", cents(r, n), w, 0.1, " ct")

    # Square membrane ratios, Russell / elementary.
    sq = [r for r, _, _ in membrane_rect(1.0, 6)]
    for k, w in enumerate([1.0, 1.5811388, 1.5811388, 2.0, 2.2360680, 2.2360680]):
        check(f"square membrane ratio {k + 1}", sq[k], w, 1e-6)

    # Square plate ratios: f ~ (m^2 + n^2), so 2,5,5,8,10,10 over 2.
    pl = [r for r, _, _ in plate_rect(1.0, 6)]
    for k, w in enumerate([1.0, 2.5, 2.5, 4.0, 5.0, 5.0]):
        check(f"square plate ratio {k + 1}", pl[k], w, 1e-9)

    print("  ->", "all published checks pass" if ok else "SOMETHING FAILED")
    return ok


def orthonormality():
    """Mean square of each mode shape over the object, which should be 1."""
    print("\nMode-shape mass normalisation (mean square over the object, want 1)")
    e = beam_eigenvalues(6)
    for n, b in enumerate(e, start=1):
        v = integrate(lambda x: beam_shape(b, x) ** 2, 0.0, 1.0, 40001)
        print(f"  beam mode {n}: {v:.9f}")
    for n in (1, 2, 5):
        v = integrate(lambda x: (math.sqrt(2) * math.sin(n * math.pi * x)) ** 2, 0.0, 1.0)
        print(f"  string mode {n}: {v:.9f}")
    for m, n in ((1, 1), (2, 3)):
        f = lambda x, y: (2 * math.sin(m * math.pi * x) * math.sin(n * math.pi * y)) ** 2
        v = integrate(lambda x: integrate(lambda y: f(x, y), 0.0, 1.0, 601), 0.0, 1.0, 601)
        print(f"  membrane mode ({m},{n}): {v:.9f}")
    for m, n in ((0, 1), (1, 1), (2, 2)):
        z = bessel_zeros(m, n)[n - 1]
        jm1 = bessel_j(m + 1, z)
        norm = (1.0 / abs(jm1)) if m == 0 else (math.sqrt(2) / abs(jm1))
        # (1/area) INT J_m(zr)^2 cos^2(m th) r dr dth over the unit disc.
        radial = integrate(lambda r: (norm * bessel_j(m, z * r)) ** 2 * r, 0.0, 1.0, 4001)
        ang = math.pi if m > 0 else 2 * math.pi
        print(f"  round membrane mode ({m},{n}): {radial * ang / math.pi:.9f}")


def tables():
    print("\nFree-free beam, first 8 ratios")
    print("  " + "  ".join(f"{r:.4f}" for r in beam_ratios(8)))
    print("\nMarimba targets (Fletcher & Rossing / Woodhouse), and the free bar")
    print(f"  free bar overtone 1: {beam_ratios(3)[1]:.4f}   marimba tunes it to 4.0")
    print(f"  free bar overtone 2: {beam_ratios(3)[2]:.4f}   marimba tunes it to 9.2 or 10")
    print("\nCircular membrane, first 10 ratios")
    print("  " + "  ".join(f"{r:.4f}" for r, _, _ in membrane_round(10)))
    print("\nSquare plate, first 10 ratios")
    print("  " + "  ".join(f"{r:.4f}" for r, _, _ in plate_rect(1.0, 10)))
    print("\nPartials below 20 kHz, from a 55 Hz fundamental")
    for name, ratios in (
        ("free-free beam", beam_ratios(60)),
        ("string", string_ratios(400)),
        ("stiff string B=3e-4", string_ratios(400, 3.0e-4)),
    ):
        n = sum(1 for r in ratios if r * 55.0 <= 20000.0)
        print(f"  {name:<22} {n}")


def compare(path):
    """Diff a dump from the engine against this file's own arithmetic."""
    print(f"\nComparing {path}")
    rows = {}
    with open(path) as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            obj, i, j, r = line.split(",")
            rows.setdefault(obj, []).append((int(i), int(j), float(r)))

    worst_all = 0.0
    for obj, got in sorted(rows.items()):
        if obj == "Beam":
            want = {(i, 0): r for i, r in enumerate(beam_ratios(len(got) + 2), start=1)}
        elif obj == "Tine":
            want = {(i, 0): r for i, r in enumerate(tine_ratios(len(got) + 2), start=1)}
        elif obj == "String":
            want = {(i, 0): r for i, r in enumerate(string_ratios(len(got) + 2), start=1)}
        elif obj == "Membrane":
            want = {(m, n): r for r, m, n in membrane_rect(1.0, 10**9)}
        elif obj == "Plate":
            want = {(m, n): r for r, m, n in plate_rect(1.0, 10**9)}
        elif obj == "PlateRound":
            want = {(m, n): r for r, m, n in plate_round(10**9)}
        elif obj == "MembraneRound":
            want = {(m, n): r for r, m, n in membrane_round(10**9)}
        else:
            print(f"  {obj}: no independent series here, skipped")
            continue
        worst = 0.0
        worst_at = None
        missing = 0
        for i, j, r in got:
            w = want.get((i, j))
            if w is None:
                missing += 1
                continue
            c = abs(cents(r, w))
            if c > worst:
                worst, worst_at = c, (i, j)
        worst_all = max(worst_all, worst)
        note = f" ({missing} not in the probe's own range)" if missing else ""
        print(f"  {obj:<16} worst {worst:.6f} cents at {worst_at}{note}")
    print(f"  -> worst over every object: {worst_all:.6f} cents")
    return worst_all


if __name__ == "__main__":
    ok = published_checks()
    orthonormality()
    tables()
    if "--compare" in sys.argv:
        compare(sys.argv[sys.argv.index("--compare") + 1])
    sys.exit(0 if ok else 1)
