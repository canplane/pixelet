// @pxl/native/babel.config.js

/**
 * Expo's preset, and the build's constants written into the code as values
 * — what vite.config.ts's `define` did — so a branch on one is folded where
 * it is compiled and what only a development build uses is not in a
 * production one at all. They are values and not globals for that reason:
 * `__IS_DEV__ ? EVERY : SHIPPED` read at run time carries both.
 */

const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");

/**
 * Every member of every `export const enum` under the shell and the xpute
 * packages, by enum and member: a const enum is a compile-time name for a
 * number, and a bundle should hold the number alone. A transpiler that sees
 * one file at a time cannot inline a name declared in another, and Babel's
 * TypeScript transform emits the enum as an object instead — every command's
 * and errno's name as a string, reverse mapping and all. So the declarations
 * are read up front, as vite.config.ts's const_enum did, and `inline_enums`
 * below does the rest.
 *
 * It refuses rather than guesses: a member whose value is not a plain
 * literal or arithmetic over literals leaves its whole enum alone, and is
 * named in `refused` so that leaving it is not taken for a table gone stale.
 */
function read_enums() {
  const DECL = /export\s+const\s+enum\s+(\w+)\s*\{([^}]*)\}/g;
  const VALUE = /^(?:0x[0-9a-fA-F]+|\d+|"[^"\\]*"|'[^'\\]*'|[-+*/|&^~()<>\s])+$/;
  const enums = new Map();
  const refused = new Set();
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (e.name !== "node_modules") walk(full);
        continue;
      }
      if (!/\.tsx?$/.test(e.name) || e.name.includes(".test.")) continue;
      for (const [, name, body] of fs.readFileSync(full, "utf8").matchAll(DECL)) {
        const members = new Map();
        let whole = true;
        for (let line of body.replace(/\/\*[\s\S]*?\*\//g, "").split("\n")) {
          line = line.replace(/\/\/.*/, "").trim().replace(/,$/, "");
          if (!line) continue;
          const at = line.indexOf("=");
          const key = at < 0 ? "" : line.slice(0, at).trim();
          const value = at < 0 ? "" : line.slice(at + 1).trim();
          if (!/^\w+$/.test(key) || !VALUE.test(value)) {
            whole = false;
            break;
          }
          members.set(key, /^["']/.test(value) ? value.slice(1, -1) : Function(`return (${value})`)());
        }
        if (whole) enums.set(name, members);
        else refused.add(name);
      }
    }
  };
  walk(path.join(root, "apps/web/src"));
  walk(path.join(root, "packages"));
  return { enums, refused };
}
const { enums: ENUMS, refused: REFUSED } = read_enums();

/** The table is read once, when Metro loads this file, so an enum or a member
 * added since — a spec regenerated under a running Metro — is not in it. Left
 * alone, it bundles as a name nothing binds and throws on the device; said
 * here, it stops the build with what to do. */
const stale = (what) => `${what} is not in the const enum table Metro read when it started; the enums changed since, so restart Metro with --clear`;

/** Replaces each `Enum.Member` with its value and strikes the declarations,
 * so the import that named the enum is left with no use and goes with the
 * TypeScript transform's other type-only imports. A use of an enum as a
 * value that is left — `Command[x]`, the enum handed on whole — is a build
 * error, never a bundle that reaches a device and throws. */
const inline_enums = ({ types: t }) => ({
  visitor: {
    MemberExpression(p, state) {
      const { object, property, computed } = p.node;
      if (computed || !t.isIdentifier(object) || !t.isIdentifier(property)) return;
      const members = ENUMS.get(object.name);
      if (!members) return;
      const binding = p.scope.getBinding(object.name);
      if (binding && !binding.path.isImportSpecifier() && !state.declared.has(binding.identifier)) return;
      if (!members.has(property.name)) throw p.buildCodeFrameError(stale(`${object.name}.${property.name}`));
      p.replaceWith(t.valueToNode(members.get(property.name)));
    },
    TSEnumDeclaration(p) {
      if (!p.node.const) return;
      if (!ENUMS.has(p.node.id.name)) {
        if (p.parentPath.isExportNamedDeclaration() && !REFUSED.has(p.node.id.name)) throw p.buildCodeFrameError(stale(`const enum ${p.node.id.name}`));
        return;
      }
      // The declaration itself first: a path struck only through its parent
      // still reads as live to the plugins after this one on the same node,
      // and the TypeScript transform turns it into a declaration with no
      // parent.
      const parent = p.parentPath;
      p.remove();
      if (parent.isExportNamedDeclaration()) parent.remove();
    },
    Program: {
      // The file's own declarations, by the identifier that names them, read
      // before any is struck: the traversal reaches a declaration before the
      // uses below it, and a struck path answers no question about itself.
      enter(p, state) {
        state.declared = new Set();
        for (const s of p.get("body")) {
          const d = s.isExportNamedDeclaration() ? s.get("declaration") : s;
          if (d.isTSEnumDeclaration() && d.node.const && ENUMS.has(d.node.id.name)) state.declared.add(d.node.id);
        }
      },
      exit(p, state) {
        p.traverse({
          ReferencedIdentifier(r) {
            if (!ENUMS.has(r.node.name) || r.parentPath.isTSTypeReference() || r.parentPath.isTSQualifiedName()) return;
            if (r.findParent((a) => a.isTSType() || a.isTSTypeAnnotation() || a.isTSTypeParameterInstantiation())) return;
            const binding = r.scope.getBinding(r.node.name);
            if (binding && !binding.path.isImportSpecifier()) return;
            throw r.buildCodeFrameError(`const enum ${r.node.name} is used as a value in ${state.filename}; it cannot be inlined`);
          },
        });
      },
    },
  },
});

module.exports = (api) => {
  const production = process.env.NODE_ENV === "production";
  api.cache.using(() => process.env.NODE_ENV);

  const DEFINE = {
    __IS_DEV__: !production,
    __DEV_BUNDLE__: !production,
    __APP_ENV__: production ? "production" : "development",
    // Every route a dev server forwards is the published deployment's.
    __PUBLISHED_TILES__: true,
  };

  /** Replaces each free name above with its value. */
  const define = ({ types: t }) => ({
    visitor: {
      Identifier(p) {
        const name = p.node.name;
        if (!Object.hasOwn(DEFINE, name) || !p.isReferencedIdentifier() || p.scope.hasBinding(name)) return;
        p.replaceWith(t.valueToNode(DEFINE[name]));
      },
    },
  });

  // Expo's own copy of the preset, found from Expo, which is what the app
  // depends on.
  const preset = require.resolve("babel-preset-expo", { paths: [require.resolve("expo/package.json")] });
  // The defines and the enums are this tree's: a package's own sources are
  // left as they are, where a name of theirs could be one of ours — @expo/ui
  // has a `Slot` of its own, which the enum pass took for the mode bar's.
  // A function and not a RegExp: Metro reads the config with no file for its
  // cache key, and Babel refuses a RegExp it has nothing to test against.
  return { presets: [preset], overrides: [{ exclude: (file) => /[\\/]node_modules[\\/]/.test(file ?? ""), plugins: [define, inline_enums] }] };
};
