import { defineConfig } from "vitest/config";
import { platform_file } from "./build/path.ts";

// The tests run the page's files: x.web.ts where an import names x.ts, as
// Metro takes them for the page (build/path.ts).
export default defineConfig({
  plugins: [
    {
      name: "pxl-platform-file",
      enforce: "pre",
      resolveId: (spec, from) =>
        from ? platform_file(from, spec, "web") : null,
    },
  ],
  test: {
    include: [
      "client/src/**/*.test.ts",
      "tool/shader/*.test.ts",
      "gpu/host/src/web.test.ts",
      "gpu/expo/src/**/*.test.ts",
    ],
    // One test has Tint compile every shader, which runs long, so tests have
    // no timeout.
    testTimeout: 0,
  },
});
