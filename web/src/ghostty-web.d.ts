/**
 * SPIKE (spike/ghostty-web): the ghostty-web package ships types for most of
 * its surface but not the xterm.js-compatible `Terminal` class. Augment the
 * module with a loose `any`-typed declaration — this spike duck-types
 * everything anyway.
 */
declare module "ghostty-web" {
  export function init(): Promise<void>;
  export class Terminal {
    constructor(options?: Record<string, unknown>);
    [key: string]: any;
  }
  export class FitAddon {
    fit(): void;
    [key: string]: any;
  }
}
