/**
 * @aeristerminal/aeris-charts — public API.
 *
 * A thin, typed façade over the `aeris_charts_wasm` engine. It owns the browser-side concerns that the
 * engine deliberately leaves out: creating the stacked canvases (WebGPU/Canvas2D pane + package-owned
 * plugin overlay + Canvas2D axis/input overlay), the input-gesture recognizer, typed-array packing
 * of data at the boundary, and the chart/series/time-scale handle objects. All model, layout, and
 * rendering math lives in Rust.
 *
 * WebGPU device acquisition is async, so `create_chart` returns a Promise — the one deliberate
 * divergence from the reference's synchronous `createChart`.
 */
export * from "./types.js";
export * from "./errors.js";
export * from "./theme.js";
export * from "./primitives.js";
export * from "./canvas_plugins.js";
export * from "./custom_series.js";
export * from "./feature_series.js";
export * from "./builtin_plugins.js";
export * from "./primitive_features.js";
export * from "./general_components.js";
export * from "./accessibility.js";
export * from "./offscreen.js";
export * from "./shortcuts.js";
export * from "./grid.js";
export { build_variant, buildVariant, type chart_build_variant } from "./build_variant.js";
import type { chart_api, chart_options, deep_partial } from "./types.js";
/**
 * Pre-instantiate the wasm engine, optionally from an explicit URL. Only needed when the default
 * resolution (the `.wasm` shipped beside `dist/index.js`) doesn't apply — e.g. Vite's dev
 * pre-bundler (`import wasm_url from "@aeristerminal/aeris-charts/wasm?url"`). Call once,
 * before the first `create_chart`; later calls are no-ops.
 */
export declare function init_wasm(wasm_url?: string | URL): Promise<unknown>;
/** Camel-case alias of {@link init_wasm}. */
export declare const initWasm: typeof init_wasm;
/**
 * Create a chart inside `container`. Resolves once the rendering backend is ready and the first
 * frame is drawn. It provides a familiar `createChart`-style entry point and is asynchronous because
 * backend acquisition must complete first.
 */
export declare function create_chart(container: HTMLElement, options?: deep_partial<chart_options>): Promise<chart_api>;
/** Camel-case alias of {@link create_chart}; both return the same chart API. */
export declare const createChart: typeof create_chart;
