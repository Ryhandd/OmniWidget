// Unduh tiap aset di sidecars.manifest.json, hitung SHA-256 dan ukuran, lalu tulis kembali.
// Pakai:  npm run checksums          (hanya entri yang masih kosong)
//         npm run checksums -- --force (hitung ulang semua)
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const file = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri', 'sidecars.manifest.json');
const force = process.argv.includes('--force');
const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));

for (const [name, entry] of Object.entries(manifest.sidecars)) {
  for (const [plat, asset] of Object.entries(entry.platforms)) {
    if (asset.sha256 && !force) continue;
    if (asset.url.endsWith('.pkg')) { console.warn(`! ${name}/${plat}: .pkg tidak didukung extractor, dilewati`); continue; }
    if (asset.url.includes('/latest/')) console.warn(`! ${name}/${plat}: URL "latest" bisa berubah isi; checksum akan basi`);
    process.stdout.write(`${name}/${plat} … `);
    const res = await fetch(asset.url, { redirect: 'follow' });
    if (!res.ok) { console.log(`HTTP ${res.status}, dilewati`); continue; }
    const hash = crypto.createHash('sha256');
    let size = 0;
    for await (const chunk of res.body) { hash.update(chunk); size += chunk.length; }
    asset.sha256 = hash.digest('hex');
    asset.size = size;
    console.log(`${asset.sha256.slice(0, 12)}… (${(size / 1048576).toFixed(1)} MB)`);
    fs.writeFileSync(file, JSON.stringify(manifest, null, 2) + '\n'); // simpan bertahap
  }
}
console.log('Selesai.');
