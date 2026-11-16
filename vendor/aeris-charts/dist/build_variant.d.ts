/** Which published build the loaded WebAssembly module is. */
export type chart_build_variant = "standard" | "complete";
/** Package entry that contains every complete-only chart family. */
export declare const COMPLETE_BUILD_ENTRY = "@aeristerminal/aeris-charts/complete";
/**
 * Chart-family kinds whose engine modules exist only behind the `complete` Cargo feature. A family
 * whose module is gated adds its creation kind here; the creation APIs reject it atomically in the
 * standard build, before any engine mutation.
 */
export declare const complete_only_families: ReadonlySet<string>;
/** Record which build the module just initialized; called once WebAssembly instantiation succeeds. */
export declare function record_loaded_build(): void;
/**
 * The build of the initialized WebAssembly engine: `"standard"` for
 * `@aeristerminal/aeris-charts`, `"complete"` for `@aeristerminal/aeris-charts/complete`. The
 * answer comes from the engine itself. Throws `unsupported_operation` before the engine has been
 * initialized by `init_wasm`, `create_chart`, or another chart constructor.
 */
export declare function build_variant(): chart_build_variant;
/** Camel-case alias of {@link build_variant}. */
export declare const buildVariant: typeof build_variant;
/** Throw `complete_build_required` when `kind` is a complete-only family and the standard build is loaded. */
export declare function require_family_in_build(kind: string): void;
