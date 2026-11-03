import { lazy, Suspense } from 'preact/compat';
import type { ComponentType } from 'preact';
import { MODULES } from '../core/registry';

const cache = new Map<string, ComponentType<any>>();

function resolve(id: string): ComponentType<any> | null {
  const def = MODULES.find((m) => m.id === id);
  if (!def) return null;
  if (!cache.has(id)) cache.set(id, lazy(def.load));
  return cache.get(id)!;
}

// Satu modul aktif pada satu waktu; berpindah tab meng-unmount modul sebelumnya.
// `key` pada pembungkus memicu animasi masuk singkat setiap ganti tab.
export function ModuleHost({ id }: { id: string }) {
  const Active = resolve(id);
  return (
    <div class="host" role="tabpanel">
      <div key={id} class="host__view">
        {Active ? (
          <Suspense fallback={<div class="host__loading"><i class="skel" /><i class="skel skel--short" /></div>}>
            <Active />
          </Suspense>
        ) : (
          <div class="host__loading err">Modul tidak ditemukan.</div>
        )}
      </div>
    </div>
  );
}
