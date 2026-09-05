// Connect to a running bridge, read the manifest, and report what arrived.
// Deliberately dumb: it does what the page's client does and nothing else, so
// that "the manifest never arrives" and "the manifest arrives and the page
// chokes on it" can be told apart.
const port = process.argv[2] || '4246';
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
ws.binaryType = 'arraybuffer';

let sawManifest = false;
const t0 = Date.now();
const timer = setTimeout(() => {
  if (!sawManifest) {
    console.log(`NO MANIFEST after ${Date.now() - t0} ms`);
    process.exit(1);
  }
}, 8000);

ws.onopen = () => console.log(`open after ${Date.now() - t0} ms`);
ws.onerror = (e) => console.log('error', e.message || e.type);
ws.onclose = (e) => console.log(`closed code=${e.code} reason=${JSON.stringify(e.reason)} clean=${e.wasClean}`);

const counts = {};
ws.onmessage = (ev) => {
  if (typeof ev.data !== 'string') {
    counts.binary = (counts.binary || 0) + 1;
    return;
  }
  let m;
  try {
    m = JSON.parse(ev.data);
  } catch (err) {
    console.log(`UNPARSEABLE text frame, ${ev.data.length} bytes: ${err.message}`);
    console.log(ev.data.slice(0, 200));
    return;
  }
  counts[m.t] = (counts[m.t] || 0) + 1;
  if (m.t !== 'manifest') return;
  sawManifest = true;
  clearTimeout(timer);
  console.log(`manifest after ${Date.now() - t0} ms, ${ev.data.length} bytes`);
  console.log(`  params  ${m.params?.length}`);
  console.log(`  streams ${m.streams?.length}`);
  const meta = m.meta || {};
  console.log(`  meta keys ${Object.keys(meta).join(', ')}`);
  console.log(`  objects ${Array.isArray(meta.objects) ? meta.objects.length : 'MISSING'}`);
  const ids = (m.params || []).map((p) => p.id);
  console.log(`  first ids ${ids.slice(0, 6).join(' ')}`);
  console.log(`  has modes param: ${ids.includes('modes')}`);
  const dup = ids.filter((x, i) => ids.indexOf(x) !== i);
  if (dup.length) console.log(`  DUPLICATE param ids: ${dup.join(' ')}`);
  for (const p of m.params || []) {
    for (const k of ['min', 'max', 'default']) {
      if (typeof p[k] === 'number' && !Number.isFinite(p[k])) {
        console.log(`  NON-FINITE ${k} on ${p.id}: ${p[k]}`);
      }
    }
  }
  for (const s of m.streams || []) {
    console.log(`  stream ${s.id} cap=${s.capacity} kind=${s.kind} sticky=${s.sticky}`);
  }
  setTimeout(() => {
    console.log(`frames in 3 s: ${JSON.stringify(counts)}`);
    process.exit(0);
  }, 3000);
};
