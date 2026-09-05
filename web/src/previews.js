/**
 * Characteristic partial ratios, one row per object, for the browse view's
 * previews — **a generated table, not a computation.**
 *
 * The panel renders and does not compute: every partial it draws for the
 * loaded object arrives on the `modes` stream. A browser showing every object
 * at once cannot do that, because only one of them is loaded, and solving ten
 * eigenvalue problems in the front end to draw ten thumbnails is exactly what
 * this architecture forbids. So the rows read a table.
 *
 * **These are the shape of each series, not your settings.** They are taken at
 * reference settings — default aspect, no inharmonicity — so a row shows what
 * an object *is* rather than what it would sound like at your current damping,
 * which is the comparison the browser exists to make anyway.
 *
 * **Generated from the engine** by `tools/gen-previews.mjs`, out of
 * `cargo run --release --bin benchmark -- --dump series`. Rerun it when the
 * engine's series move. `PREVIEW_SOURCE` says, per row, whether the numbers
 * came off the engine or from the closed form the catalogue cites — the air
 * columns are the second, because a waveguide has no mode list to dump.
 *
 * @type {Record<string, number[]>}
 */
export const PREVIEW_RATIOS = {
  beam: [1, 2.75654, 5.40392, 8.93295, 13.3443, 18.6379, 24.8138, 31.8719, 39.8123, 48.635, 58.3399, 68.9271, 80.3966, 92.7483, 105.982, 120.099, 135.097, 150.978, 167.741, 185.386, 203.914, 223.324, 243.616, 264.79, 286.847, 309.786, 333.607, 358.311, 383.896],
  marimba: [1, 4, 9.2, 15.2081, 22.7182, 31.7304, 42.2446, 54.2609, 67.7792, 82.7995, 99.3218, 117.346, 136.873, 157.901, 180.432, 204.464, 229.999, 257.035, 285.574, 315.614, 347.157, 380.202],
  string: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40],
  membrane: [1, 1.58114, 1.58114, 2, 2.23607, 2.23607, 2.54951, 2.54951, 2.91548, 2.91548, 3, 3.16228, 3.16228, 3.53553, 3.53553, 3.60555, 3.80789, 4, 4.12311, 4.30116, 4.47214, 4.52769, 4.74342, 5, 5.09902, 5.14782, 5.38516, 5.70088, 5.70088, 5.83095, 6.04152, 6.32456, 6.40312, 6.5192, 6.7082, 6.96419, 7.10634, 7.2111, 7.38241, 7.61577],
  plate: [1, 2.5, 2.5, 4, 5, 5, 6.5, 6.5, 8.5, 8.5, 9, 10, 10, 12.5, 12.5, 13, 13, 14.5, 14.5, 16, 17, 17, 18.5, 18.5, 20, 20, 20.5, 20.5, 22.5, 22.5, 25, 25, 25, 26, 26, 26.5, 26.5, 29, 29, 30.5],
  pipe: [1, 3.00001, 5.00004, 7.00012, 9.00026, 11.0005, 13.0008, 15.0012, 17.0017, 19.0023, 21.0031, 23.0041, 25.0052, 27.0064, 29.0078, 31.0094, 33.0112, 35.0131, 37.0153, 39.0176, 41.0201, 43.0227, 45.0255, 47.0285, 49.0317, 51.035, 53.0385, 55.0422, 57.0459, 59.0499, 61.0539, 63.0581, 65.0625, 67.0669, 69.0715, 71.0762, 73.0809, 75.0858, 77.0908, 79.0958],
  tube: [1, 2, 3.00001, 4.00003, 5.00005, 6.00009, 7.00015, 8.00022, 9.00031, 10.0004, 11.0006, 12.0007, 13.0009, 14.0011, 15.0014, 16.0017, 17.002, 18.0024, 19.0028, 20.0032, 21.0037, 22.0042, 23.0047, 24.0053, 25.0059, 26.0066, 27.0073, 28.0081, 29.0089, 30.0097, 31.0106, 32.0115, 33.0125, 34.0135, 35.0146, 36.0156, 37.0168, 38.0179, 39.0191, 40.0204],
  membrane_round: [1, 1.59334, 2.13555, 2.29542, 2.65307, 2.9173, 3.15546, 3.50015, 3.59848, 3.64745, 4.05893, 4.13174, 4.23044, 4.60104, 4.61005, 4.83189, 4.90328, 5.08357, 5.13077, 5.41212, 5.5404, 5.55313, 5.65084, 5.97654, 6.01936, 6.15261, 6.16314, 6.20873, 6.48274, 6.52861, 6.669, 6.74621, 6.84899, 6.94364, 7.07071, 7.16943, 7.32526, 7.40238, 7.46824, 7.5145],
  tine: [1, 6.26689, 17.5475, 34.3861, 56.8426, 84.913, 118.598, 157.896, 202.809, 253.336, 309.476, 371.231],
  plate_round: [1, 2.08112, 3.41402, 3.89309, 4.99519, 5.95436, 6.8194, 8.27957, 8.72217, 8.8822, 10.8676, 11.18, 11.7542, 13.7097, 13.7148, 15.0565, 15.4842, 16.469, 16.8173, 18.6283, 19.4557, 19.4848, 20.1717, 22.467, 22.6681, 23.7593, 23.7747, 24.1788, 26.1045, 26.5694, 27.6236, 28.307, 29.1469, 29.7636, 30.9326, 31.7158, 33.1262, 33.6443, 34.3913, 34.8057],
};

/**
 * Where each row's numbers came from. Printed in the browser, because "these
 * are the engine's partials" and "these are the equation's partials" are not
 * the same claim and the face does not get to blur them.
 *
 * @type {Record<string, 'engine'|'closed form'>}
 */
export const PREVIEW_SOURCE = {
  beam: 'engine',
  marimba: 'engine',
  string: 'engine',
  membrane: 'engine',
  plate: 'engine',
  pipe: 'engine',
  tube: 'engine',
  membrane_round: 'engine',
  tine: 'engine',
  plate_round: 'engine',
};
