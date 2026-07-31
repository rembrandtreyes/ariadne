/** Number/date formatting helpers — every displayed number gets a label or
 * unit at the call site (ISC-12); these keep the values themselves readable. */

export function formatCount(n: number): string {
  return new Intl.NumberFormat('en-US').format(n);
}

export function formatPercent(ratio: number): string {
  return `${Math.round(ratio * 100)}%`;
}

export function formatTimestamp(unixSeconds: number | null): string {
  if (!unixSeconds) return 'unknown';
  return new Date(unixSeconds * 1000).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function formatIsoDate(iso: string | null): string {
  if (!iso) return 'never';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}
