/**
 * Reading a stream by the names it declares.
 *
 * **Every stream field on this page is looked up by name, never by offset.**
 * The panel asks for `db_bare`; a build that does not publish it gets nothing
 * back and the node ghosts are simply not drawn. Reading by offset would
 * print whichever field happened to sit at that index instead — a wrong
 * number structurally indistinguishable from a right one, which is the
 * failure this exists to make impossible. It also means the engine can add or
 * move a field without breaking a panel.
 *
 * Kept free of the Vue layer and of the framework client on purpose: these
 * two functions are what every optional readout rests on, so they are worth
 * testing, and a module that imports `.vue` files cannot be loaded by the
 * test runner.
 */

/** A `layout` string from a stream's meta, as names, a stride and an index. */
export function parseLayout(layout) {
  const names = String(layout || '')
    .split(',')
    .map((n) => n.trim())
    .filter(Boolean);
  return { names, stride: names.length, index: Object.fromEntries(names.map((n, i) => [n, i])) };
}

/**
 * Whether a stream declares a field at all.
 *
 * **Not the same question as whether it has a value, and the difference is a
 * sentence on the face.** [`fieldAt`] deliberately collapses three cases into
 * `null`, which is right for anything that draws a number and wrong for
 * anything that explains why there is not one:
 *
 * * the build has no such field — a gap, and worth reporting as one;
 * * the field is declared and the engine published a non-finite value — which
 *   usually means *this does not apply*, and is a correct state rather than a
 *   fault.
 *
 * Collapsing them printed the good case as a fault. On a string holding every
 * partial it has there is no wall to draw, so the engine publishes `NaN` for
 * `ceiling_hz` — and the display announced *no ceiling_hz, so where the bank
 * runs out is not marked*, which a reader takes for a broken build. The truth
 * was that nothing had been thrown away. This is how a reader tells the two
 * apart: the layout says what exists, the frame says what was computed.
 */
export const declares = (layout, name) => layout?.index?.[name] != null;

/**
 * One field out of a frame, by name, or `null`.
 *
 * `null` for three different reasons that mean the same thing *to a number*:
 * the build does not declare the field, no frame has arrived, or the engine
 * published a non-finite value for it. Ask [`declares`] when the difference
 * matters, which is whenever the panel is about to explain the absence rather
 * than draw around it.
 *
 * **A non-finite value is how an engine says "not computed" or "not
 * applicable".** A real zero and an uncomputed zero are otherwise
 * indistinguishable to a panel, which is how the level meter once reported
 * `0.0 dB GR` for a measurement nothing had made. An unset field must be
 * absent, not plausible.
 *
 * @param {ArrayLike<number>|null} frame
 * @param {{ index: Record<string, number> }} layout
 * @param {string} name
 * @param {number} [offset] Start of the record, for a frame of repeating records.
 */
export function fieldAt(frame, layout, name, offset = 0) {
  const i = layout.index[name];
  if (i == null || !frame) return null;
  const v = frame[offset + i];
  return Number.isFinite(v) ? v : null;
}

/**
 * The largest integer a 32-bit float carries exactly.
 *
 * Past `2²⁴` a float is spaced more than one apart, so a number claiming to be
 * a count of things has already lost the ability to be one.
 */
export const COUNT_MAX = 2 ** 24;

/**
 * Whether a value can be a count of things at all.
 *
 * **A number that cannot be a count is not a measurement, and the panel does
 * not print it.** This is the same rule as "a non-finite value means not
 * computed", carried one step further, and it exists because of a real frame:
 * with the fundamental driven to 1.2 Hz the object had more partials under
 * Nyquist than a count can hold, and `modes_available` arrived as
 * **1.8446744e19** — which is `u64::MAX`, but by an unsigned *cast of an
 * infinity* rather than by a subtraction running past zero. The engine's
 * inharmonicity has a real asymptote, so above it every partial an ideal
 * string has genuinely does fit under the axis: infinitely many, in a finite
 * band. That is true, and it is simply not a number a count can be.
 *
 * The panel printed it faithfully as *this object has 18446744073709552.0 k
 * partials*, which is the failure this project keeps meeting from the other
 * side: a number arriving in the right field, in the right units, that nothing
 * measured. **The rule does not depend on the cause** — which is the point of
 * putting it here rather than waiting for the engine to be right.
 *
 * Refusing it is not the same as hiding it. The reader that uses this also
 * reports which fields it refused, so the panel says the engine published
 * something that is not a count rather than quietly showing a dash.
 */
export const isCount = (v) => v != null && Number.isFinite(v) && v >= 0 && v <= COUNT_MAX && Number.isInteger(v);

/**
 * One field out of a frame that is a count of things, or `null` when it is
 * absent, not computed, or not a count.
 */
export function countAt(frame, layout, name, offset = 0) {
  const v = fieldAt(frame, layout, name, offset);
  return isCount(v) ? v : null;
}
