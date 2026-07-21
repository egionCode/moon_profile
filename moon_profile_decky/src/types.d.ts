declare module "*.svg" {
  const content: string;
  export default content;
}

declare module "*.png" {
  const content: string;
  export default content;
}

declare module "*.jpg" {
  const content: string;
  export default content;
}

// Substituted by rollup's replace() plugin at build time (see
// rollup.config.js/src/env.ts) - not a real Node process object, just
// enough of a shape for that one property to type-check.
declare const process: { env: { STEAMGRIDDB_API_KEY?: string } };
