// @pxl/build/require-pnpm.ts

/** The workspace's preinstall: it is installed with pnpm, the version
 * package.json's `packageManager` names. */
const agent = process.env.npm_config_user_agent ?? "";
if (!agent.startsWith("pnpm/")) {
  console.error("ts/ installs with pnpm: `corepack pnpm install`.");
  process.exit(1);
}
