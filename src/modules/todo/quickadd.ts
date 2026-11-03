// Parser quick-add ringan (tanpa library NLP), ID + EN. Logika murni -> teruji di test:web.
//   "Kirim laporan besok jam 9 !"     -> besok 09:00, prioritas 1
//   "Minum obat tiap hari jam 7.30"    -> hari ini/besok 07:30, ulang harian
//   "Telpon dosen 20 menit lagi"       -> sekarang + 20 menit
export type Repeat = 'daily' | 'weekdays' | 'weekly' | 'monthly';
export type Parsed = { title: string; dueAt: number | null; repeat: Repeat | null; priority: number };

const DAYS: Record<string, number> = {
  minggu: 0, ahad: 0, sunday: 0, senin: 1, monday: 1, selasa: 2, tuesday: 2, rabu: 3, wednesday: 3,
  kamis: 4, thursday: 4, jumat: 5, "jum'at": 5, friday: 5, sabtu: 6, saturday: 6,
};

const REPEATS: Array<[RegExp, Repeat]> = [
  [/\b(?:tiap|setiap)\s+hari\s+kerja\b|\bweekdays\b/i, 'weekdays'],
  [/\b(?:tiap|setiap)\s+hari\b|\bdaily\b|\bevery\s*day\b/i, 'daily'],
  [/\b(?:tiap|setiap)\s+minggu\b|\bweekly\b|\bevery\s*week\b/i, 'weekly'],
  [/\b(?:tiap|setiap)\s+bulan\b|\bmonthly\b|\bevery\s*month\b/i, 'monthly'],
];

export function parseQuickAdd(input: string, now = new Date()): Parsed {
  let text = ` ${input.trim()} `;
  const cut = (re: RegExp) => { const m = text.match(re); if (m) text = text.replace(m[0], ' '); return m; };

  let priority = 0;
  const bang = cut(/\s(!{1,3})(?=\s)/);
  if (bang) priority = Math.min(2, bang[1].length);

  let repeat: Repeat | null = null;
  for (const [re, r] of REPEATS) if (cut(re)) { repeat = r; break; }

  // Relatif: "20 menit lagi", "in 2h", "2 jam lagi", "10m". ("Baca 30 menit" = durasi, bukan tenggat.)
  const rel = cut(/\s(?:in\s+(\d{1,3})\s*(minutes?|mins?|m|hours?|h)|(\d{1,3})\s*(menit|mnt|jam)\s+lagi|(\d{1,3})(m|h))(?=\s)/i);
  if (rel) {
    const n = Number(rel[1] ?? rel[3] ?? rel[5]);
    const mins = /^(jam|h|hours?)$/i.test(rel[2] ?? rel[4] ?? rel[6]) ? n * 60 : n;
    return { title: tidy(text), dueAt: now.getTime() + mins * 60_000, repeat, priority };
  }

  // Tanggal.
  let day: Date | null = null;
  const base = new Date(now); base.setHours(0, 0, 0, 0);
  const plus = (d: number) => { const x = new Date(base); x.setDate(x.getDate() + d); return x; };
  if (cut(/\s(?:lusa|day after tomorrow)(?=\s)/i)) day = plus(2);
  else if (cut(/\s(?:besok|tomorrow|tmr)(?=\s)/i)) day = plus(1);
  else if (cut(/\s(?:hari ini|today|nanti)(?=\s)/i)) day = plus(0);
  else {
    const m = cut(new RegExp(`\\s(?:hari\\s+|on\\s+|next\\s+)?(${Object.keys(DAYS).join('|')})(?=\\s)`, 'i'));
    if (m) {
      const want = DAYS[m[1].toLowerCase()];
      const diff = (want - base.getDay() + 7) % 7 || 7; // hari yang sama = minggu depan
      day = plus(diff);
    } else {
      const d = cut(/\s(\d{1,2})[/-](\d{1,2})(?:[/-](\d{2,4}))?(?=\s)/); // 25/12 atau 25-12-2026
      if (d) {
        const y = d[3] ? (d[3].length === 2 ? 2000 + Number(d[3]) : Number(d[3])) : base.getFullYear();
        day = new Date(y, Number(d[2]) - 1, Number(d[1]));
        if (!d[3] && day < base) day.setFullYear(y + 1);
      }
    }
  }

  // Jam: "jam 9", "pukul 14.30", "at 9pm", "21:00", "9am".
  let h: number | null = null, min = 0;
  const t =
    cut(/\s(?:jam|pukul|pkl\.?|at)\s+(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm|pagi|siang|sore|malam)?(?=\s)/i) ??
    cut(/\s(\d{1,2})[:.](\d{2})\s*(am|pm)?(?=\s)/i) ??
    cut(/\s(\d{1,2})\s*(am|pm)(?=\s)/i);
  if (t) {
    h = Number(t[1]);
    const second = t[2] && /^\d+$/.test(t[2]) ? t[2] : undefined;
    const mer = (second ? t[3] : t[3] ?? t[2])?.toLowerCase();
    min = second ? Number(second) : 0;
    if ((mer === 'pm' || mer === 'sore' || mer === 'malam') && h < 12) h += 12;
    if ((mer === 'am') && h === 12) h = 0;
    if (mer === 'siang' && h < 11) h += 12;
    if (h > 23 || min > 59) h = null;
  }

  let dueAt: number | null = null;
  if (day || h !== null) {
    const d = new Date(day ?? base);
    d.setHours(h ?? 9, min, 0, 0); // tanggal tanpa jam -> 09:00
    if (!day && d.getTime() <= now.getTime()) d.setDate(d.getDate() + 1); // jam sudah lewat -> besok
    dueAt = d.getTime();
  } else if (repeat) {
    const d = new Date(now); d.setHours(9, 0, 0, 0);
    if (d.getTime() <= now.getTime()) d.setDate(d.getDate() + 1);
    dueAt = d.getTime();
  }
  return { title: tidy(text) || input.trim(), dueAt, repeat, priority };
}

const tidy = (s: string) => s.replace(/\s+/g, ' ').trim();

/** Kemunculan berikutnya (zona waktu lokal). */
export function nextOccurrence(ms: number, repeat: Repeat): number {
  const d = new Date(ms);
  switch (repeat) {
    case 'daily': d.setDate(d.getDate() + 1); break;
    case 'weekly': d.setDate(d.getDate() + 7); break;
    case 'weekdays':
      do d.setDate(d.getDate() + 1); while (d.getDay() === 0 || d.getDay() === 6);
      break;
    case 'monthly': {
      const want = d.getDate();
      d.setDate(1);
      d.setMonth(d.getMonth() + 1);
      const last = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();
      d.setDate(Math.min(want, last));
      break;
    }
  }
  return d.getTime();
}

/** Lompati kemunculan yang sudah lewat (mis. tugas harian yang telat 3 hari). */
export function nextFuture(ms: number, repeat: Repeat, now = Date.now()): number {
  let t = nextOccurrence(ms, repeat);
  for (let i = 0; t <= now && i < 1000; i++) t = nextOccurrence(t, repeat);
  return t;
}

export const REPEAT_LABEL: Record<Repeat, string> = { daily: 'Harian', weekdays: 'Hari kerja', weekly: 'Mingguan', monthly: 'Bulanan' };

export function whenLabel(ms: number, now = new Date()): string {
  const d = new Date(ms);
  const day0 = new Date(now); day0.setHours(0, 0, 0, 0);
  const diff = Math.round((new Date(d).setHours(0, 0, 0, 0) - day0.getTime()) / 86_400_000);
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (diff === 0) return `Hari ini ${time}`;
  if (diff === 1) return `Besok ${time}`;
  if (diff === -1) return `Kemarin ${time}`;
  if (diff > 1 && diff < 7) return `${d.toLocaleDateString([], { weekday: 'long' })} ${time}`;
  return `${d.toLocaleDateString([], { day: 'numeric', month: 'short' })} ${time}`;
}
