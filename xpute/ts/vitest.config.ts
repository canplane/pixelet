import { defineConfig } from "vitest/config";

export default defineConfig({
  // The aliases are tsconfig.json's.
  resolve: { tsconfigPaths: true },
  test: {
    include: [
      "kit/src/**/*.test.ts",
      "host/src/**/*.test.ts",
      "example/**/*.test.ts",
    ],
    testTimeout: 0,
    setupFiles: ["vitest.setup.ts"],
  },
});
