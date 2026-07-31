/** Minimal hash router. Every view has a URL (ISC-10): '#/' Overview,
 * '#/spike' renderer spike. Back/forward and reload work because the hash IS
 * the state — no router library needed at this route count. */

export function currentRoute(): string {
  const hash = window.location.hash.replace(/^#/, '');
  return hash === '' ? '/' : hash;
}

export function navigate(route: string): void {
  window.location.hash = route;
}

export function onRouteChange(handler: (route: string) => void): () => void {
  const listener = () => handler(currentRoute());
  window.addEventListener('hashchange', listener);
  return () => window.removeEventListener('hashchange', listener);
}
