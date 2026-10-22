/**
 * Stop propagation of the given events with NATIVE listeners on the element.
 *
 * Needed because Svelte 5 delegates events like pointerdown/click from the app
 * root: a delegated `on:pointerdown|stopPropagation` only stops Svelte's
 * re-dispatch, while the native event still bubbles up to the canvas gesture
 * layer (use-gesture), whose preventDefault then suppresses the click event.
 * A real listener on the element cuts the native bubble chain for real.
 */
export function stopNative(node: HTMLElement, events: string[] = ["pointerdown", "dblclick"]) {
  const handlers = new Map<string, (e: Event) => void>();
  for (const name of events) {
    const h = (e: Event) => e.stopPropagation();
    handlers.set(name, h);
    node.addEventListener(name, h);
  }
  return {
    destroy() {
      for (const [name, h] of handlers) {
        node.removeEventListener(name, h);
      }
    },
  };
}
