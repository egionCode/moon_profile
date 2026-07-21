import { existsSync, readFileSync } from "fs";
import deckyPlugin from "@decky/rollup";
import replace from "@rollup/plugin-replace";

// Minimal .env parser (KEY=VALUE per line, "#" comments and blank lines
// skipped) - avoids pulling in the "dotenv" package for a single
// build-time constant. .env itself is gitignored (this repo is public
// on GitHub); see .env.example for the expected keys.
function loadDotEnv(path) {
  if (!existsSync(path)) {
    return {};
  }
  const values = {};
  for (const line of readFileSync(path, "utf-8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }
    const eq = trimmed.indexOf("=");
    if (eq === -1) {
      continue;
    }
    values[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
  }
  return values;
}

const env = loadDotEnv("./.env");

export default deckyPlugin({
  plugins: [
    // Inlines the real SteamGridDB key into the built dist/index.js (see
    // src/env.ts) - it's the maintainer's own personal account, not a
    // per-install setting, so there's no Settings UI for it anymore
    // (removed from GamesGridSection.tsx/Config).
    replace({
      preventAssignment: false,
      "process.env.STEAMGRIDDB_API_KEY": JSON.stringify(env.STEAMGRIDDB_API_KEY ?? ""),
    }),
  ],
});
