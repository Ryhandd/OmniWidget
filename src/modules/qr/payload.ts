// Templat isi QR + klasifikasi hasil pindai (TDD 4.7). Logika murni -> teruji di test:web.
export type Template = 'text' | 'url' | 'wifi' | 'email' | 'tel' | 'vcard';
export const TEMPLATES: Array<{ id: Template; label: string }> = [
  { id: 'url', label: 'URL' }, { id: 'text', label: 'Teks' }, { id: 'wifi', label: 'Wi-Fi' },
  { id: 'email', label: 'Email' }, { id: 'tel', label: 'Telepon' }, { id: 'vcard', label: 'Kontak' },
];

export type Fields = Record<string, string>;

/** Escape spesifikasi WIFI: \ ; , : " */
export const wifiEscape = (s: string) => s.replace(/([\\;,:"])/g, '\\$1');

export function buildPayload(t: Template, f: Fields): string {
  const v = (k: string) => (f[k] ?? '').trim();
  switch (t) {
    case 'text': return f.text ?? '';
    case 'url': { const u = v('url'); return u && !/^[a-z][a-z0-9+.-]*:/i.test(u) ? `https://${u}` : u; }
    case 'wifi': {
      if (!v('ssid')) return '';
      const sec = f.security || 'WPA';
      const pass = sec === 'nopass' ? '' : `P:${wifiEscape(f.password ?? '')};`;
      return `WIFI:T:${sec};S:${wifiEscape(f.ssid.trim())};${pass}${f.hidden === '1' ? 'H:true;' : ''};`;
    }
    case 'email': {
      if (!v('to')) return '';
      const q = [v('subject') && `subject=${encodeURIComponent(v('subject'))}`, v('body') && `body=${encodeURIComponent(v('body'))}`].filter(Boolean).join('&');
      return `mailto:${v('to')}${q ? `?${q}` : ''}`;
    }
    case 'tel': return v('tel') ? `tel:${v('tel').replace(/[^\d+]/g, '')}` : '';
    case 'vcard': {
      if (!v('name')) return '';
      const esc = (s: string) => s.replace(/([\\;,])/g, '\\$1').replace(/\n/g, '\\n');
      return [
        'BEGIN:VCARD', 'VERSION:3.0', `FN:${esc(v('name'))}`,
        v('org') && `ORG:${esc(v('org'))}`, v('tel') && `TEL:${esc(v('tel'))}`, v('email') && `EMAIL:${esc(v('email'))}`,
        'END:VCARD',
      ].filter(Boolean).join('\n');
    }
  }
}

export type Scanned =
  | { kind: 'url'; text: string; host: string; safe: boolean }
  | { kind: 'wifi'; text: string; ssid: string; security: string; password: string }
  | { kind: 'email' | 'tel' | 'vcard' | 'text'; text: string };

function wifiField(s: string, key: string) {
  const m = s.match(new RegExp(`(?:^|;)${key}:((?:\\\\.|[^;])*)`));
  return m ? m[1].replace(/\\(.)/g, '$1') : '';
}

/** Klasifikasi isi. URL hanya dianggap bisa dibuka bila http/https (mencegah "quishing" skema lain). */
export function classify(text: string): Scanned {
  const t = text.trim();
  if (/^WIFI:/i.test(t)) {
    const body = t.slice(5);
    return { kind: 'wifi', text: t, ssid: wifiField(body, 'S'), security: wifiField(body, 'T') || 'nopass', password: wifiField(body, 'P') };
  }
  if (/^https?:\/\//i.test(t)) {
    try { const u = new URL(t); return { kind: 'url', text: t, host: u.hostname, safe: true }; } catch { /* lanjut */ }
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(t)) return { kind: 'url', text: t, host: t.split(':')[0] + ':', safe: false };
  if (/^mailto:/i.test(t)) return { kind: 'email', text: t };
  if (/^tel:/i.test(t)) return { kind: 'tel', text: t };
  if (/^BEGIN:VCARD/i.test(t)) return { kind: 'vcard', text: t };
  return { kind: 'text', text: t };
}

/** Rasio kontras WCAG antara dua #rrggbb. */
export function contrast(a: string, b: string): number {
  const lum = (h: string) => {
    const c = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
