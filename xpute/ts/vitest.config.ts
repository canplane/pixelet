import { defineConfig } from "vitest/config";

export default defineConfig({
  // Where the tree's spec/ is, which the golden reader takes as its root
  // (dev/golden.ts).
  define: {
    GOLDEN_ROOT: JSON.stringify(new URL("../spec/", import.meta.url).href),
  },
  test: {
    include: [
      "kit/src/**/*.test.ts",
      "runtime/src/**/*.test.ts",
      "gpu/src/runtime.frame.test.ts",
      "gpu/src/web.test.ts",
      "dev/example/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
