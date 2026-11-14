import { defineConfig } from "vitest/config";

export default defineConfig({
  // xpute's root, which the golden reader reads each part's records under
  // (@xpute/golden), as the C++ tests do.
  define: {
    GOLDEN_ROOT: JSON.stringify(new URL("./", import.meta.url).href),
  },
  test: {
    include: [
      "kit/ts/src/**/*.test.ts",
      "link/ts/src/**/*.test.ts",
      "web/host/src/**/*.test.ts",
      "gpu/web/host/src/web.test.ts",
      "gpu/native/runtime/expo/src/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
