import { useCallback, useEffect, useRef, useState } from 'preact/hooks';

export type ToastTone = 'ok' | 'err';
export type ToastState = { id: number; msg: string; tone: ToastTone } | null;

/** Toast tunggal (yang baru menggantikan yang lama), hilang otomatis; timer dibersihkan saat unmount. */
export function useToast() {
  const [toast, setToast] = useState<ToastState>(null);
  const timer = useRef<number | undefined>(undefined);
  const seq = useRef(0);

  const show = useCallback((msg: string, tone: ToastTone = 'ok') => {
    window.clearTimeout(timer.current);
    setToast({ id: ++seq.current, msg, tone });
    timer.current = window.setTimeout(() => setToast(null), tone === 'err' ? 4200 : 1800);
  }, []);

  useEffect(() => () => window.clearTimeout(timer.current), []);
  return { toast, show };
}

export function Toast({ toast }: { toast: ToastState }) {
  if (!toast) return null;
  return (
    <div
      key={toast.id}
      class={`anat__toast anat__toast--${toast.tone}`}
      role={toast.tone === 'err' ? 'alert' : 'status'}
    >
      {toast.msg}
    </div>
  );
}
