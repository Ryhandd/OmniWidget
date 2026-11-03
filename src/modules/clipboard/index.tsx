import { useRef, useState } from 'preact/hooks';
import { ClipRow } from './components/ClipRow';
import { SettingsPanel } from './components/SettingsPanel';
import { Toolbar } from './components/Toolbar';
import { group, usage } from './state';
import { useClipboard, useMinuteClock } from './useClipboard';
import './styles.css';

export default function ClipboardModule() {
  const { state: s, notice, thumbs, actions } = useClipboard();
  const now = useMinuteClock();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const list = useRef<HTMLDivElement>(null);

  const g = group(s.items, s.query);
  const shown = g.pinned.length + g.recent.length;

  // Navigasi roving: panah atas/bawah memindahkan fokus antar baris; dari kolom cari, panah bawah ke baris pertama.
  const onListKey = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const rows = [...(list.current?.querySelectorAll<HTMLButtonElement>('[data-row]') ?? [])];
    const i = rows.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    const next = rows[i + (e.key === 'ArrowDown' ? 1 : -1)];
    if (next) { next.focus(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { list.current?.parentElement?.querySelector<HTMLInputElement>('.clip__search')?.focus(); e.preventDefault(); }
  };
  const onSearchKey = (e: KeyboardEvent) => {
    // Hanya dari kolom cari; panah dari baris sudah ditangani onListKey (event ini ikut naik ke sini).
    if (e.key === 'ArrowDown' && (e.target as HTMLElement).classList.contains('clip__search')) {
      const first = list.current?.querySelector<HTMLButtonElement>('[data-row]');
      if (first) { first.focus(); e.preventDefault(); }
    }
  };

  const renderGroup = (title: string, items: typeof g.pinned) =>
    items.length > 0 && (
      <>
        <h4 class="clip__group">{title}</h4>
        <ul class="clip__list">
          {items.map((it) => (
            <ClipRow
              key={it.id}
              item={it}
              now={now}
              thumbs={thumbs}
              onPaste={actions.paste}
              onPin={actions.pin}
              onRemove={actions.remove}
              loadText={actions.loadText}
            />
          ))}
        </ul>
      </>
    );

  return (
    <div class="clip" onKeyDown={onSearchKey}>
      <Toolbar
        query={s.query}
        settings={s.settings}
        count={s.items.length}
        usage={usage(s.items, s.capacity)}
        settingsOpen={settingsOpen}
        onQuery={actions.setQuery}
        onTogglePause={() => actions.setSettings({ paused: !s.settings.paused })}
        onToggleSettings={() => setSettingsOpen((v) => !v)}
        onClear={actions.clear}
      />

      {settingsOpen && <SettingsPanel settings={s.settings} onChange={actions.setSettings} />}

      {s.settings.paused && (
        <div class="clip__banner" role="status">Perekaman dijeda: salinan baru tidak masuk riwayat.</div>
      )}

      {s.error && (
        <div class="clip__notice clip__notice--err" role="alert">
          <span>{s.error}</span>
          <button type="button" class="clip__link" onClick={actions.dismissError}>Tutup</button>
        </div>
      )}

      <div ref={list} class="clip__scroll" onKeyDown={onListKey}>
        {s.phase === 'loading' ? (
          <p class="muted clip__empty" role="status">Memuat…</p>
        ) : s.items.length === 0 ? (
          <div class="clip__empty">
            <h4>Riwayat kosong</h4>
            <p class="muted">Salin teks atau gambar di aplikasi mana pun; yang terbaru muncul di sini (hingga {s.capacity} item).</p>
          </div>
        ) : shown === 0 ? (
          <p class="muted clip__empty">Tidak ada yang cocok dengan “{s.query.trim()}”.</p>
        ) : (
          <>
            {renderGroup('Disematkan', g.pinned)}
            {renderGroup(g.pinned.length ? 'Terbaru' : '', g.recent)}
          </>
        )}
      </div>

      <div class="clip__toast" aria-live="polite">
        {notice && <span key={notice.id} class={`clip__toast-msg clip__toast-msg--${notice.tone}`} role={notice.tone === 'err' ? 'alert' : 'status'}>{notice.msg}</span>}
      </div>
    </div>
  );
}
