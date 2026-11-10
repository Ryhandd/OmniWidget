import { useEffect, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import { THEMES, applyTheme } from '../../themes/theme';
import { AiSection } from './AiSection';
import './styles.css';

export default function SettingsModule() {
  const [theme, setTheme] = useState(document.documentElement.dataset.theme ?? 'default');
  const [lock, setLock] = useState(false);
  const [sleepMin, setSleepMin] = useState(5);
  const [sleepNote, setSleepNote] = useState('');
  const [autostart, setAutostart] = useState(false);
  const [islandBehavior, setIslandBehavior] = useState<'auto' | 'hover'>(() => { try { return localStorage.getItem('omniwidget.island.behavior') === 'auto' ? 'auto' : 'hover'; } catch { return 'hover'; } });

  // Nilai awal berasal dari Rust (sumber kebenaran), bukan dari state lokal.
  useEffect(() => {
    bridge.invoke<{ lockSnap: boolean; sleepAfterMin: number; islandBehavior: string }>('shell_get_state')
      .then((s) => { setLock(s.lockSnap); setSleepMin(s.sleepAfterMin); })
      .catch(() => {});
    bridge.invoke<boolean>('app_autostart_get').then(setAutostart).catch(() => {});
  }, []);

  const toggleAutostart = (v: boolean) => {
    setAutostart(v);
    bridge.invoke<boolean>('app_autostart_set', { enabled: v }).then(setAutostart).catch(() => setAutostart(!v));
  };

  const pickSleep = (m: number) => { setSleepMin(m); bridge.invoke('shell_set_sleep_after', { minutes: m }).catch(() => {}); };

  const pickIslandBehavior = (v: 'auto' | 'hover') => {
    setIslandBehavior(v);
    try { localStorage.setItem('omniwidget.island.behavior', v); } catch { /* abaikan */ }
    window.dispatchEvent(new CustomEvent('omni:island-behavior', { detail: v }));
    bridge.invoke('shell_set_island_behavior', { behavior: v === 'auto' ? 'island_auto_open' : 'island_hover_only' }).catch(() => {});
  };

  // Rust menolak tidur (false) selama konversi berjalan; WebView tetap hidup dan pengguna diberi tahu.
  const sleepNow = () => {
    setSleepNote('');
    bridge.invoke<boolean>('shell_deep_sleep')
      .then((ok) => { if (!ok) setSleepNote('Tidak bisa tidur: konversi masih berjalan. Coba lagi setelah selesai.'); })
      .catch(() => {});
  };

  const pickTheme = async (id: string) => { setTheme(id); await applyTheme(id); };
  const toggleLock = (v: boolean) => { setLock(v); bridge.invoke('window_set_lock_snap', { locked: v }).catch(() => {}); };

  return (
    <div class="set">
      <section>
        <h4>Tema</h4>
        <select class="field" value={theme} onChange={(e) => pickTheme((e.target as HTMLSelectElement).value as 'auto' | 'hover')}>
          {Object.entries(THEMES).map(([id, t]) => <option key={id} value={id}>{t.name}</option>)}
        </select>
      </section>

      <section>
        <h4>Startup</h4>
        <label class="set__row">
          <input type="checkbox" checked={autostart} onChange={(e) => toggleAutostart((e.target as HTMLInputElement).checked)} />
          <span>Jalankan OmniWidget saat laptop menyala</span>
        </label>
      </section>

      <section>
        <h4>Posisi</h4>
        <label class="set__row">
          <input type="checkbox" checked={lock} onChange={(e) => toggleLock((e.target as HTMLInputElement).checked)} />
          <span>Selalu kembali ke top-center</span>
        </label>
        <div class="set__note muted">Klik dua kali pada handle untuk reset posisi. Hotkey: Ctrl+Alt+Space.</div>
      </section>

      <section>
        <h4>Hemat memori</h4>
        <label class="set__row">
          <span class="grow">Deep Sleep otomatis saat pill tidak dipakai</span>
          <select class="field" style={{ width: 'auto' }} value={sleepMin} onChange={(e) => pickSleep(Number((e.target as HTMLSelectElement).value))}>
            <option value={0}>Mati</option>
            <option value={1}>1 menit</option>
            <option value={5}>5 menit</option>
            <option value={15}>15 menit</option>
            <option value={30}>30 menit</option>
          </select>
        </label>
        <div class="set__row" style={{ marginTop: 8 }}>
          <button class="btn btn--ghost" onClick={sleepNow}>Tidur sekarang</button>
        </div>
        {sleepNote && <div class="set__note err">{sleepNote}</div>}
        <div class="set__note muted">WebView dihancurkan; hanya tray dan hotkey yang aktif. Bangun lewat Ctrl+Alt+Space atau klik ikon tray.</div>
      </section>

      <section>
        <h4>Tampilan island saat ciut</h4>
        <label class="set__row">
          <span class="grow">Mode tampilan</span>
          <select class="field" style={{ width: 'auto' }} value={islandBehavior} onChange={(e) => pickIslandBehavior((e.target as HTMLSelectElement).value as 'auto' | 'hover')}>
            <option value="hover">Hanya jam, menu muncul saat hover (default)</option>
            <option value="auto">Semua menu selalu tampil</option>
          </select>
        </label>
        <div class="set__note muted">Pengaturan ini berlaku saat widget ciut. Saat lagu diputar, island tetap menampilkan jam + lagu.</div>
      </section>

      <section>
        <h4>AI (Anatomy Prompt)</h4>
        <AiSection />
      </section>

    </div>
  );
}
