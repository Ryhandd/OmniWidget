# Build OmniWidget jadi installer Windows

## Opsi A — di laptop sendiri (sekali setup)
1. Install **Node.js 20+** — https://nodejs.org
2. Install **Rust** — https://rustup.rs (pilih default, toolchain MSVC)
3. Install **Visual Studio Build Tools** — centang workload *Desktop development with C++*
4. Di folder project:
   ```
   npm install
   npm run build
   ```
5. Hasil:
   - `src-tauri/target/release/bundle/nsis/OmniWidget_1.0.0_x64-setup.exe`  ← installer (disarankan)
   - `src-tauri/target/release/bundle/msi/OmniWidget_1.0.0_x64_en-US.msi`
   - `src-tauri/target/release/omniwidget.exe`  ← versi portable (butuh WebView2, sudah ada di Win 10/11)

Mode pengembangan (hot reload): `npm run dev`.

## Opsi B — GitHub Actions (tanpa install apa pun)
Push project ke GitHub → tab **Actions** → **Build Windows** → **Run workflow** → setelah selesai unduh artifact **OmniWidget-windows**.

## Catatan
- Installer belum ditandatangani (code signing berbayar), jadi SmartScreen bisa muncul: klik *More info → Run anyway*.
- Semua fitur Converter (gambar, PDF, audio, dokumen) berjalan tanpa memasang komponen tambahan.
  Pustaka **Pdfium** ikut dipaketkan installer sebagai resource, jadi PDF → gambar/teks langsung bisa dipakai.
- Autostart dimatikan default; nyalakan di **Pengaturan → Startup**.
