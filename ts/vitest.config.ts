import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: { tsconfigPaths: true },
  test: {
    include: ["client/src/**/*.test.ts", "tool/shader/*.test.ts"],
    // One test has Tint compile every shader, which runs long, so tests have
    // no timeout.
    testTimeout: 0,
  },
});
