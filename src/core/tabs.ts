// Pilih tab yang terlihat; sisanya masuk menu "More". Tab aktif selalu terlihat.
export const MAX_VISIBLE_TABS = 6;

export function splitTabs<T extends { id: string }>(
  all: readonly T[],
  activeId: string,
  max = MAX_VISIBLE_TABS,
): { visible: T[]; overflow: T[] } {
  if (all.length <= max) return { visible: [...all], overflow: [] };
  let visible = all.slice(0, max);
  const active = all.find((t) => t.id === activeId);
  if (active && !visible.includes(active)) visible = [...visible.slice(0, max - 1), active];
  return { visible, overflow: all.filter((t) => !visible.includes(t)) };
}
