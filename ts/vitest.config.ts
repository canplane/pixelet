import { defineConfig } from "vitest/config";
import { platformFile } from "./build/path.ts";

// The tests run the web files: x.web.ts where an import names x.ts, as
// Metro takes them for the web (build/path.ts).
export default defineConfig({
  plugins: [
    {
      name: "pxl-platform-file",
      enforce: "pre",
      resolveId: (spec, from) =>
        from ? platformFile(from, spec, "web") : null,
    },
  ],
  test: {
    include: [
      "client/src/**/*.test.ts",
      "tool/shader/*.test.ts",
      "gpu/host/src/web.test.ts",
      "gpu/host/src/vsync.test.ts",
      "gpu/expo/src/**/*.test.ts",
    ],
    // One test has Tint compile every shader, which runs long, so tests have
    // no timeout.
    testTimeout: 0,
  },
});
