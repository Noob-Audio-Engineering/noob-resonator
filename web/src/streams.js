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
 * One field out of a frame, by name, or `null`.
 *
 * `null` for three different reasons that mean the same thing to a reader:
 * the build does not declare the field, no frame has arrived, or the engine
 * published a non-finite value for it.
 *
 * **A non-finite value is how an engine says "not computed".** A real zero
 * and an uncomputed zero are otherwise indistinguishable to a panel, which is
 * how the level meter once reported `0.0 dB GR` for a measurement nothing had
 * made. An unset field must be absent, not plausible.
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
