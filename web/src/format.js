/**
 * Printing a parameter's value, where the page has to fill in for the client.
 *
 * Kept free of the Vue layer and of the framework client for the same reason
 * `streams.js` is: this is what every knob on the panel prints through, so it
 * is worth testing, and a module that imports `.vue` files cannot be loaded by
 * the test runner.
 */

/**
 * A parameter's value as text, honouring the manifest's `decimals` hint.
 *
 * **The client this build ships against does not implement that hint yet**, so
 * the page does. The engine declares `decimals: 0` on the mode budget, on
 * Transpose and on the voice pitches — counts and semitones, where a fraction
 * means nothing — and the installed formatter ignores the field and falls back
 * to a magnitude rule: under ten it prints two decimals, under a hundred one.
 * So the Modes knob read **4.00** at its minimum and **32.0** stepped, which is
 * the false precision the hint exists to remove, across the whole range a mode
 * budget is actually used in.
 *
 * **This is not a second authority.** It is the framework's own declared
 * contract applied where the client has not caught up; when the client
 * implements it, both produce the same string and this becomes dead weight
 * rather than a disagreement. Enumerations and toggles are left alone, because
 * their rendering is the client's and has nothing to do with decimals.
 *
 * Worth recording how it came back, because the mistake is more useful than the
 * fix. The rounding lived on the page once as a one-control override and was
 * removed when the hint shipped — **verified against the live engine at the
 * default of 1024**, where the client's magnitude rule prints a clean integer
 * whatever the hint says. That is a value which could not fail. Checking 4 and
 * 32, where the rule changes, would have caught it in the same minute.
 *
 * @param {{ spec?: object, plain?: number, text?: string }|null} p
 */
export function valueText(p) {
  if (!p) return '—';
  const spec = p.spec;
  const d = spec?.decimals;
  if (d == null || !Number.isFinite(p.plain)) return p.text;
  if ((spec.labels && spec.labels.length) || spec.steps === 2) return p.text;
  const txt = p.plain.toFixed(Math.max(0, Math.min(6, Math.round(d))));
  return spec.unit ? `${txt} ${spec.unit}` : txt;
}
