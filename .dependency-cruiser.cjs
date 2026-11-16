/** UI import rules from AGENTS.md. */
module.exports = {
  forbidden: [
    {
      name: "no-ui-or-lib-to-features-bridge-app",
      comment: "ui/ and lib/ never import features/, bridge/ or app/.",
      severity: "error",
      from: { path: "^src/(ui|lib)/" },
      to: { path: "^src/(features|bridge|app)/" },
    },
    {
      name: "no-feature-to-feature",
      comment: "A feature never imports another feature; shared code moves to ui/ or lib/.",
      severity: "error",
      from: { path: "^src/features/([^/]+)/" },
      to: { path: "^src/features/", pathNot: "^src/features/$1/" },
    },
    {
      name: "only-bridge-talks-to-tauri",
      comment: "Only bridge/ imports @tauri-apps/api.",
      severity: "error",
      from: { path: "^src/", pathNot: "^src/bridge/" },
      to: { path: "@tauri-apps/api", dependencyTypes: ["npm"] },
    },
    {
      name: "no-circular",
      severity: "error",
      from: {},
      to: { circular: true },
    },
  ],
  options: {
    doNotFollow: { path: "node_modules" },
    // TypeScript 7's compiler API is not published yet; swc parses the sources.
    parser: "swc",
    tsConfig: { fileName: "tsconfig.json" },
    enhancedResolveOptions: {
      exportsFields: ["exports"],
      conditionNames: ["import", "require", "node", "default", "types"],
      extensions: [".ts", ".tsx", ".js", ".jsx"],
    },
    reporterOptions: { text: { highlightFocused: true } },
  },
};