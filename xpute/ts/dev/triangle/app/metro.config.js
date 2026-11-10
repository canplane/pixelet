// xpute-triangle-app/metro.config.js

// xpute's packages ship as TypeScript and import each file by its full name, so
// a native bundle swaps `x.ts` for `x.native.ts` here, where Metro's own
// platform lookup does not. `x.native.ts` still reaches its default by `./x.ts`.

const fs = require("node:fs");
const path = require("node:path");
const { getDefaultConfig } = require("expo/metro-config");

const base = getDefaultConfig(__dirname);

module.exports = {
  ...base,
  // The workspace this app sits in, where pnpm keeps what it links here.
  watchFolders: [path.resolve(__dirname, "../../..")],
  resolver: {
    ...base.resolver,
    resolveRequest(ctx, name, platform) {
      const r = ctx.resolveRequest(ctx, name, platform);
      if (r.type !== "sourceFile") return r;
      const ext = path.extname(r.filePath);
      const stem = r.filePath.slice(0, -ext.length);
      const own = `${stem}.native${ext}`;
      return !ctx.originModulePath.startsWith(`${stem}.`) && fs.existsSync(own)
        ? { type: "sourceFile", filePath: own }
        : r;
    },
  },
};
