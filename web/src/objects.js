/**
 * The ten objects, as the page knows them: what each one is called, which
 * engine produces it, what it is for, and where its numbers come from.
 *
 * **There is no arithmetic in this file and there must not be.** Every partial
 * series, every mode shape and every level belongs to the engine; this is the
 * catalogue the panel prints beside them. The equations that generate a series
 * live in `dev/physics/`, are loaded only in development, and exist for one
 * reason: so the page still renders before the plug-in is running.
 *
 * **The order is frozen and the list is append-only.** The first seven indices
 * are Corpus's own order, established from their presets, and a saved
 * project's object is its index — so nothing above may ever move. Everything
 * from the eighth on is ours, and each was appended rather than inserted.
 *
 * **This list is checked against the engine, not trusted.** It went stale
 * once: the engine appended Tine and Plate Round, this catalogue still had
 * eight entries, and `objectAt` clamps — so choosing either would have drawn
 * "Membrane Round" on the face while the audio thread ran a different object.
 * A wrong name printed confidently is worse than a blank, so `tools/gen-previews.mjs`
 * now fails outright if the engine's dump names an object this file does not
 * have, or the other way round.
 *
 * `derivation` and `source` are printed on the face and are not decoration.
 * Nine of the ten series are the solution of a stated closed form. The
 * marimba is the exception and is marked as one, in the warning colour: an
 * undercut bar is arrived at by cutting the arch until the partials land, so
 * no bare equation gives them. Marking that difference costs one word and it
 * is the difference between a citation and an ornament.
 */

/** @typedef {'modal'|'waveguide'} Engine */

/**
 * @typedef {object} ResonatorObject
 * @property {string} id
 * @property {string} label
 * @property {Engine} engine
 * @property {boolean} twoD Whether it has a surface, and so a second contact axis.
 * @property {string} short One line, for the key and the row.
 * @property {string} blurb What this object is and why it sounds like it does.
 * @property {'closed form'|'tuning target'} derivation Where its ratios come from.
 * @property {string} source The equation, or the practice, named.
 * @property {string} [caveat] What the citation does not cover, where that matters.
 * @property {string} uses What it is actually reached for.
 */

/** @type {ResonatorObject[]} */
export const OBJECTS = [
  {
    id: 'beam',
    label: 'Beam',
    engine: 'modal',
    twoD: false,
    short: 'a free bar',
    blurb:
      'A bar free at both ends. Its partials are nowhere near whole numbers — 1, 2.76, 5.40, 8.93 — so there is no fundamental for them to be harmonics of. That is why a glockenspiel clangs rather than sings.',
    derivation: 'closed form',
    source: 'the transverse beam equation: frequencies as the square of the roots of cos x · cosh x = 1',
    uses: 'Metallic percussion and bell tones. Feed it a click, a rim shot or a hi-hat and you get a glockenspiel’s clang, because there is no fundamental for the partials to be harmonics of.',
  },
  {
    id: 'marimba',
    label: 'Marimba',
    engine: 'modal',
    twoD: false,
    short: 'a bar, undercut',
    blurb:
      'The same bar with an arch cut out of the underside, which drops the second partial onto a whole ratio — 4 for a marimba, 3 for a xylophone — and the third to around 9 or 10. Tuning those two by hand is what turns a clang into a pitch, and Bar Tuning and Bar Third are those two decisions, left where the maker has them.',
    derivation: 'tuning target',
    source: 'bar-percussion tuning practice: the undercut is cut until partials 2 and 3 arrive, so no bare equation gives them',
    caveat:
      'the mode shapes here are the uniform bar’s, so the node positions Hit and Pos read from are an approximation — the arch moves those too',
    uses: 'Tuned mallet parts and wooden tone. The one to reach for when a transient should come back as a pitch rather than a clang — and the only object here whose tuning is a builder’s decision you can change.',
  },
  {
    id: 'string',
    label: 'String',
    engine: 'modal',
    twoD: false,
    short: 'under tension',
    blurb:
      'Whole-number partials, so every one of them reinforces the same fundamental and the ear hears one pitch. Real strings are a little stiff, which stretches the upper partials slightly sharp; that stretch is what Inharm reaches for.',
    derivation: 'closed form',
    source: 'the ideal string fixed at both ends: fₙ = n·f₁, mode shape sin(nπu)',
    uses: 'Anything that should read as pitched: guitar and piano body, drones struck out of a hi-hat, plucked bass from a click. The one place Inharm earns its keep, because a little stretch is what a real string does.',
  },
  {
    id: 'membrane',
    label: 'Membrane',
    engine: 'modal',
    twoD: true,
    short: 'a tensioned skin',
    blurb:
      'Two dimensions instead of one, so two mode indices run independently and the partials are dense, close-packed and share no common divisor. There is no pitch to speak of, which is why a drum is a drum. Ratio sets the shape of the rectangle.',
    derivation: 'closed form',
    source: 'the rectangular membrane: f ∝ √(a² + b²/r²) over both mode indices',
    uses: 'Untuned body on drums, and boxy resonance on anything. Rectangular, so Ratio changes the object rather than the tone — a wide aspect is a floor tom, a square one is a tight snare.',
  },
  {
    id: 'plate',
    label: 'Plate',
    engine: 'modal',
    twoD: true,
    short: 'a stiff sheet',
    blurb:
      'Two-dimensional as well, but held up by its own stiffness rather than by tension, and a stiff object’s frequencies go as the square of the membrane’s eigenvalue. So the same dense series climbs far faster and packs far tighter — a sheet of metal, not a drum head.',
    derivation: 'closed form',
    source: 'the thin-plate equation, simply supported: f ∝ a² + b²/r², the square of the membrane family',
    uses: 'Metal sheets, springs and thunder. The series climbs so fast that a transient comes back as a wash rather than a note, which is plate reverb’s cousin and the densest thing here.',
  },
  {
    id: 'pipe',
    label: 'Pipe',
    engine: 'waveguide',
    twoD: false,
    short: 'stopped at one end',
    blurb:
      'An air column closed at the far end. The round trip reflects +1 there and −1 at the mouth, so it comes back inverted and only the odd harmonics survive — and the column only needs to be a quarter of a wavelength, so the same length of pipe sounds an octave below an open one.',
    derivation: 'closed form',
    source: 'the one-dimensional wave equation with a pressure antinode at the far end: fₖ = (2k−1)·f₁',
    uses: 'Hollow and woody, an octave below where you expect. The stopped column keeps only the odd harmonics, which is a clarinet’s honk and something no filter will give you.',
  },
  {
    id: 'tube',
    label: 'Tube',
    engine: 'waveguide',
    twoD: false,
    short: 'open at both ends',
    blurb:
      'The same air column with the far end opened. Now it reflects −1 at both ends, the round trip comes back the way it left, and the whole harmonic series is there — over a column twice as long for the same pitch. Opening is that reflection, and it moves continuously between the two.',
    derivation: 'closed form',
    source: 'the one-dimensional wave equation with a pressure node at both ends: fₖ = k·f₁',
    uses: 'Flutes, blown bottles and didgeridoo drones. The whole harmonic series over a column twice as long as the stopped one at the same pitch, so it is the brighter and more open of the pair.',
  },
  {
    id: 'membrane_round',
    label: 'Membrane Round',
    engine: 'modal',
    twoD: true,
    short: 'a drum head',
    blurb:
      'A circular head fixed at the rim — an actual drum, where Corpus only offers a rectangle. Its partials are the zeros of the Bessel functions, 1 : 1.593 : 2.136 : 2.296, which share no common divisor with anything; and a circle has no aspect, so there is no Ratio to set.',
    derivation: 'closed form',
    source: 'the circular membrane fixed at the rim: f ∝ jₘₙ, the nth zero of the mth Bessel function',
    uses: 'Actual drums — toms, timpani, hand percussion. The rim is a node for every mode, so striking near the edge gives almost nothing and moving in towards the centre thins it to the round modes alone.',
  },
  {
    id: 'tine',
    label: 'Tine',
    engine: 'modal',
    twoD: false,
    short: 'a bar clamped at one end',
    blurb:
      'The same bar with one end held instead of free — a tuning fork’s prong, a music box’s tooth, an electric piano’s tine. Clamping it throws the partials much further apart: 1, 6.27, 17.5, where the free bar gives 2.76 and 5.40. The second partial is over two and a half octaves up, so there is nothing in the range where a glockenspiel clangs, and what is left reads as a pure and slightly hollow pitch.',
    derivation: 'closed form',
    source: 'the transverse beam equation clamped at one end: frequencies as the square of the roots of cos x · cosh x = −1',
    uses: 'Electric piano, music box and bell-like sine tones. The one object here that gives a near-pure pitch out of a struck body, because its overtones are too far up to argue with the fundamental — and moving Hit towards the clamped end is what puts the bark back in.',
  },
  {
    id: 'plate_round',
    label: 'Plate Round',
    engine: 'modal',
    twoD: true,
    short: 'a stiff disc',
    blurb:
      'A disc held at its rim and standing up by its own stiffness rather than by tension — a bell plate, a gong, the family a cymbal belongs to. Its partials are 1 : 2.08 : 3.41 : 3.89 : 5.00, far wider than the round head’s 1 : 1.59 : 2.14, because a stiff object’s frequencies go as the square of the eigenvalue where a tensioned one goes as the eigenvalue itself. Same rim, same outline, an entirely different instrument.',
    derivation: 'closed form',
    source: 'the thin-plate equation on a disc clamped at the rim: f ∝ λ², the roots of Jₘ(λ)Iₘ₊₁(λ) + Iₘ(λ)Jₘ₊₁(λ) = 0',
    caveat:
      'a real cymbal is free at its rim rather than clamped, and its crash is a nonlinearity no linear resonator has — this is the clamped disc, which is a bell plate',
    uses: 'Gongs, bell plates and metallic washes with a pitch in them. Denser than a bar and wider than a drum head, so it is the object to reach for when a hit should come back as metal that still has a note.',
  },
];

export const objectAt = (i) => OBJECTS[Math.max(0, Math.min(OBJECTS.length - 1, Math.round(i || 0)))];
export const objectById = (id) => OBJECTS.find((o) => o.id === id) || OBJECTS[0];

/** The two engines, as the browse view groups them. */
export const ENGINES = [
  {
    id: 'modal',
    label: 'Mode bank',
    hint: 'the object vibrates',
    note: 'The object itself vibrates. Its motion decomposes into normal modes, and each one is a decaying sinusoid — a single two-pole resonator, paid for one at a time.',
  },
  {
    id: 'waveguide',
    label: 'Waveguide',
    hint: 'the air inside does',
    note: 'The object is only a boundary; the air inside is what moves. A pair of delay lines with a reflection at each end gives every harmonic under Nyquist for the price of giving four.',
  },
];
