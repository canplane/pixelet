import { defineConfig } from "vitest/config";

export default defineConfig({
  // The aliases are tsconfig.json's.
  resolve: { tsconfigPaths: true },
  test: {
    include: ["client/src/**/*.test.ts", "app/web/src/**/*.test.ts"],
    // Tint compiles every generated shader in one test.
    testTimeout: 0,
  },
});
