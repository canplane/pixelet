import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "kit/src/**/*.test.ts",
      "runtime/src/**/*.test.ts",
      "gpu/src/runtime.frame.test.ts",
      "gpu/src/web.test.ts",
      "example/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
