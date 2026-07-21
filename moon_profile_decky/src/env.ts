// Build-time constant, inlined by rollup's replace() plugin
// (rollup.config.js) from a local .env file (gitignored - this repo is
// public) directly into the literal string baked into dist/index.js.
// Not a runtime setting: there is exactly one SteamGridDB account behind
// this plugin, the maintainer's own, so there's nothing to reconfigure
// per install (see docs/prd.md's non-Steam artwork section).
export const STEAMGRIDDB_API_KEY: string = process.env.STEAMGRIDDB_API_KEY ?? "";
