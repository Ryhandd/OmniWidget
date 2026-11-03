import { useCallback, useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { listen } from '@tauri-apps/api/event';
import { bridge } from '../../core/bridge';
import { nextFuture, parseQuickAdd, REPEAT_LABEL, whenLabel, type Repeat } from './quickadd';
import './styles.css';

type Task = {
  id: number; title: string; note: string; done: boolean; priority: number;
  dueAt: number | null; remindAt: number | null; reminded: boolean; repeat: Repeat | null;
  createdAt: number; completedAt: number | null;
};
type Input = { title: string; note: string; priority: number; dueAt: number | null; remindAt: number | null; repeat: Repeat | null };

const api = {
  list: () => bridge.invoke<Task[]>('todo_list'),
  add: (task: Input) => bridge.invoke<Task>('todo_add', { task }),
  update: (id: number, task: Input) => bridge.invoke<Task>('todo_update', { id, task }),
  done: (id: number, done: boolean) => bridge.invoke<Task>('todo_set_done', { id, done }),
  snooze: (id: number, minutes: number) => bridge.invoke<Task>('todo_snooze', { id, minutes }),
  remove: (id: number) => bridge.invoke('todo_delete', { id }),
  clearDone: () => bridge.invoke<number>('todo_clear_done'),
};

const toInput = (t: Task): Input => ({ title: t.title, note: t.note, priority: t.priority, dueAt: t.dueAt, remindAt: t.remindAt, repeat: t.repeat });
const toLocalInput = (ms: number | null) => {
  if (!ms) return '';
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
};

function group(tasks: Task[], now: number) {
  const end = new Date(now); end.setHours(23, 59, 59, 999);
  const g = { late: [] as Task[], today: [] as Task[], soon: [] as Task[], none: [] as Task[], done: [] as Task[] };
  for (const t of tasks) {
    if (t.done) g.done.push(t);
    else if (t.dueAt == null) g.none.push(t);
    else if (t.dueAt < now) g.late.push(t);
    else if (t.dueAt <= end.getTime()) g.today.push(t);
    else g.soon.push(t);
  }
  return g;
}

export default function TodoModule() {
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const [text, setText] = useState('');
  const [edit, setEdit] = useState<number | null>(null);
  const [showDone, setShowDone] = useState(false);
  const [err, setErr] = useState('');
  const [now, setNow] = useState(Date.now());

  const load = useCallback(() => api.list().then(setTasks).catch((e) => { setTasks([]); setErr(String(e)); }), []);
  useEffect(() => {
    void load();
    let off: (() => void) | undefined;
    listen('todo://changed', () => void load()).then((u) => (off = u)).catch(() => {});
    const t = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => { off?.(); clearInterval(t); };
  }, [load]);

  const preview = useMemo(() => (text.trim() ? parseQuickAdd(text) : null), [text]);

  const add = async () => {
    if (!preview?.title) return;
    setErr('');
    try {
      await api.add({ title: preview.title, note: '', priority: preview.priority, dueAt: preview.dueAt, remindAt: preview.dueAt, repeat: preview.repeat });
      setText('');
      void load();
    } catch (e) { setErr(String(e)); }
  };

  const toggle = async (t: Task) => {
    try {
      // Tugas berulang tidak "selesai": dijadwalkan ke kemunculan berikutnya.
      if (!t.done && t.repeat && t.dueAt) {
        const next = nextFuture(t.dueAt, t.repeat);
        const shift = next - t.dueAt;
        await api.update(t.id, { ...toInput(t), dueAt: next, remindAt: t.remindAt ? t.remindAt + shift : null });
      } else {
        await api.done(t.id, !t.done);
      }
      void load();
    } catch (e) { setErr(String(e)); }
  };

  const g = useMemo(() => group(tasks ?? [], now), [tasks, now]);
  const open = (tasks ?? []).filter((t) => !t.done).length;

  const row = (t: Task) => (
    <TaskRow
      key={t.id}
      task={t}
      now={now}
      editing={edit === t.id}
      onEdit={() => setEdit(edit === t.id ? null : t.id)}
      onToggle={() => void toggle(t)}
      onSnooze={(m) => api.snooze(t.id, m).then(load).catch((e) => setErr(String(e)))}
      onDelete={() => api.remove(t.id).then(load).catch((e) => setErr(String(e)))}
      onSave={(inp) => api.update(t.id, inp).then(() => { setEdit(null); void load(); }).catch((e) => setErr(String(e)))}
    />
  );

  const section = (title: string, list: Task[], tone = '') =>
    list.length > 0 && (
      <section class="todo__sec">
        <h5 class={tone}>{title} <span>{list.length}</span></h5>
        {list.map(row)}
      </section>
    );

  return (
    <div class="todo">
      <form class="todo__add" onSubmit={(e) => { e.preventDefault(); void add(); }}>
        <input
          class="field"
          value={text}
          placeholder="Tambah tugas… mis. “Kirim laporan besok jam 9 !”"
          onInput={(e) => setText((e.target as HTMLInputElement).value)}
          onKeyDown={(e) => { if (e.key === 'Escape' && text) { e.stopPropagation(); setText(''); } }}
        />
        <button class="btn" type="submit" disabled={!preview?.title}>Tambah</button>
      </form>
      {preview && (preview.dueAt || preview.repeat || preview.priority > 0) && (
        <div class="todo__hint">
          {preview.dueAt && <span>🔔 {whenLabel(preview.dueAt)}</span>}
          {preview.repeat && <span>🔁 {REPEAT_LABEL[preview.repeat]}</span>}
          {preview.priority > 0 && <span>{'!'.repeat(preview.priority)} Prioritas</span>}
        </div>
      )}
      {err && <div class="todo__err err">{err}</div>}

      <div class="todo__list">
        {tasks === null ? (
          <div class="muted todo__empty">Memuat…</div>
        ) : open === 0 && g.done.length === 0 ? (
          <div class="todo__empty">
            <div class="todo__big">☑</div>
            <div>Belum ada tugas.</div>
            <div class="muted">Ketik di atas: “besok”, “lusa”, “senin”, “jam 14.30”, “20 menit lagi”, “tiap hari”, “!”.</div>
          </div>
        ) : (
          <>
            {section('Terlambat', g.late, 'err')}
            {section('Hari ini', g.today)}
            {section('Mendatang', g.soon)}
            {section('Tanpa tanggal', g.none)}
            {g.done.length > 0 && (
              <section class="todo__sec">
                <h5>
                  <button type="button" class="todo__link" onClick={() => setShowDone((v) => !v)}>
                    {showDone ? '▾' : '▸'} Selesai <span>{g.done.length}</span>
                  </button>
                  {showDone && <button type="button" class="todo__link todo__clear" onClick={() => api.clearDone().then(load)}>Hapus semua</button>}
                </h5>
                {showDone && g.done.map(row)}
              </section>
            )}
          </>
        )}
      </div>
    </div>
  );
}

type RowProps = {
  task: Task; now: number; editing: boolean;
  onEdit: () => void; onToggle: () => void; onSnooze: (m: number) => void; onDelete: () => void; onSave: (t: Input) => void;
};

function TaskRow({ task: t, now, editing, onEdit, onToggle, onSnooze, onDelete, onSave }: RowProps) {
  const ringing = t.reminded && !t.done;
  const late = !t.done && t.dueAt != null && t.dueAt < now;
  return (
    <div class={`todo__item${t.done ? ' is-done' : ''}${ringing ? ' is-ringing' : ''}`} data-prio={t.priority}>
      <div class="todo__main">
        <button type="button" class="todo__check" aria-label={t.done ? 'Tandai belum selesai' : 'Tandai selesai'} aria-pressed={t.done} onClick={onToggle}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.5l2.5 2.5L12 5.5" /></svg>
        </button>
        <button type="button" class="todo__body" onClick={onEdit}>
          <span class="todo__title">{t.title}</span>
          {(t.dueAt || t.repeat || t.note) && (
            <span class="todo__meta">
              {t.dueAt && <span class={late ? 'err' : ''}>{t.remindAt ? '🔔' : '◷'} {whenLabel(t.dueAt, new Date(now))}</span>}
              {t.repeat && <span>🔁 {REPEAT_LABEL[t.repeat]}</span>}
              {t.note && <span>✎ catatan</span>}
            </span>
          )}
        </button>
        {ringing && (
          <span class="todo__snooze">
            <button type="button" onClick={() => onSnooze(10)} title="Tunda 10 menit">+10m</button>
            <button type="button" onClick={() => onSnooze(60)} title="Tunda 1 jam">+1j</button>
          </span>
        )}
      </div>
      {editing && <Editor task={t} onSave={onSave} onDelete={onDelete} onCancel={onEdit} />}
    </div>
  );
}

function Editor({ task, onSave, onDelete, onCancel }: { task: Task; onSave: (t: Input) => void; onDelete: () => void; onCancel: () => void }) {
  const [f, setF] = useState<Input>(toInput(task));
  const [remind, setRemind] = useState(task.remindAt != null);
  const [sure, setSure] = useState(false);
  const title = useRef<HTMLInputElement>(null);
  useEffect(() => title.current?.focus(), []);
  const set = <K extends keyof Input>(k: K, v: Input[K]) => setF((x) => ({ ...x, [k]: v }));
  const save = () => onSave({ ...f, remindAt: remind && f.dueAt ? (f.remindAt && task.dueAt === f.dueAt ? f.remindAt : f.dueAt) : null });

  return (
    <form class="todo__edit" onSubmit={(e) => { e.preventDefault(); save(); }}
      onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onCancel(); } }}>
      <input ref={title} class="field" value={f.title} onInput={(e) => set('title', (e.target as HTMLInputElement).value)} />
      <div class="todo__grid">
        <label>Tenggat
          <input class="field" type="datetime-local" value={toLocalInput(f.dueAt)}
            onInput={(e) => { const v = (e.target as HTMLInputElement).value; set('dueAt', v ? new Date(v).getTime() : null); }} />
        </label>
        <label>Ulangi
          <select class="field" value={f.repeat ?? ''} onChange={(e) => set('repeat', ((e.target as HTMLSelectElement).value || null) as Repeat | null)}>
            <option value="">Tidak</option>
            {Object.entries(REPEAT_LABEL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
          </select>
        </label>
        <label>Prioritas
          <select class="field" value={f.priority} onChange={(e) => set('priority', Number((e.target as HTMLSelectElement).value))}>
            <option value={0}>Normal</option><option value={1}>Penting</option><option value={2}>Mendesak</option>
          </select>
        </label>
        <label class="todo__toggle">
          <input type="checkbox" checked={remind} disabled={!f.dueAt} onChange={(e) => setRemind((e.target as HTMLInputElement).checked)} />
          Ingatkan saat tenggat
        </label>
      </div>
      <textarea class="field" rows={2} placeholder="Catatan" value={f.note} onInput={(e) => set('note', (e.target as HTMLTextAreaElement).value)} />
      <div class="todo__actions">
        {sure ? (
          <button type="button" class="btn todo__danger" onClick={onDelete}>Yakin hapus</button>
        ) : (
          <button type="button" class="btn btn--ghost" onClick={() => setSure(true)}>Hapus</button>
        )}
        <span class="grow" />
        <button type="button" class="btn btn--ghost" onClick={onCancel}>Batal</button>
        <button type="submit" class="btn" disabled={!f.title.trim()}>Simpan</button>
      </div>
    </form>
  );
}
