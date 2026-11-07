import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "kit/src/**/*.test.ts",
      "runtime/src/**/*.test.ts",
      "gpu/src/runtime.frames.test.ts",
      "gpu/src/web.test.ts",
      "example/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
