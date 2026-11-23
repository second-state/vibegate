import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  // The vendored webm-writer files in src/lib/vendor/webmwriter are
  // CommonJS — include them in the CJS→ESM conversion (by default only
  // node_modules gets converted).
  build: {
    commonjsOptions: {
      include: [/node_modules/, /src\/lib\/vendor/],
    },
  },
});
