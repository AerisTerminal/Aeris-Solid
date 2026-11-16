import { AerisChart } from "../pkg/aeris_charts_wasm.js";
import { type theme_name } from "./theme.js";
import type { any_series_options, backend_status, chart_options, deep_partial, frame_stats, footprint_bar, footprint_trade, footprint_trade_columns, ingestion_diagnostics, ohlc_columns, series_kind, series_data } from "./types.js";
declare const UNSUPPORTED_WORKER_OPTIONS: readonly ["autoSize", "localization", "handle_scroll", "handle_scale", "kinetic_scroll", "tracking_mode", "accessibility"];
type offscreen_layout_options = Omit<deep_partial<chart_options["layout"]>, "panes"> & {
    panes?: Omit<deep_partial<chart_options["layout"]["panes"]>, "enableResize">;
};
/** Engine-owned options supported by a worker chart. DOM gestures, localization callbacks,
 * ResizeObserver sizing, and pane-separator dragging stay on the DOM `create_chart` façade. */
export type offscreen_chart_options = Omit<deep_partial<chart_options>, (typeof UNSUPPORTED_WORKER_OPTIONS)[number] | "layout"> & {
    layout?: offscreen_layout_options;
};
export interface offscreen_chart_init {
    /** Chart media size in CSS pixels. Workers cannot infer this from a transferred canvas. */
    width: number;
    height: number;
    /** Explicit device-pixel ratio used for both backing stores and engine geometry. */
    dpr: number;
    options?: offscreen_chart_options;
    /** Initial kind for the construction-time primary series (default `candlestick`). */
    series_kind?: series_kind;
    /** Test/runtime hook matching the DOM constructor's deterministic fallback path. */
    force_fallback_adapter?: boolean;
}
export interface offscreen_pointer_event {
    type: "down" | "move" | "up" | "cancel" | "leave";
    /** Full-chart media coordinates, not bitmap coordinates. */
    x: number;
    y: number;
    pointer_id?: number;
    click_count?: number;
    button?: number;
    buttons?: number;
    ctrl_key?: boolean;
    meta_key?: boolean;
    shift_key?: boolean;
    alt_key?: boolean;
    pointer_type?: "mouse" | "touch" | "pen";
    timestamp_ms?: number;
    /** The relaying page's `prefers-reduced-motion`; the worker keeps the last reported value. */
    reduced_motion?: boolean;
    pressure?: number;
    tilt_x?: number;
    tilt_y?: number;
}
export interface offscreen_wheel_event {
    /** Full-chart media coordinates. */
    x: number;
    y: number;
    delta_x: number;
    delta_y: number;
    /** 0 = pixels, 1 = lines, 2 = pages (the WheelEvent values). */
    delta_mode?: 0 | 1 | 2;
    /** Pixel-mode raw delta unit, in device pixels per CSS pixel; omit for CSS-pixel events. */
    pixel_ratio?: number;
    /** Ctrl (or `meta_key`, macOS Cmd) zooms around the pointer instead of pinning the right edge. */
    ctrl_key?: boolean;
    shift_key?: boolean;
    alt_key?: boolean;
    meta_key?: boolean;
    timestamp_ms?: number;
}
export interface offscreen_key_event {
    key: string;
    /** Omit for a discrete press; use down/up for a held key. */
    type?: "press" | "down" | "up";
    ctrl_key?: boolean;
    shift_key?: boolean;
    alt_key?: boolean;
    meta_key?: boolean;
    repeat?: boolean;
    timestamp_ms?: number;
    reduced_motion?: boolean;
}
/**
 * A chart hosted entirely in a worker. It intentionally exposes the allocation-free typed data
 * path and normalized input injection rather than DOM-only APIs (accessibility, HTML plugins,
 * synchronous screenshots, and ResizeObserver sizing).
 */
export declare class offscreen_chart {
    private readonly wasm;
    private readonly gpu_canvas;
    private readonly fallback_canvas;
    private wheel_behavior;
    private price_axis_wheel_zoom;
    private selected_theme;
    register_custom_study(): never;
    add_custom_study(): never;
    private readonly stats_scratch;
    private readonly backend_change_handlers;
    private primary_adopted;
    private last_ingestion;
    private removed;
    private input_timer;
    /** Pending frame callback advancing kinetic scroll or animated zoom. */
    private input_frame;
    private readonly font_loading_done_handler;
    private width;
    private height;
    private dpr;
    private last_backend;
    private reduced_motion;
    private readonly backend_loss_handler;
    constructor(wasm: AerisChart, gpu_canvas: OffscreenCanvas, fallback_canvas: OffscreenCanvas, width: number, height: number, dpr: number, wheel_behavior: "auto" | "pan" | "zoom", price_axis_wheel_zoom: boolean, selected_theme?: theme_name);
    backend(): "webgpu" | "canvas2d";
    /** Structured backend selection/fallback diagnostics for this worker chart. */
    backend_status(): Readonly<backend_status>;
    /** Diagnostics for the most recent worker-side data ingestion. */
    last_ingestion_diagnostics(): ingestion_diagnostics | null;
    private record_ingestion;
    /** Subscribe to runtime WebGPU → Canvas2D fallback. The owner should reveal the HTML canvas
     * matching `backend`; returns an unsubscribe function. Construction-time backend is available
     * synchronously from {@link backend}. */
    subscribe_backend_change(handler: (backend: "webgpu" | "canvas2d") => void): () => void;
    /** Adopt the primary series on the first call; later calls append independent series. */
    add_series(kind: series_kind, options?: Partial<any_series_options>): number;
    set_data_typed(columns: ohlc_columns, series_id?: number): void;
    update_typed(columns: ohlc_columns, series_id?: number): void;
    /** Replace structured data for a Rust-native advanced series in a worker chart. */
    set_feature_data(data: readonly series_data[], series_id?: number): void;
    /** Append or replace one structured advanced-series row in a worker chart. */
    update_feature(item: series_data, series_id?: number): void;
    set_footprint_trades_typed(columns: footprint_trade_columns, series_id?: number): void;
    update_footprint_trades_typed(columns: footprint_trade_columns, series_id?: number): "tip" | "historical";
    update_footprint_trade(trade: footprint_trade, series_id?: number): "tip" | "historical";
    footprint_bars(series_id?: number): readonly footprint_bar[];
    apply_options(options: offscreen_chart_options): void;
    /** Restore canonical Aeris visual defaults for the currently selected theme without resetting
     * data, scale/view state, or advanced-series semantics. */
    reset_style_to_defaults(): void;
    fit_content(): void;
    /** Open the footprint view: readable cluster zoom at the real-time edge with footprint autoscale. */
    fit_footprint_viewport(): void;
    resize(width: number, height: number, dpr?: number): void;
    render(): void;
    /**
     * Inject a normalized pointer sample relayed from the host. Primary-pointer drags pan the time
     * scale; hover/move drives the crosshair. Axis coordinates are accepted but do not begin a pan.
     */
    inject_pointer_event(event: offscreen_pointer_event): void;
    /** Inject a relayed wheel sample using the DOM recognizer's delta normalization. */
    inject_wheel_event(event: offscreen_wheel_event): void;
    /** Route a worker key sample through the same controller used by browser and GPUI hosts. */
    inject_key_event(event: offscreen_key_event): boolean;
    private request_input_frame;
    private consume_input_events;
    private schedule_input_tick;
    visible_logical_range(): {
        from: number;
        to: number;
    } | null;
    frame_stats(): frame_stats;
    remove(): void;
    private assert_live;
}
/**
 * Construct a worker chart from two transferred canvases. The caller should display only the
 * canvas matching `chart.backend()`; two surfaces preserve runtime Canvas2D fallback without
 * rebuilding chart state.
 */
/** @experimental Worker/offscreen hosting is not part of the frozen stable surface. */
export declare function create_offscreen_chart(gpu_canvas: OffscreenCanvas, fallback_canvas: OffscreenCanvas, init: offscreen_chart_init): Promise<offscreen_chart>;
export {};
