# Graph Report - OmniWidget  (2026-11-06)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 1679 nodes · 3767 edges · 75 communities (64 shown, 11 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 108 edges (avg confidence: 0.84)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `4293ab78`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- anatomy/index.tsx
- qr.rs
- ClipRow.tsx
- pomodoro.rs
- spotify/state.ts
- window_manager.rs
- Ring
- llm/mod.rs
- spotify/mod.rs
- privacy.rs
- todo.rs
- ClipState
- package.json
- pdf.rs
- api.rs
- callback.rs
- staging.rs
- Storage
- SpotifyError
- converter/mod.rs
- ConvError
- pkce.rs
- bridge.ts
- images.rs
- bundle
- crypto.rs
- ShellState
- output.rs
- intake.ts
- preact
- geometry.rs
- clipboard/mod.rs
- sysmon.rs
- overlay/main.tsx
- converter/index.tsx
- todo/index.tsx
- qr/index.tsx
- pomodoro.ts
- controller.ts
- job.rs
- raster.rs
- control_req
- Format
- with
- Sealer
- indicators.ts
- lifecycle.rs
- manager
- sidecar.rs
- Control
- compilerOptions
- converter/api.ts
- run
- Inner
- test-tabs.mjs
- sysmon/index.tsx
- commands.rs
- arc
- fill-checksums.mjs
- InstallEvent
- hash_text
- serialize
- .apply_patch
- formats.rs
- default.json
- Engine
- autostart.rs
- extract_archive
- omniwidget

## God Nodes (most connected - your core abstractions)
1. `SpotifyError` - 54 edges
2. `ConvError` - 48 edges
3. `ClipState` - 41 edges
4. `Storage` - 41 edges
5. `preact` - 35 edges
6. `Ring` - 29 edges
7. `Format` - 25 edges
8. `ShellState` - 23 edges
9. `lock()` - 23 edges
10. `Item` - 22 edges

## Surprising Connections (you probably didn't know these)
- `JobOptions` --references--> `Pages`  [INFERRED]
  src-tauri/src/plugins/converter/mod.rs → src-tauri/src/plugins/converter/pdf.rs
- `Plan` --references--> `Pages`  [INFERRED]
  src-tauri/src/plugins/converter/mod.rs → src-tauri/src/plugins/converter/pdf.rs
- `remember()` --references--> `Tokens`  [INFERRED]
  src-tauri/src/plugins/spotify/mod.rs → src-tauri/src/plugins/spotify/api.rs
- `percent_decode()` --calls--> `hex()`  [INFERRED]
  src-tauri/src/plugins/converter/staging.rs → src-tauri/src/plugins/qr.rs
- `clipboard_clear()` --calls--> `blocking()`  [INFERRED]
  src-tauri/src/plugins/clipboard/mod.rs → src-tauri/src/plugins/llm/mod.rs

## Import Cycles
- 1-file cycle: `src-tauri/src/plugins/clipboard/crypto.rs -> src-tauri/src/plugins/clipboard/crypto.rs`
- 2-file cycle: `src-tauri/src/core/lifecycle.rs -> src-tauri/src/core/window_manager.rs -> src-tauri/src/core/lifecycle.rs`
- 2-file cycle: `src-tauri/src/core/lifecycle.rs -> src-tauri/src/core/tray.rs -> src-tauri/src/core/lifecycle.rs`
- 3-file cycle: `src-tauri/src/core/lifecycle.rs -> src-tauri/src/core/tray.rs -> src-tauri/src/core/window_manager.rs -> src-tauri/src/core/lifecycle.rs`

## Communities (75 total, 11 thin omitted)

### Community 0 - "anatomy/index.tsx"
Cohesion: 0.06
Nodes (69): errText(), llm, LlmConfigInput, LlmProvider, LlmStatus, FormatToggle(), Props, IdeaPanel() (+61 more)

### Community 1 - "qr.rs"
Cohesion: 0.06
Nodes (40): capture_close(), capture_patch(), capture_pick(), capture_qr(), capture_recent(), capture_start(), CaptureState, cleanup() (+32 more)

### Community 2 - "ClipRow.tsx"
Cohesion: 0.07
Nodes (42): clipApi, onClipChanged(), ClipRow(), Props, base, IconExpand(), IconGear(), IconImage() (+34 more)

### Community 3 - "pomodoro.rs"
Cohesion: 0.08
Nodes (36): MAX_PARK, now_ms(), poke(), Scheduler, start(), Timed, cfg(), commit() (+28 more)

### Community 4 - "spotify/state.ts"
Cohesion: 0.08
Nodes (46): copyText(), spotifyApi, ConnectPanel(), Props, Controls(), Props, REPEAT_LABEL, DevicePicker() (+38 more)

### Community 5 - "window_manager.rs"
Cohesion: 0.08
Nodes (31): active_area(), ANIM_GEN, animate_to(), err(), EXPANDED_DIP, FRAME_MS, Mode, Expanded (+23 more)

### Community 6 - "Ring"
Cohesion: 0.11
Nodes (20): anggaran_disk_menggusur_gambar_tertua_tapi_tidak_yang_terbaru_atau_tersemat(), anggaran_thumbnail_ram_melepas_yang_tertua_dulu(), batas_sematan_dan_pelepasan_menggusur(), CAPACITY, duplikat_pindah_ke_depan_dan_tetap_tersemat(), image(), IMAGE_DISK_BUDGET, Item (+12 more)

### Community 7 - "llm/mod.rs"
Cohesion: 0.10
Nodes (33): ANTHROPIC_VERSION, api_error(), base_url_https_wajib_kecuali_localhost(), build_request(), cfg(), CONFIG_KEY, config_round_trip_dan_secret_tidak_bocor_ke_status(), DEFAULT_MAX_TOKENS (+25 more)

### Community 8 - "spotify/mod.rs"
Cohesion: 0.12
Nodes (28): Access, bearer(), cover_url_ok(), EXPIRY_SKEW, forget_session(), HTTP_TIMEOUT, image_mime(), KEY_CLIENT_ID (+20 more)

### Community 9 - "privacy.rs"
Cohesion: 0.08
Nodes (25): clipboard_write(), MAX_BYTES, blocks_recording(), can_include_in_history_dword_dan_gagal_tertutup(), DENY_APPS, denylist_aplikasi(), fmts(), format_sensitif_tak_peka_huruf_besar_kecil() (+17 more)

### Community 10 - "todo.rs"
Cohesion: 0.14
Nodes (26): blocking(), changed(), clean_memvalidasi_input(), COLS, EVENT, get(), init(), input() (+18 more)

### Community 11 - "ClipState"
Cohesion: 0.18
Nodes (17): boot(), clipboard_clear(), clipboard_snapshot(), ClipSnapshot, ClipState, fs_count(), gambar_thumbnail_dilepas_dimuat_ulang_dan_berkas_ikut_terhapus(), keluar_membersihkan_sesuai_pengaturan() (+9 more)

### Community 12 - "package.json"
Cohesion: 0.08
Nodes (30): dependencies, preact@^10.19.3, @tauri-apps/plugin-dialog@^2.0.0, devDependencies, autoprefixer@^10.4.20, postcss@^8.4.49, @preact/preset-vite@^2.8.2, tailwindcss@^3.4.17 (+22 more)

### Community 13 - "pdf.rs"
Cohesion: 0.11
Nodes (27): convert(), MAX_EXPORT_PAGES, Pages, All, First, plan_page(), render_page(), A4 (+19 more)

### Community 14 - "api.rs"
Cohesion: 0.12
Nodes (26): API_BASE, api_error(), Device, error_token_invalid_grant_berarti_login_ulang(), exchange_code(), MAX_BODY_BYTES, parse_device(), parse_devices() (+18 more)

### Community 15 - "callback.rs"
Cohesion: 0.12
Nodes (20): decide(), Hit, hit(), keputusan_menerima_hanya_state_yang_cocok(), page!, MAX_HEAD_BYTES, PAGE_BAD, PAGE_DENIED (+12 more)

### Community 16 - "staging.rs"
Cohesion: 0.14
Nodes (24): check_size(), converter_stage_upload(), COUNTER, default_out_dir(), discard(), ekstensi_palsu_tidak_lolos_tapi_isi_valid_tanpa_ekstensi_lolos(), MAX_AGE, MAX_EXT_CHARS (+16 more)

### Community 17 - "Storage"
Cohesion: 0.15
Nodes (14): blocking(), kunci_secret_ditolak_untuk_perintah_webview_tetapi_boleh_untuk_kode_rust(), kv_delete(), kv_get(), kv_set(), MAX_KEY_LEN, MAX_VALUE_BYTES, reject_secret() (+6 more)

### Community 18 - "SpotifyError"
Cohesion: 0.08
Nodes (19): SpotifyError, Api, BadClientId, Browser, Cancelled, Denied, Forbidden, Invalid (+11 more)

### Community 19 - "converter/mod.rs"
Cohesion: 0.16
Nodes (20): detect(), converter_cancel(), converter_probe(), converter_start(), engine_available(), execute(), JobEvent, Cancelled (+12 more)

### Community 20 - "ConvError"
Cohesion: 0.07
Nodes (20): ConvError, BadOutDir, BadPageSize, Cancelled, EmptyUpload, Image, InputMissing, Io (+12 more)

### Community 21 - "pkce.rs"
Cohesion: 0.10
Nodes (14): AUTHORIZE_URL, authorize_url(), CALLBACK_PATH, challenge_of(), challenge_sesuai_vektor_rfc_7636_lampiran_b(), Pkce, random_b64(), REDIRECT_PORT (+6 more)

### Community 22 - "bridge.ts"
Cohesion: 0.11
Nodes (19): @tauri-apps/api@^2.0.0, @tauri-apps/api, EVENTS, startActivityReporter(), bridge, startIndicatorBridge(), AiSection(), InstallEvent (+11 more)

### Community 23 - "images.rs"
Cohesion: 0.17
Nodes (24): aad(), Analysis, analyze(), analyze_memberi_ukuran_asli_dan_thumbnail_dalam_batas(), delete(), exists(), gambar_derau_turun_ke_sisi_lebih_kecil_agar_muat(), gambar_kecil_tidak_diperbesar_dan_png_rusak_ditolak() (+16 more)

### Community 24 - "bundle"
Cohesion: 0.07
Nodes (26): app, macOSPrivateApi, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl (+18 more)

### Community 25 - "crypto.rs"
Cohesion: 0.11
Nodes (13): create(), decode(), entry(), kunci_tetap_sama_menghasilkan_sealer_yang_saling_kompatibel(), menolak_kunci_salah_aad_salah_dan_data_diubah(), NONCE_LEN, OsKeyStore, random_key() (+5 more)

### Community 26 - "ShellState"
Cohesion: 0.13
Nodes (5): download_file(), AwakeGuard, DEFAULT_SLEEP_SECS, hold_dihitung_dan_idle_diulang_saat_yang_terakhir_lepas(), ShellState

### Community 27 - "output.rs"
Cohesion: 0.13
Nodes (19): finalize(), finalize_dir_memberi_sufiks(), finalize_file_menimpa_placeholder_bukan_berkas_pengguna(), guard_membersihkan_kecuali_disarm(), MAX_STEM_CHARS, MAX_SUFFIX_TRIES, OutKind, Dir (+11 more)

### Community 28 - "intake.ts"
Cohesion: 0.15
Nodes (23): useFileDrop(), converterApi, Candidate, extFromMime(), extOf(), filesFromClipboard(), hasFiles(), isEditableTarget() (+15 more)

### Community 29 - "preact"
Cohesion: 0.16
Nodes (20): preact, ModuleDef, MODULES, MAX_VISIBLE_TABS, splitTabs(), App(), clear(), isEditing() (+12 more)

### Community 30 - "geometry.rs"
Cohesion: 0.17
Nodes (24): anchored_resize_clamps_at_edges(), anchored_resize_keeps_center(), anchored_x(), Area, clamp_into(), ease_out_cubic(), easing_endpoints(), free_position_is_clamped_inside_area() (+16 more)

### Community 31 - "clipboard/mod.rs"
Cohesion: 0.14
Nodes (14): clipboard_paste(), clipboard_pin(), clipboard_remove(), clipboard_text(), clipboard_thumb(), DEBOUNCE, EVENT, on_exit() (+6 more)

### Community 32 - "sysmon.rs"
Cohesion: 0.19
Nodes (15): daftar_proteksi(), ensure(), is_protected(), ProcGroup, PROTECTED, release_heap(), Sample, self_rss() (+7 more)

### Community 33 - "overlay/main.tsx"
Cohesion: 0.17
Nodes (17): ColorFormat, formatColor(), FORMATS, hue(), inkOn(), Rgb, toHex(), toHsl() (+9 more)

### Community 34 - "converter/index.tsx"
Cohesion: 0.17
Nodes (17): DropZone(), Props, JobStatus(), label(), OutputFolder(), Props, REQUIRES_LABEL, TargetPicker() (+9 more)

### Community 35 - "todo/index.tsx"
Cohesion: 0.15
Nodes (20): api, Editor(), group(), Input, RowProps, Task, TaskRow(), TodoModule() (+12 more)

### Community 36 - "qr/index.tsx"
Cohesion: 0.18
Nodes (18): islandHold, subs, withHold(), highlight(), Maker(), QrModule(), Result(), Scanner() (+10 more)

### Community 37 - "pomodoro.ts"
Cohesion: 0.19
Nodes (19): act(), fmt(), ICON, Phase, PHASE_LABEL, pomo, PomoConfig, PomoSnap (+11 more)

### Community 38 - "controller.ts"
Cohesion: 0.17
Nodes (19): dispatch(), subs, syncIndicator(), useConverter(), Action, canConvert(), firstAvailable(), initialState (+11 more)

### Community 39 - "job.rs"
Cohesion: 0.20
Nodes (6): antrean_membatasi_konkurensi(), DEFAULT_CONCURRENCY, id_unik_dan_cancel_menyalakan_token(), JobHandle, JobRegistry, pembatalan_membangunkan_job_yang_antre()

### Community 40 - "raster.rs"
Cohesion: 0.16
Nodes (5): convert(), MAX_MEGAPIXELS, convert(), flatten_on_white(), MAX_MEGAPIXELS

### Community 41 - "control_req"
Cohesion: 0.17
Nodes (12): control_req(), devices_req(), player_req(), Reply, Req, request_kontrol(), send(), valid_device_id() (+4 more)

### Community 42 - "Format"
Cohesion: 0.11
Nodes (18): Format, Bmp, Docx, Flac, Gif, Html, Ico, Jpg (+10 more)

### Community 43 - "with"
Cohesion: 0.11
Nodes (17): runs-on, steps, jobs, build, name, on, push, workflow_dispatch (+9 more)

### Community 44 - "Sealer"
Cohesion: 0.20
Nodes (11): Sealer, NoKeychain, open(), AAD, bolak_balik_menjaga_urutan_sematan_dan_id_u64_besar(), isi_di_disk_terenkripsi_dan_kunci_salah_tidak_membuka(), item(), load_all() (+3 more)

### Community 45 - "indicators.ts"
Cohesion: 0.14
Nodes (14): FromRust, Indicator, indicators, items, Msg, subs, timers, useIndicators() (+6 more)

### Community 46 - "lifecycle.rs"
Cohesion: 0.22
Nodes (12): build_main(), eligible(), enter_deep_sleep(), is_asleep(), MAIN, RECHECK, shell_activity(), shell_deep_sleep() (+4 more)

### Community 48 - "sidecar.rs"
Cohesion: 0.23
Nodes (6): CancelToken, convert(), ffmpeg_args(), pandoc_args(), resolve(), run()

### Community 49 - "Control"
Cohesion: 0.13
Nodes (14): Control, Next, Pause, Play, Previous, Repeat, Seek, Shuffle (+6 more)

### Community 50 - "compilerOptions"
Cohesion: 0.13
Nodes (14): compilerOptions, jsx, jsxImportSource, module, moduleResolution, noEmit, paths, skipLibCheck (+6 more)

### Community 51 - "converter/api.ts"
Cohesion: 0.36
Nodes (8): loadPrefs(), savePrefs(), Props, DEFAULT_PREFS, OutputPrefs, parsePrefs(), PREFS_KEY, useOutputPrefs()

### Community 52 - "run"
Cohesion: 0.30
Nodes (11): wake(), init(), all_areas(), cursor_area(), Placement, RestoreFree, TopCenter, show() (+3 more)

### Community 53 - "Inner"
Cohesion: 0.18
Nodes (7): KeyStore, ClipItemDto, Inner, MemKeys, Kind, Image, Text

### Community 55 - "sysmon/index.tsx"
Cohesion: 0.36
Nodes (7): Card(), gb(), Group, mb(), Sample, SysmonModule(), TopApps()

### Community 56 - "commands.rs"
Cohesion: 0.47
Nodes (6): get_current_platform(), resolve_dir(), sidecar_cancel(), sidecar_install(), sidecar_status(), uuid_v4_simple()

### Community 58 - "fill-checksums.mjs"
Cohesion: 0.25
Nodes (3): file, force, manifest

### Community 59 - "InstallEvent"
Cohesion: 0.25
Nodes (8): InstallEvent, Cancelled, Downloading, Extracting, Failed, Installed, Started, Verifying

### Community 60 - "hash_text"
Cohesion: 0.57
Nodes (4): Fnv, fnv_vektor_standar_dan_domain_terpisah(), hash_image(), hash_text()

### Community 61 - "serialize"
Cohesion: 0.43
Nodes (4): Manifest, parse_manifest(), PlatformAsset, SidecarEntry

### Community 62 - ".apply_patch"
Cohesion: 0.52
Nodes (4): clipboard_set_settings(), ClipSettings, ClipSettingsPatch, keychain_tak_tersedia_menolak_persist_dan_start_tetap_aman()

### Community 63 - "formats.rs"
Cohesion: 0.52
Nodes (6): AUDIO, find(), IMAGES, Route, routes(), targets_for()

### Community 64 - "default.json"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 65 - "Engine"
Cohesion: 0.33
Nodes (5): Engine, Ffmpeg, Native, Pandoc, Pdfium

### Community 67 - "extract_archive"
Cohesion: 0.70
Nodes (4): extract_archive(), extract_tar_gz(), extract_tar_xz(), extract_zip()

## Knowledge Gaps
- **105 isolated node(s):** `All`, `First`, `InvalidSize`, `TooLarge`, `Api` (+100 more)
  These have ≤1 connection - possible missing edges. (Counts symbols only; 487 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **11 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `preact` connect `preact` to `anatomy/index.tsx`, `overlay/main.tsx`, `ClipRow.tsx`, `todo/index.tsx`, `qr/index.tsx`, `pomodoro.ts`, `controller.ts`, `spotify/state.ts`, `package.json`, `indicators.ts`, `converter/api.ts`, `bridge.ts`, `sysmon/index.tsx`, `intake.ts`?**
  _High betweenness centrality (0.055) - this node is a cross-community bridge._
- **What connects `All`, `First`, `InvalidSize` to the rest of the system?**
  _105 weakly-connected nodes found - possible missing edges._
- **Should `anatomy/index.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.06259183073758448 - nodes in this community are weakly interconnected._
- **Why does `ConvError` connect `ConvError` to `raster.rs`, `pdf.rs`, `staging.rs`, `sidecar.rs`, `converter/mod.rs`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Should `qr.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06057692307692308 - nodes in this community are weakly interconnected._
- **Why does `Storage` connect `Storage` to `pomodoro.rs`, `llm/mod.rs`, `spotify/mod.rs`, `todo.rs`, `ClipState`, `Sealer`, `manager`, `Inner`, `clipboard/mod.rs`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Should `ClipRow.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.06845238095238096 - nodes in this community are weakly interconnected._