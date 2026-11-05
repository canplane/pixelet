import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "kit/src/**/*.test.ts",
      "host/src/**/*.test.ts",
      "gpu/src/host.frames.test.ts",
      "example/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
