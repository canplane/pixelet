import { defineConfig } from "vitest/config";

export default defineConfig({
  // Where kit's spec/ is, which the golden reader takes as its root
  // (@xpute/golden): only kit's tests read golden records.
  define: {
    GOLDEN_ROOT: JSON.stringify(new URL("./kit/spec/", import.meta.url).href),
  },
  test: {
    include: [
      "kit/ts/src/**/*.test.ts",
      "link/ts/src/**/*.test.ts",
      "web/host/src/**/*.test.ts",
      "gpu/web/host/src/runtime.frame.test.ts",
      "gpu/web/host/src/web.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
