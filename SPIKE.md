# SPIKE: ghostty-web as the terminal frontend (replacing xterm.js)

Branch: `spike/ghostty-web` · Base: `dev/0.2.x` (26f04af) · Date: 2026-11-27

## Why

Kitty graphics don't render in xterm.js: `@xterm/addon-image@0.9.0` (stable)
contains no kitty code at all — kitty support lives in the 0.10 beta line,
which requires the xterm.js 6.x beta across the whole addon stack. libghostty
(Ghostty's core) supports kitty graphics natively, so swapping the frontend to
`ghostty-web` (Coder's libghostty-WASM build with an xterm.js-compatible API)
looked like a way to get kitty images for free.

## What the spike did

- Swapped `session.ts` to `ghostty-web@0.4.0` (`init()` + compat `Terminal` +
  its own `FitAddon`); dropped the webgl/unicode-graphemes/image addons.
- Term creation moved into `attach()` (the WASM `init()` is async; the old
  sync constructor can't await it).
- `ghostty-web.d.ts` shim (the package doesn't type-declare the compat
  Terminal); `term`/`fitAddon` became `any`.
- `ghostty-vt.wasm` copied to `web/public/` (the loader tries
  `<wasm-dir>/../ghostty-vt.wasm`, `./ghostty-vt.wasm`, `/ghostty-vt.wasm` —
  the last one covers non-bundled paths).
- Right-click cell tracking falls back from `.xterm-screen` to the container
  element (same geometry, canvas fills it).

## Results

Works without further changes:

- Full text/VT rendering on the canvas renderer: colors, powerline glyphs,
  cursor, fit/resize (verified headless against the hc backend, 78×27).
- xterm.js API compatibility held up for everything we use: onData/onResize,
  write(Uint8Array), getSelection, loadAddon, options mutation (live theme),
  dispose, cols/rows getters, hidden textarea focus.
- `term.buffer.active.getLine().getCell()` has the same shape termshot needs
  (untested end-to-end, but no obvious blocker).

Does NOT work:

- **Kitty graphics: still not rendered — the spike's original goal fails.**
  ghostty-web has zero kitty-graphics code: the WASM API surface exposes only
  cells/text (no image placements) and the renderer draws nothing but cells.
  The only "kitty" in the repo is the keyboard protocol (KittyKeyFlags).
  Verified two ways: a real `hc attach` forwards the bytes (red square shows
  in Ghostty), while the same emission into ghostty-web and into
  xterm.js+addon-image both render nothing.

- Untested / unknown: mobile IME + soft-keyboard edge cases, selection
  behavior differences, termshot end-to-end, performance vs the WebGL
  renderer under heavy output.

## Verdict

ghostty-web is a surprisingly clean drop-in for the text surface, but it does
not buy kitty graphics today — neither stack renders kitty images in the
browser:

| stack | kitty graphics | IIP/SIXEL | notes |
| --- | --- | --- | --- |
| xterm.js 5.5 + addon-image 0.9.0 (current main) | no (0.10 beta + xterm 6 required) | yes (verified) | stable, known |
| ghostty-web 0.4 (this spike) | no (no implementation at all) | untested | new, fast-moving |

Revisit triggers:

1. xterm.js 6 stable + addon-image 0.10 stable → upgrade for kitty.
2. libghostty WASM exposing image placements + a ghostty-web renderer that
   draws them → re-run this spike; the session.ts swap is already here.
3. If kitty images become a hard requirement before either lands, the only
   lever is asking herdr-core to offer a "graphics pass-through" attach mode
   plus a custom frontend renderer — significant work on both sides.

The session.ts diff on this branch is kept as the reference for that day.
