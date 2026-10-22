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

/**
 * Handle pointerdown with a NATIVE listener (stopping propagation), so the
 * canvas gesture layer never sees touches over this element. Use this for
 * every pointerdown handler inside the canvas — Svelte 5's delegated
 * `on:pointerdown|stopPropagation` cannot stop the native bubble, and
 * use-gesture's preventDefault then suppresses synthesized mouse events and
 * focus (mobile soft keyboard never opens).
 */
export function nativePointerDown(
  node: HTMLElement,
  handler: (e: PointerEvent) => void,
) {
  const h = (e: PointerEvent) => {
    e.stopPropagation();
    handler(e);
  };
  node.addEventListener("pointerdown", h);
  return {
    destroy() {
      node.removeEventListener("pointerdown", h);
    },
  };
}
