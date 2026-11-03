// Tes logika tab overflow tanpa framework: transpile tabs.ts dengan esbuild (sudah ikut Vite).
import { build } from 'esbuild';
import assert from 'node:assert/strict';

const out = await build({ entryPoints: ['src/core/tabs.ts'], bundle: true, format: 'esm', write: false });
const url = 'data:text/javascript;base64,' + Buffer.from(out.outputFiles[0].text).toString('base64');
const { splitTabs } = await import(url);

const mk = (n) => Array.from({ length: n }, (_, i) => ({ id: `m${i}` }));
const ids = (a) => a.map((t) => t.id);

let r = splitTabs(mk(4), 'm0');
assert.deepEqual(ids(r.visible), ['m0', 'm1', 'm2', 'm3']); assert.equal(r.overflow.length, 0);

r = splitTabs(mk(6), 'm5');
assert.equal(r.overflow.length, 0, 'tepat 6 tab: tanpa More');

r = splitTabs(mk(10), 'm1');
assert.deepEqual(ids(r.visible), ['m0', 'm1', 'm2', 'm3', 'm4', 'm5']);
assert.equal(r.overflow.length, 4);

r = splitTabs(mk(10), 'm8');
assert.equal(r.visible.length, 6);
assert.ok(ids(r.visible).includes('m8'), 'tab aktif di overflow harus dinaikkan');
assert.equal(r.visible.length + r.overflow.length, 10, 'tidak ada tab hilang/ganda');
assert.equal(new Set([...ids(r.visible), ...ids(r.overflow)]).size, 10);

r = splitTabs(mk(10), 'tidak-ada');
assert.deepEqual(ids(r.visible), ['m0', 'm1', 'm2', 'm3', 'm4', 'm5']);
console.log('tabs: semua tes lolos');
