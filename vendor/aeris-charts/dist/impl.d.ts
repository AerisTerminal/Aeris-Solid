/**
 * Handle implementations over the wasm engine: series, time scale, price scale, pane, chart.
 * Extracted from `index.ts`.
 */
import { AerisChart } from "../pkg/aeris_charts_wasm.js";
import type { series_primitive, series_primitive_handle } from "./primitives.js";
import type { canvas_primitive, canvas_primitive_handle } from "./canvas_plugins.js";
import type { custom_series_pane_view } from "./custom_series.js";
import { type accessibility_handle } from "./accessibility.js";
import type { alert_api, crosshair_action_request_handler, any_series_options, backend_status, bars_info, chart_api, chart_context_handler, chart_options, chart_state, chart_value_snapshot, comparison_legend_entry, data_changed_handler, dbl_click_handler, deep_partial, drawing_api, drawing_created_handler, drawing_kind, drawing_options, depth_event_columns, depth_event_layer_options, depth_heatmap_options, depth_ladder_row, depth_options, depth_profile_options, depth_snapshot_columns, depth_study_snapshot, depth_update_columns, drawing_point, drawing_tool_change_handler, drawing_interval, drawing_property_schema, drawing_kind_options, drawing_template, builtin_drawing_icon, kst_periods, study_calendar_policy, previous_period, frame_stats, footprint_series_api, footprint_series_options, footprint_trade, footprint_trade_columns, general_accessibility_snapshot, general_axis_api, general_axis_options, general_brush_range, general_brush_snapshot, general_legend_snapshot, general_pane_composition, general_pane_composition_result, general_pane_options, general_reference_api, general_reference_options, general_series_api, general_series_hit, general_series_kind, general_series_options, general_shared_tooltip_snapshot, general_tooltip_snapshot, general_update_options, general_xy_row, layout_hit, layout_input, layout_series_api, layout_series_kind, layout_series_options, box_plot_row, bubble_columns, bubble_row, bullet_row, category_box_columns, category_bullet_columns, category_waterfall_columns, waterfall_row, category_error_columns, category_heatmap_columns, category_range_columns, category_xy_columns, numeric_error_columns, numeric_heatmap_columns, numeric_range_columns, numeric_xy_columns, error_bar_row, general_matrix_columns, heatmap_grid_row, histogram_columns, histogram_row, matrix_row, range_area_row, temporal_error_columns, temporal_heatmap_columns, temporal_range_columns, temporal_xy_columns, ingestion_diagnostics, handle_scale_options, handle_scroll_options, indicator_info, indicator_input_source, indicator_kind, indicator_output_style, indicator_schema, kinetic_scroll_options, pivot_kind, vwap_reset, last_value_data, localization_options, logical_range, mismatch_direction, mouse_event_handler, ohlc_columns, ohlc_data, options_change_handler, pane_api, price_line_api, price_line_options, persistence_restore_result, price_scale_api, price_scale_create_options, price_scale_info, ring_source_layout, series_api, series_change_handler, series_data, series_kind, series_marker, series_marker_options, series_options, size_change_handler, time, replay_change_handler, replay_clock_stats, replay_seek_stats, replay_state, replay_step, resample_boundary, resample_options, resampled_bar, study_annotations, synthetic_bar, synthetic_bar_options, trade_stream_stats, profile_source, profile_request, profile_snapshot, naked_profile_level, tpo_request, tpo_snapshot, tpo_series_api, tpo_series_options, periodic_profile_presentation_request, periodic_profile_presentation_options, anchored_vwap_point, profile_drawing_options, profile_drawing_snapshot, time_and_sales_options, time_and_sales_row, time_scale_api, tracking_mode_options, trading_api, trading_hit, chart_sync_event, crosshair_sync_position, trading_intent, visible_logical_range_handler, visible_time_range_handler, volume_profile_indicator_api, volume_profile_indicator_options, options_volume_api, options_volume_row, big_trades_api, big_trades_options, auction_marker_options, auction_markers_api, custom_study_definition, custom_study_fault_event, custom_study_binding_options } from "./types.js";
import { popover_palette, type theme_name } from "./theme.js";
/** Whether the host supplied an Aeris Charts Commercial License key. */
export declare function has_license_key(license_key: unknown): boolean;
/** Print the runtime license notice once per page unless a commercial license key is supplied. */
export declare function announce_license(license_key: unknown): void;
/**
 * Instantiate the wasm module once per page. `wasm_url` overrides the default asset resolution
 * (`new URL("aeris_charts_wasm_bg.wasm", import.meta.url)` beside the bundle) — the escape hatch for
 * bundlers that relocate the JS away from the .wasm (e.g. Vite's dev pre-bundler). Only the
 * first call's argument takes effect.
 */
export declare function ensure_init(wasm_url?: string | URL): Promise<unknown>;
/**
 * Convert a `time` input to the engine's UTC-seconds form. Business days and strict `"YYYY-MM-DD"`
 * strings are taken at UTC midnight. A malformed or normalized date yields `NaN`, which causes the
 * ingestion transaction to be rejected.
 */
export declare function time_to_utc_seconds(t: time): number;
declare class general_series_impl implements general_series_api {
    readonly id: number;
    private readonly dataset;
    readonly kind: general_series_kind;
    private x_axis_id;
    private y_axis_id;
    private readonly chart;
    applyOptions(...args: Parameters<general_series_api["apply_options"]>): void;
    setVisible(...args: Parameters<general_series_api["set_visible"]>): void;
    setData(...args: Parameters<general_series_api["set_data"]>): void;
    setDataTyped(...args: Parameters<general_series_api["set_data_typed"]>): void;
    updateData(...args: Parameters<general_series_api["update_data"]>): void;
    updateDataTyped(...args: Parameters<general_series_api["update_data_typed"]>): void;
    dataAt(...args: Parameters<general_series_api["data_at"]>): general_tooltip_snapshot | null;
    selectedHit(): general_series_hit | null;
    accessibilitySnapshot(...args: Parameters<general_series_api["accessibility_snapshot"]>): general_accessibility_snapshot;
    private readonly data_changed_subs;
    private removed;
    constructor(id: number, dataset: number, kind: general_series_kind, x_axis_id: string, y_axis_id: string, chart: chart_impl);
    mark_removed(): void;
    retarget(x_axis_id: string, y_axis_id: string): void;
    private assert_live;
    private x_scale;
    options(): general_series_options;
    apply_options(patch: Partial<general_series_options>): void;
    set_visible(visible: boolean): void;
    set_data(data: readonly (general_xy_row | bubble_row | range_area_row | error_bar_row | box_plot_row | waterfall_row | bullet_row | heatmap_grid_row | histogram_row | matrix_row)[]): void;
    set_data_typed(columns: numeric_xy_columns | temporal_xy_columns | category_xy_columns | bubble_columns | numeric_range_columns | temporal_range_columns | category_range_columns | numeric_error_columns | temporal_error_columns | category_error_columns | category_box_columns | category_waterfall_columns | category_bullet_columns | category_heatmap_columns | numeric_heatmap_columns | temporal_heatmap_columns | histogram_columns | general_matrix_columns): void;
    update_data(data: readonly (general_xy_row | bubble_row | range_area_row | error_bar_row | box_plot_row | waterfall_row | bullet_row | heatmap_grid_row | histogram_row | matrix_row)[], options?: general_update_options): void;
    update_data_typed(columns: numeric_xy_columns | temporal_xy_columns | category_xy_columns | bubble_columns | numeric_range_columns | temporal_range_columns | category_range_columns | numeric_error_columns | temporal_error_columns | category_error_columns | category_box_columns | category_waterfall_columns | category_bullet_columns | category_heatmap_columns | numeric_heatmap_columns | temporal_heatmap_columns | histogram_columns | general_matrix_columns, options?: general_update_options): void;
    private upsert_data;
    private install_data;
    data_at(row: number): general_tooltip_snapshot | null;
    selected_hit(): general_series_hit | null;
    accessibility_snapshot(offset?: number, limit?: number): general_accessibility_snapshot;
    subscribe_data_changed(handler: data_changed_handler): void;
    unsubscribe_data_changed(handler: data_changed_handler): void;
    remove(): void;
    remove_from_engine(): boolean;
}
declare class series_impl implements series_api {
    readonly id: number;
    readonly kind: series_kind;
    protected readonly chart: chart_impl;
    setData(...args: Parameters<series_api["set_data"]>): void;
    setDataTyped(...args: Parameters<series_api["set_data_typed"]>): void;
    updateTyped(...args: Parameters<series_api["update_typed"]>): void;
    applyOptions(...args: Parameters<series_api["apply_options"]>): void;
    moveToPane(...args: Parameters<series_api["move_to_pane"]>): void;
    priceScale(): price_scale_api;
    protected readonly data_changed_subs: Set<data_changed_handler>;
    private removed;
    private last_ingestion;
    constructor(id: number, kind: series_kind, chart: chart_impl);
    /** Called by `chart_impl.remove_series`; makes every subsequent method on this handle throw. */
    mark_removed(): void;
    /**
     * Fire this series' `data_changed` after a ring drain delivered rows to it. Scope is `"update"`:
     * a drain appends or replace-lasts, exactly like `update`/`update_typed`. Called by the chart's
     * drain loop, which already owns the repaint, so this only notifies.
     */
    emit_ring_data_changed(): void;
    protected assert_live(): void;
    set_data(data: readonly series_data[]): void;
    /**
     * Columnar fast path: already-packed typed arrays go straight to the engine, skipping
     * the per-object JS packing of `set_data`, which is avoidable work for feed handlers that
     * already hold columnar data. `times` are UTC seconds
     * (the engine's time unit, see `time_to_utc_seconds`); single-value series repeat
     * their value across all four price channels. Per-point colors are not carried —
     * apply them after with the usual options/colors path. The engine's sort/dedupe/
     * sanitize rules apply exactly as with `set_data`.
     */
    set_data_typed(columns: ohlc_columns): void;
    /**
     * Columnar streaming fast path: the batch goes straight to the engine as five typed arrays,
     * so a feed running at tens of thousands of ticks per second allocates no per-point JS object
     * at the boundary. One `data_changed("update")` per call regardless of batch size — the
     * notification describes the call, not the rows, exactly as it does for `update`.
     */
    update_typed(columns: ohlc_columns): void;
    /**
     * Bind (or with `null`, unbind) a `SharedArrayBuffer` ring the engine drains on its own frame
     * tick. The two typed-array views are built here, once, and handed to the engine — it holds them
     * for the lifetime of the binding so no view is constructed per frame.
     */
    set_ring_source(buffer: SharedArrayBuffer | null, layout?: ring_source_layout): void;
    update(point: series_data): void;
    last_ingestion_diagnostics(): ingestion_diagnostics | null;
    protected record_ingestion(json: string | undefined): boolean;
    private record_single_ingestion;
    pop(count?: number): void;
    last_value_data(global_last?: boolean): last_value_data | null;
    price_formatter(): (price: number) => string;
    apply_options(options: Partial<any_series_options>): void;
    /** Push the current bid/ask quotes (`bid_ask_visible` renders them); `null` hides a side. */
    set_bid_ask(bid: number | null, ask: number | null): void;
    options(): any_series_options;
    set_type(kind: series_kind): void;
    move_to_pane(pane_index: number, stretch?: number): void;
    create_price_line(options: price_line_options): price_line_api;
    set_markers(markers: readonly series_marker[], options?: Partial<series_marker_options>): void;
    price_scale(): price_scale_api;
    price_scale_id(): string;
    move_to_price_scale(id: string): void;
    pane_index(): number;
    price_to_coordinate(price: number): number | null;
    coordinate_to_price(coordinate: number): number | null;
    bars_in_logical_range(range: logical_range): bars_info | null;
    private unpack_point;
    data_by_index(logical_index: number, mismatch_direction?: mismatch_direction): series_data | null;
    data(): readonly series_data[];
    series_type(): series_kind;
    indicator_info(): indicator_info | null;
    indicator_output_style(): indicator_output_style | null;
    set_indicator_output_style(patch: Partial<indicator_output_style>): boolean;
    subscribe_data_changed(handler: data_changed_handler): void;
    unsubscribe_data_changed(handler: data_changed_handler): void;
    attach_primitive(primitive: series_primitive): series_primitive_handle;
    /** Package-internal boundary for the first-class Rust primitive helpers. */
    native_add_image_watermark(width: number, height: number, pixels: Uint8Array, options_json: string): number;
    native_add_anchored_text(options_json: string): number;
    native_set_anchored_text_options(id: number, options_json: string): boolean;
    native_add_bands_indicator(options_json: string): number;
    native_set_bands_indicator_options(primitive_id: number, options_json: string): boolean;
    native_add_overlay_price_scale(options_json: string): number;
    native_set_overlay_price_scale_options(primitive_id: number, options_json: string): boolean;
    native_add_accessibility_focus(options_json: string): number;
    native_set_accessibility_focus(primitive_id: number, time: number | null, options_json: string): boolean;
    native_add_session_highlighting(options_json: string): number;
    native_set_session_highlighting_data(primitive_id: number, highlights_json: string): boolean;
    native_add_crosshair_highlight(color?: string): number;
    native_add_vertical_line(time: number, options_json: string): number;
    native_add_delta_tooltip(options_json: string): number;
    native_set_brushable_area(options_json: string): boolean;
    native_brushable_area_range_json(): string;
    native_clear_brushable_area(): boolean;
    native_set_area_brush_state(state_json: string): boolean;
    native_add_tooltip(options_json: string): number;
    native_set_tooltip_options(primitive_id: number, options_json: string): boolean;
    native_tooltip_snapshot_json(primitive_id: number): string;
    native_delta_tooltip_active_range_json(primitive_id: number): string;
    native_clear_delta_tooltip(primitive_id: number): boolean;
    native_add_trend_line(first_time: number, first_price: number, second_time: number, second_price: number, options_json: string): number;
    native_add_volume_profile(data_json: string, options_json: string): number;
    native_set_volume_profile_data(id: number, data_json: string): boolean;
    native_remove_primitive(id: number): void;
    native_repaint(): void;
}
export interface native_primitive_handle {
    detach(): void;
}
export interface native_bands_indicator_handle extends native_primitive_handle {
    set_options_json(options_json: string): boolean;
}
export interface native_overlay_price_scale_handle extends native_primitive_handle {
    set_options_json(options_json: string): boolean;
}
export interface native_volume_profile_handle extends native_primitive_handle {
    set_data_json(data_json: string): boolean;
}
export interface native_accessibility_focus_handle extends native_primitive_handle {
    set(time: number | null, options_json: string): boolean;
}
export interface native_session_highlighting_handle extends native_primitive_handle {
    set_data_json(data_json: string): boolean;
}
export interface native_anchored_text_handle extends native_primitive_handle {
    set_options_json(options_json: string): boolean;
}
export interface native_text_watermark_handle extends native_primitive_handle {
    set_options_json(options_json: string): boolean;
}
export interface native_delta_tooltip_handle extends native_primitive_handle {
    active_range_json(): string;
    clear(): boolean;
}
export interface native_tooltip_handle extends native_primitive_handle {
    set_options_json(options_json: string): boolean;
    snapshot_json(): string;
}
/** Package-internal controller for transient brush styling on a built-in Area series. */
export declare function set_native_area_brush_state(series: series_api, state_json: string): boolean;
/** Package-internal attachment for the engine-owned brushable Area composition. */
export declare function attach_native_brushable_area(series: series_api, options_json: string): {
    active_range_json(): string;
    clear(): boolean;
    detach(): void;
};
export declare function attach_native_bands_indicator(series: series_api, options_json: string): native_bands_indicator_handle;
export declare function attach_native_overlay_price_scale(series: series_api, options_json: string): native_overlay_price_scale_handle;
export declare function attach_native_accessibility_focus(series: series_api, options_json: string): native_accessibility_focus_handle;
export declare function attach_native_session_highlighting(series: series_api, options_json: string): native_session_highlighting_handle;
export declare function attach_native_crosshair_highlight(series: series_api, color?: string): native_primitive_handle;
export declare function attach_native_vertical_line(series: series_api, time: number, options_json: string): native_primitive_handle;
export declare function attach_native_delta_tooltip(series: series_api, options_json: string): native_delta_tooltip_handle;
export declare function attach_native_tooltip(series: series_api, options_json: string): native_tooltip_handle;
export declare function attach_native_trend_line(series: series_api, first_time: number, first_price: number, second_time: number, second_price: number, options_json: string): native_primitive_handle;
export declare function attach_native_volume_profile(series: series_api, data_json: string, options_json: string): native_volume_profile_handle;
export declare function attach_native_image_watermark(series: series_api, width: number, height: number, pixels: Uint8Array, options_json: string): native_primitive_handle;
export declare function attach_native_anchored_text(series: series_api, options_json: string): native_anchored_text_handle;
export declare function attach_native_text_watermark(pane: pane_api, options_json: string): native_text_watermark_handle;
/** One registered canvas primitive (Phase C-e) in the package-side registry. */
interface canvas_primitive_entry {
    primitive: canvas_primitive;
    pane_index: number;
    detached: boolean;
}
/** Gesture toggles resolved to concrete values the recognizer reads on each event. */
export interface resolved_gestures {
    pan: boolean;
    pan_horz_touch: boolean;
    pan_vert_touch: boolean;
    wheel_scroll: boolean;
    wheel_zoom: boolean;
    price_axis_wheel_zoom: boolean;
    pinch_zoom: boolean;
    axis_dblclick_reset_time: boolean;
    axis_dblclick_reset_price: boolean;
    axis_scale_price: boolean;
    axis_scale_time: boolean;
    kinetic_touch: boolean;
    kinetic_mouse: boolean;
    wheel_behavior: "auto" | "pan" | "zoom";
    panes_resize: boolean;
    tracking_exit_mode: "on_next_tap" | "on_touch_end";
}
export interface chart_host_snapshot {
    readonly position_value: string;
    readonly position_priority: string;
    readonly assigned_position: boolean;
    readonly role: string | null;
    readonly aria_label: string | null;
    owned_role: boolean;
    owned_aria_label: string | null;
}
export declare function snapshot_chart_host(container: HTMLElement): chart_host_snapshot;
export declare function restore_chart_host(container: HTMLElement, snapshot: chart_host_snapshot): void;
export declare class chart_impl implements chart_api {
    private readonly container;
    private readonly gpu_pane;
    private readonly fallback_pane;
    private readonly plugin_canvas;
    private readonly overlay;
    private selected_theme;
    private readonly host_snapshot;
    addSeries(kind: "footprint", options?: Partial<any_series_options> & Partial<footprint_series_options>): footprint_series_api;
    addSeries(kind: "tpo", options?: Partial<any_series_options> & Partial<tpo_series_options>): tpo_series_api;
    addSeries(kind: general_series_kind, options: general_series_options): general_series_api;
    addSeries(kind: series_kind, options?: Partial<any_series_options>): series_api;
    removeSeries(...args: Parameters<chart_api["remove_series"]>): void;
    addPane(preserve_empty?: boolean): pane_api;
    addPane(options: general_pane_options): pane_api;
    addAxis(...args: Parameters<chart_api["add_axis"]>): general_axis_api;
    removeAxis(...args: Parameters<chart_api["remove_axis"]>): boolean;
    applyOptions(...args: Parameters<chart_api["apply_options"]>): void;
    timeScale(): time_scale_api;
    priceScale(...args: Parameters<chart_api["price_scale"]>): price_scale_api;
    takeScreenshot(...args: Parameters<chart_api["take_screenshot"]>): HTMLCanvasElement;
    exportState(): chart_state;
    importState(...args: Parameters<chart_api["import_state"]>): persistence_restore_result;
    private wasm_instance;
    private guarded_wasm;
    private custom_study_chart_guard_installed;
    private in_custom_study_callback;
    private readonly custom_study_fault_subs;
    private next_extra_series;
    private readonly gestures_cfg;
    private accessibility_handle;
    /** DPR used by the engine/canvas, including an explicit manual-resize override. */
    private pixel_ratio;
    private readonly ts;
    private readonly trading_handle;
    private readonly alert_handle;
    private readonly crosshair_action_handlers;
    private observer;
    private detach_gestures;
    /** Re-reads controller cursor, events, repaint, and input wake after a non-input command. */
    private sync_gesture_input;
    private removed;
    private readonly series_by_id;
    /** Options-volume column series → the removal of their whole study. */
    private readonly options_volume_removers;
    private next_options_volume_id;
    private readonly general_series_by_id;
    private readonly layout_series_by_id;
    private readonly layout_change_subscribers_by_id;
    private readonly crosshair_subs;
    private readonly click_subs;
    private readonly chart_context_subs;
    private readonly dbl_click_subs;
    private readonly visible_logical_range_subs;
    private readonly visible_time_range_subs;
    private readonly size_change_subs;
    private readonly series_added_subs;
    private readonly series_removed_subs;
    private readonly options_change_subs;
    private readonly delta_tooltip_range_listeners;
    private readonly general_brush_change_subs;
    private readonly general_legend_change_subs;
    private last_visible_logical_range;
    private last_visible_time_range;
    private last_ts_width;
    private last_ts_height;
    private auto_size;
    private dpr_query;
    private reduced_motion_query;
    private readonly general_motion_change_handler;
    private general_motion_listener_bound;
    private readonly dpr_change_handler;
    /** Crosshair position tracked TS-side (for crosshair-less screenshots); `null` when hidden. */
    private last_crosshair;
    private dom_general_tooltips;
    private engine_general_tooltip_visible;
    /** Last hover hit-test result (Phase C-d), refreshed on crosshair moves; feeds event params. */
    private hover;
    /** Host-side serialization cache for the engine-owned armed drawing template. */
    private tool_options_json;
    private tool_listener;
    /** The armed tool last reported to listeners; the engine may disarm on its own. */
    private reported_tool;
    private readonly tool_change_subs;
    private readonly replay_change_subs;
    private readonly drawing_created_subs;
    /** Borderless caret surface shared by two explicitly separate product edit modes. */
    private text_editor;
    private text_editor_id;
    /** Snapshot of the drawing's text when the editor opened — restored on Escape. */
    /**
     * Re-anchor the open editor after any repaint-driving change (wheel zoom/scroll, pinch,
     * resize, data update): it re-queries the anchor's coordinates from the settled engine
     * scales and repositions the box, so the editor tracks its drawing instead of displacing.
     */
    private text_editor_reposition;
    private anim_frame;
    /** The pending second-aligned countdown tick; `null` while no countdown is visible. */
    private countdown_timer;
    /** True while any pointer/touch is down — pauses the countdown tick so it can't repaint mid-gesture. */
    private interacting;
    /** Pending rAF handle for a coalesced repaint; `null` when no repaint is scheduled. */
    private repaint_raf;
    /** The plugin canvas holds pixels from a previous canvas-primitive pass. */
    private plugin_canvas_painted;
    /** Pending rAF handle for the ring-drain loop; `null` while no ring source is bound. */
    private ring_raf;
    /** Reused `[pair_count, series_id, rows, ...]` report from `drain_ring_sources`. */
    private ring_report;
    /**
     * Reusable transfer buffer for `frame_stats()`. Sized by the engine itself so an engine that
     * appends a slot needs no matching bundle change, and allocated once per chart so a host
     * polling telemetry every frame adds no JS-heap allocation of its own (the acceptance bar is
     * that reading stats for 60s does not itself raise `cpu_ms`). Each read still copies these
     * few dozen bytes across the wasm boundary — fixed size, no growth.
     */
    private readonly stats_scratch;
    /** The gesture recognizer marks pointer/touch activity (down = true, all-up = false). */
    set_interacting(active: boolean): void;
    trading(): trading_api;
    alerts(): alert_api;
    set_crosshair_action_button_visible(visible: boolean): void;
    subscribe_crosshair_action(handler: crosshair_action_request_handler): void;
    unsubscribe_crosshair_action(handler: crosshair_action_request_handler): void;
    accessibility(): accessibility_handle;
    value_snapshot(logical_index?: number): chart_value_snapshot[];
    set_comparison_anchor(time: number | null): boolean;
    comparison_anchor(): number | null;
    comparison_legend_snapshot(): comparison_legend_entry[];
    /** Internal package hook used by the singleton accessibility controller. */
    set_accessibility_handle(handle: accessibility_handle | null): void;
    alert_create_hit_at(x: number, y: number): boolean;
    activate_alert_create_at(x: number, y: number): boolean;
    private dispatch_alert_create_requests;
    trading_hit_at(x: number, y: number): trading_hit | null;
    trading_hit_at_device(x: number, y: number, device: number): trading_hit | null;
    trading_cursor_at(x: number, y: number): string | null;
    clear_trading_hover(): boolean;
    trading_pressed_at(x: number, y: number): boolean;
    trading_pressed_at_device(x: number, y: number, device: number): boolean;
    clear_trading_pressed(): boolean;
    deactivate_trading_group(): boolean;
    trading_drag_start_at(x: number, y: number): boolean;
    trading_drag_start_at_device(x: number, y: number, device: number): boolean;
    trading_keyboard_start_order(id: string): boolean;
    trading_keyboard_adjust(ticks: number): boolean;
    trading_keyboard_commit(): void;
    trading_keyboard_cancel(): boolean;
    trading_drag_to(y: number): boolean;
    trading_drag_end(): void;
    cancel_trading_drag(): void;
    discard_trading_interaction(): boolean;
    trading_activate_at(x: number, y: number): boolean;
    add_delta_tooltip_range_listener(listener: () => void): () => void;
    private notify_delta_tooltip_ranges;
    /**
     * Cached "any live series has countdown_visible" flag (refreshed by `sync_countdown_timer`)
     * so streaming `update` calls can cheaply decide whether data-arrival may start the timer.
     */
    countdown_series_present: boolean;
    /** The Phase C-e plugin overlay canvas and its 2D context (package-owned host DOM). */
    private readonly plugin_ctx;
    private readonly canvas_primitives;
    private plugin_resize_observer;
    private backend_loss_count;
    private attribution_required;
    private attribution;
    private attribution_left;
    private attribution_bottom;
    private readonly backend_loss_handler;
    private readonly font_loading_done_handler;
    constructor(wasm: AerisChart, container: HTMLElement, gpu_pane: HTMLCanvasElement, fallback_pane: HTMLCanvasElement, plugin_canvas: HTMLCanvasElement, overlay: HTMLCanvasElement, auto_size: boolean, selected_theme: theme_name | undefined, initial_general: boolean | undefined, host_snapshot: chart_host_snapshot);
    /** Internal package boundary. Every post-disposal operation fails with one stable error. */
    get wasm(): AerisChart;
    /** Deterministic browser-test hook; intentionally absent from `chart_api`. */
    backend_loss_count_for_test(): number;
    /**
     * Coalesce renders onto the next animation frame. The streaming hot path
     * (series `update` on built-in series) must not pay a full render per tick: N calls
     * inside one frame produce exactly one repaint, which is what the engine's
     * invalidate-mask design assumes (reference behavior: mutations invalidate, the
     * frame flush renders). One-shot mutations (set_data, pop, options, gestures,
     * resize, explicit render(), take_screenshot) still repaint synchronously — axis
     * sizing hysteresis and custom-series render-time price computation make their
     * render counts observable. Any synchronous repaint cancels a pending scheduled
     * frame and paints immediately.
     */
    schedule_repaint(): void;
    /**
     * Start or stop the ring-drain frame loop to match the number of bound ring sources.
     *
     * The engine has no clock of its own — the package owns the frame loop — so a bound ring needs
     * something to tick it. While at least one ring is bound this runs one `requestAnimationFrame`
     * loop for the whole chart (not per series), and it stops as soon as the last ring unbinds, so a
     * chart with no rings pays nothing.
     *
     * Called on every bind/unbind rather than tracking a count: `ring_source_count()` is the engine's
     * own answer, which cannot drift from it.
     */
    sync_ring_drain_loop(): void;
    /**
     * One frame's ring drain. This is the whole per-frame cost of a ring source when the producer is
     * idle: one engine call that does an atomic load per ring. A repaint and `data_changed` only
     * happen when rows actually arrived, so a bound-but-quiet ring does not force the chart to
     * re-render at frame rate.
     */
    private readonly ring_tick;
    /** Repaint unless torn down. Named distinctly from the public `render` for internal use. */
    repaint(): void;
    /**
     * Require the Aeris Terminal attribution logo in the bottom-left corner of the bottom pane while
     * the chart has a financial pane. NOTICE section 7(b) requires it for AGPL use of financial
     * charts; `create_chart` skips it for commercial license keys. General-only charts carry no logo.
     */
    show_attribution(): void;
    private has_financial_pane;
    /** Panes can switch between financial and general content, so the logo follows every frame. */
    private sync_attribution;
    private create_attribution;
    private sync_attribution_theme;
    /** Track the engine's bottom content corner so the logo never covers axis labels. */
    private place_attribution;
    /**
     * Canvas primitives (plugin platform Phase C-e): after the engine frame, paint every
     * attached canvas primitive onto the package-owned DOM canvas. The canvas remains above pane
     * content for plugin z-order, but each renderer is clipped to its owning engine pane so it cannot
     * cover the shared price/time-axis chrome now rendered by WebGPU/Canvas2D.
     */
    private run_canvas_primitives;
    /** Register a canvas primitive (Phase C-e) on behalf of `pane_impl`. */
    attach_canvas_primitive(pane_index: number, primitive: canvas_primitive): canvas_primitive_handle;
    /** Drop a canvas primitive; the repaint clears its paint (the pass re-clears the canvas). */
    detach_canvas_primitive(entry: canvas_primitive_entry): void;
    private read_visible_logical_range;
    private read_visible_time_range;
    private emit_visible_range_changes;
    /** Enable fixed-clock value transitions for compatible general dataset updates. */
    set_general_transition_duration(duration_ms: number): void;
    private sync_general_motion_listener;
    /** Pin the shared engine clock before a general data mutation so its first frame starts at zero progress. */
    prepare_general_transition_update(): void;
    /** Start or stop the animation rAF loop while the engine has active animated content. */
    sync_animation(): void;
    /**
     * Start or stop the 1s candle-close countdown timer to match whether any live series has
     * `countdown_visible` and data (industry-standard countdown row in the last-value cluster).
     * Central rebuild point: called from the series apply-options/set-data/remove paths and on
     * chart teardown. Ticks pin the engine clock and repaint; ticks are skipped while the
     * document is hidden.
     */
    sync_countdown_timer(): void;
    /**
     * Arm the next countdown tick 1 ms past the next wall-clock second. The label shows the floor
     * of the remaining seconds, so it changes exactly when `now` crosses a whole second; a free
     * running `setInterval` with an arbitrary phase lets jitter land two ticks in one second,
     * holding a value for two seconds and then skipping one.
     */
    private schedule_countdown_tick;
    private stop_countdown_timer;
    private start_animation;
    private stop_animation;
    add_trade_stream(key: string, options?: Partial<footprint_series_options>): number;
    add_depth_stream(key: string, options?: Partial<depth_options>): number;
    depth_stream_id(key: string): number | null;
    remove_depth_stream(stream_id: number): boolean;
    set_depth_snapshot_typed(stream_id: number, columns: depth_snapshot_columns): void;
    update_depth_typed(stream_id: number, columns: depth_update_columns): void;
    depth_ladder(stream_id: number, levels_per_side: number, minimum_size?: number, max_distance_ticks?: number): readonly depth_ladder_row[] | null;
    depth_study(stream_id: number, levels_per_side: number, minimum_size?: number, max_distance_ticks?: number): depth_study_snapshot | null;
    add_depth_heatmap(stream_id: number, options: Partial<depth_heatmap_options> & Pick<depth_heatmap_options, "price_min" | "price_max">): number;
    remove_depth_heatmap(id: number): boolean;
    set_depth_events_typed(stream_id: number, columns: depth_event_columns): void;
    add_depth_event_layer(stream_id: number, options?: Partial<depth_event_layer_options>): number;
    remove_depth_event_layer(id: number): boolean;
    add_depth_profile(stream_id: number, options?: Partial<depth_profile_options>): number;
    set_depth_profile_options(id: number, options: Partial<depth_profile_options>): boolean;
    remove_depth_profile(id: number): boolean;
    selected_depth_profile(): number | null;
    select_depth_profile(id: number | null): boolean;
    time_and_sales(stream_id: number, options?: Partial<time_and_sales_options>): readonly time_and_sales_row[] | null;
    replay_clock_micros(): number | null;
    set_replay_clock_micros(clock_micros: number | null): replay_clock_stats;
    replay_state(): replay_state;
    replay_select_start(): replay_state;
    replay_cancel_selection(): replay_state;
    replay_select_time(time_micros: number): replay_state;
    replay_play(): replay_state;
    replay_pause(): replay_state;
    replay_toggle_play(): replay_state;
    replay_step_forward(): replay_state;
    replay_step_back(): replay_state;
    set_replay_speed(steps_per_second: number): replay_state;
    set_replay_step(step: replay_step): replay_state;
    replay_jump_to_realtime(): replay_state;
    replay_exit(): replay_state;
    subscribe_replay_change(handler: replay_change_handler): void;
    unsubscribe_replay_change(handler: replay_change_handler): void;
    private replay_command;
    configure_synthetic_bar_series(series: series_api | number, options: synthetic_bar_options): void;
    set_synthetic_bar_source(series: series_api | number, data: readonly series_data[]): void;
    set_synthetic_bar_source_typed(series: series_api | number, columns: ohlc_columns): void;
    update_synthetic_bar_source(series: series_api | number, data: ohlc_data): void;
    synthetic_bar_at(series: series_api | number, logical: number): synthetic_bar | null;
    trade_stream_id(key: string): number | null;
    trade_stream_revision(stream_id: number): number | null;
    trade_stream_stats(stream_id: number): trade_stream_stats | null;
    trade_stream_replay_clock_micros(stream_id: number): number | null;
    set_trade_stream_replay_clock_micros(stream_id: number, clock_micros: number | null): replay_seek_stats;
    set_trade_stream_trades(stream_id: number, trades: readonly footprint_trade[]): void;
    set_trade_stream_trades_typed(stream_id: number, columns: footprint_trade_columns): void;
    update_trade_stream_trades(stream_id: number, trades: readonly footprint_trade[]): "tip" | "historical";
    update_trade_stream_trades_typed(stream_id: number, columns: footprint_trade_columns): "tip" | "historical";
    bind_footprint_series_to_stream(series: footprint_series_api | number, stream_id: number): void;
    bind_trade_bar_series_to_stream(series: series_api | number, stream_id: number): void;
    add_cvd_series(stream_id: number, pane?: number, reset?: "session" | "continuous" | "anchored", anchor_timestamp_micros?: number, style?: "line" | "candles"): series_api;
    add_delta_series(stream_id: number, pane?: number): series_api;
    add_big_trades(series: series_api | number, stream_id: number, options?: Partial<big_trades_options>): big_trades_api;
    add_auction_markers(series: series_api | number, stream_id: number, options?: Partial<auction_marker_options>): auction_markers_api;
    add_series(kind: "footprint", options?: Partial<any_series_options> & Partial<footprint_series_options>): footprint_series_api;
    add_series(kind: "tpo", options?: Partial<any_series_options> & Partial<tpo_series_options>): tpo_series_api;
    add_series(kind: general_series_kind, options: general_series_options): general_series_api;
    add_series(kind: series_kind, options?: Partial<any_series_options>): series_api;
    add_custom_series(pane_view: custom_series_pane_view, options?: Partial<series_options>): series_api;
    remove_series(series: series_api | general_series_api): void;
    series_order(): series_api[];
    /**
     * The live handle for an engine series id: the registered handle when one exists, otherwise a
     * fresh handle adopted into the registry (the way `panes()`/`price_scale()` build handles on
     * demand). Backs `pane_api.get_series`.
     */
    series_handle(id: number): series_impl;
    general_series_handle(id: number): general_series_impl | null;
    general_series_order(pane?: number): general_series_api[];
    set_general_series_order(ordered: general_series_api[], pane?: number): boolean;
    set_series_order(ordered: series_api[]): boolean;
    private indicator_series;
    private drain_custom_study_faults;
    private protect_chart_calls_in_custom_callbacks;
    register_custom_study<State>(definition: custom_study_definition<State>): void;
    add_custom_study(type: string, source: series_api, params?: Record<string, unknown>, options?: custom_study_binding_options): series_api[];
    subscribe_custom_study_fault(callback: (event: custom_study_fault_event) => void): () => void;
    configure_resampled_series(source: series_api, target: series_api, options: resample_options, volume_source?: series_api | null, volume_target?: series_api | null): void;
    set_study_calendar(boundaries: readonly resample_boundary[]): void;
    clear_study_calendar(): void;
    study_annotations(binding: series_api): study_annotations;
    resampled_bars(target: series_api): readonly resampled_bar[] | null;
    volume_profile_snapshot(request: profile_request): profile_snapshot | null;
    periodic_volume_profiles(source: profile_source, boundaries: readonly resample_boundary[], tick_size: number, row_count: number, value_area_percent: number): readonly profile_snapshot[] | null;
    periodic_naked_profile_levels(source: profile_source, boundaries: readonly resample_boundary[], tick_size: number, row_count: number, value_area_percent: number): readonly naked_profile_level[] | null;
    tpo_profiles(request: tpo_request): readonly tpo_snapshot[] | null;
    add_periodic_profile_presentation(anchor: series_api, request: periodic_profile_presentation_request, options?: Partial<periodic_profile_presentation_options>): number;
    remove_periodic_profile_presentation(id: number): boolean;
    anchored_vwap(source: profile_source, start_timestamp_micros: number, end_timestamp_micros: number, band_multiplier: number): readonly anchored_vwap_point[] | null;
    set_series_volume_source(price_series: series_api, volume_series: series_api | null): void;
    configure_profile_drawing(drawing: drawing_api, options: profile_drawing_options): void;
    profile_drawing_snapshot(drawing: drawing_api): profile_drawing_snapshot | null;
    add_sma(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_sma_with_source(source: series_api, input: indicator_input_source, period: number, options?: Partial<series_options>): series_api;
    add_ema(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_dema(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_tema(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_smma(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_rma(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_hma(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_bollinger(source: series_api, period: number, deviation?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_bollinger_with_source(source: series_api, input: indicator_input_source, period: number, deviation?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_rsi(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_rsi_with_source(source: series_api, input: indicator_input_source, period: number, options?: Partial<series_options>): series_api;
    set_indicator_input_source(indicator: series_api, input: indicator_input_source): boolean;
    indicator_schema(kind: indicator_kind, period?: number, deviation?: number): indicator_schema;
    add_macd(source: series_api, fast: number, slow: number, signal: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_stochastic(source: series_api, k_period: number, d_period: number, options?: Partial<series_options>): [series_api, series_api];
    add_atr(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_vwap(source: series_api, volume_source?: series_api | null, options?: Partial<series_options>): series_api;
    add_obv(source: series_api, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_accumulation_distribution(source: series_api, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_price_volume_trend(source: series_api, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_chaikin_oscillator(source: series_api, fast: number, slow: number, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_klinger(source: series_api, fast: number | undefined, slow: number | undefined, signal: number | undefined, volume_source: series_api, options?: Partial<series_options>): [series_api, series_api];
    add_kama(source: series_api, period?: number, fast?: number, slow?: number, options?: Partial<series_options>): series_api;
    add_mcginley(source: series_api, period?: number, options?: Partial<series_options>): series_api;
    add_linear_regression(source: series_api, period?: number, deviation?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_choppiness(source: series_api, period?: number, options?: Partial<series_options>): series_api;
    add_atr_bands(source: series_api, period?: number, multiplier?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_relative_volume(source: series_api, period: number, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_volume_oscillator(source: series_api, fast: number, slow: number, signal: number, volume_source: series_api, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_elder_force(source: series_api, period: number, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_ease_of_movement(source: series_api, period: number, volume_source: series_api, divisor?: number, options?: Partial<series_options>): series_api;
    add_historical_volatility(source: series_api, period: number, annualization?: number, options?: Partial<series_options>): series_api;
    add_trix(source: series_api, period: number, signal?: number, options?: Partial<series_options>): [series_api, series_api];
    add_kst(source: series_api, roc?: kst_periods, smoothing?: kst_periods, signal?: number, options?: Partial<series_options>): [series_api, series_api];
    add_tsi(source: series_api, long?: number, short?: number, signal?: number, options?: Partial<series_options>): [series_api, series_api];
    add_mass_index(source: series_api, ema_period?: number, sum_period?: number, options?: Partial<series_options>): series_api;
    add_vortex(source: series_api, period?: number, options?: Partial<series_options>): [series_api, series_api];
    add_coppock_curve(source: series_api, long_period?: number, short_period?: number, smoothing?: number, options?: Partial<series_options>): series_api;
    add_fisher_transform(source: series_api, period?: number, options?: Partial<series_options>): [series_api, series_api];
    add_ultimate_oscillator(source: series_api, short_period?: number, medium_period?: number, long_period?: number, options?: Partial<series_options>): series_api;
    add_cmf(source: series_api, period: number, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_mfi(source: series_api, period: number, volume_source: series_api, options?: Partial<series_options>): series_api;
    add_volume(source: series_api, period: number, volume_source: series_api, options?: Partial<series_options>): [series_api, series_api];
    add_vwap_bands(source: series_api, reset?: vwap_reset, standard_deviation?: number, percent?: number, volume_source?: series_api | null, options?: Partial<series_options>): [series_api, series_api, series_api, series_api, series_api];
    add_volume_profile(source: series_api, volume_source: series_api, options?: Partial<volume_profile_indicator_options>): volume_profile_indicator_api;
    add_options_volume(rows?: readonly options_volume_row[]): options_volume_api;
    add_wma(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_vwma(source: series_api, period: number, volume_source?: series_api | null, options?: Partial<series_options>): series_api;
    add_standard_deviation(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_cci(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_williams_r(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_stochastic_rsi(source: series_api, rsi_period: number, stochastic_period: number, options?: Partial<series_options>): series_api;
    add_momentum(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_roc(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_donchian(source: series_api, period: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_pivot_points(source: series_api, variant?: pivot_kind, options?: Partial<series_options>): [series_api, series_api, series_api, series_api, series_api];
    add_zigzag(source: series_api, deviation_percent?: number, options?: Partial<series_options>): series_api;
    add_keltner(source: series_api, period: number, multiplier?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_adx_dmi(source: series_api, period: number, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_parabolic_sar(source: series_api, options?: Partial<series_options>): series_api;
    add_supertrend(source: series_api, period: number, multiplier?: number, options?: Partial<series_options>): series_api;
    add_ichimoku(source: series_api, options?: Partial<series_options>): [series_api, series_api, series_api, series_api, series_api];
    subscribe_crosshair_move(handler: mouse_event_handler): void;
    unsubscribe_crosshair_move(handler: mouse_event_handler): void;
    subscribe_click(handler: mouse_event_handler): void;
    unsubscribe_click(handler: mouse_event_handler): void;
    subscribe_chart_context(handler: chart_context_handler): void;
    unsubscribe_chart_context(handler: chart_context_handler): void;
    subscribe_dbl_click(handler: dbl_click_handler): void;
    unsubscribe_dbl_click(handler: dbl_click_handler): void;
    subscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
    unsubscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
    subscribe_visible_time_range_change(handler: visible_time_range_handler): void;
    unsubscribe_visible_time_range_change(handler: visible_time_range_handler): void;
    subscribe_size_change(handler: size_change_handler): void;
    unsubscribe_size_change(handler: size_change_handler): void;
    subscribe_series_added(handler: series_change_handler): void;
    unsubscribe_series_added(handler: series_change_handler): void;
    subscribe_series_removed(handler: series_change_handler): void;
    unsubscribe_series_removed(handler: series_change_handler): void;
    subscribe_options_change(handler: options_change_handler): void;
    unsubscribe_options_change(handler: options_change_handler): void;
    /** A series' current pane index (0 fallback for a tombstoned/unknown id). */
    private pane_of_series;
    /** Notify series-lifecycle subscribers (added and removed share the payload shape). */
    private emit_series_change;
    /** Invalidate any in-flight animated `scroll_to_position` (user gestures take over scrolling). */
    cancel_scroll_animation(): void;
    /**
     * Which pane contains the pane-relative CSS point (x, y), or `null` when it falls on an axis
     * strip (price/time) or outside the pane area. Backs `mouse_event_params.pane_index`.
     */
    pane_index_at(x: number, y: number): number | null;
    /** Build event params for a cursor at (x, y) in pane CSS px. */
    private build_params;
    /**
     * Read the engine-arbitrated hover (Phase C-d) after pointer input and stash it for
     * `build_params`. The engine already updated hover promotion, the hover ring, and the cursor;
     * the gesture recognizer repaints only when the engine reports a pending frame.
     */
    sync_hover(): void;
    /** Clear the hover state (cursor left the chart) and release the z-bump. */
    clear_hover(): void;
    /** The cursor a primitive's `hit_test` reports for the current hover, or `null`. */
    hover_cursor(): string | null;
    /** The engine-hit series under the current hover, or `null` (drives the pointer cursor). */
    hover_series_id(): number | null;
    /** Emit a crosshair-move event (called by the gesture recognizer). */
    emit_crosshair(x: number, y: number): void;
    /** Emit the "cursor left the chart" crosshair event (empty params). */
    emit_crosshair_left(): void;
    remove_general_series_handle(series: general_series_impl): void;
    /** The controller has already applied selection and editing for this pane click. */
    emit_controller_click(x: number, y: number): void;
    /** Open only the DOM input surface for the engine's active text-edit session. */
    open_controller_text_editor(id: number): void;
    /** Emit engine-resolved context without running primary-click selection or activation paths. */
    emit_chart_context(x: number, y: number): boolean;
    /** Announce the current visible time range to assistive tech (used after keyboard navigation). */
    announce_view(): void;
    emit_dbl_click(x: number, y: number): void;
    add_drawing(kind: drawing_kind, points: drawing_point[], options?: Partial<drawing_options>, pane_index?: number): drawing_api;
    drawings(): drawing_api[];
    drawing_property_schema(drawing: drawing_api | number): drawing_property_schema;
    drawing_painted_colors(drawing: drawing_api | number): Record<string, string>;
    drawing_kind_options(drawing: drawing_api | number): drawing_kind_options;
    add_aroon(source: series_api, period: number, options?: Partial<series_options>): [series_api, series_api];
    private validate_structure_pivots;
    add_swing_points(source: series_api, left?: number, right?: number, options?: Partial<series_options>): [series_api, series_api];
    private validate_study_calendar_policy;
    add_session_levels(source: series_api, calendar?: study_calendar_policy, options?: Partial<series_options>): [series_api, series_api];
    add_previous_period_levels(source: series_api, period?: previous_period, calendar?: study_calendar_policy, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_opening_range(source: series_api, duration_seconds: number, calendar?: study_calendar_policy, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_awesome_oscillator(source: series_api, options?: Partial<series_options>): series_api;
    add_dpo(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_chande_momentum(source: series_api, period: number, options?: Partial<series_options>): series_api;
    add_bollinger_metrics(source: series_api, period: number, deviation: number, options?: Partial<series_options>): [series_api, series_api];
    add_envelopes(source: series_api, period: number, percent: number, exponential?: boolean, options?: Partial<series_options>): [series_api, series_api, series_api];
    add_alma(source: series_api, period: number, offset?: number, sigma?: number, options?: Partial<series_options>): series_api;
    register_drawing_icon(name: string, width: number, height: number, pixels: Uint8Array): void;
    remove_drawing_icon(name: string): boolean;
    builtin_drawing_icons(): builtin_drawing_icon[];
    drawing_object_tree(): unknown[];
    set_drawing_interval(interval: drawing_interval | null): void;
    copy_drawings(ids?: readonly number[]): string;
    paste_drawings(payload: string, pane_index?: number, logical_offset?: number, price_offset?: number): drawing_api[];
    clone_drawing(drawing: drawing_api | number, logical_offset?: number, price_offset?: number): drawing_api;
    move_drawing_z_order(drawing: drawing_api | number, delta: number): boolean;
    set_drawing_group_visibility(group_id: string, visible: boolean): number;
    set_drawing_group_locked(group_id: string, locked: boolean): number;
    set_drawings_hidden(hidden: boolean): boolean;
    drawings_hidden(): boolean;
    show_hidden_drawings(): number;
    hidden_drawing_count(): number;
    move_drawing_group(group_id: string, logical_delta: number, price_delta: number): number;
    apply_drawing_template(drawing: drawing_api | number, template: drawing_template): void;
    drawing_template(drawing: drawing_api | number, name: string): drawing_template;
    drawing_sync_payload(source: string): string;
    apply_drawing_sync_payload(payload: string): boolean;
    export_state(): chart_state;
    import_state(state: chart_state | string): persistence_restore_result;
    clear_drawings(): void;
    announce_trading_intent(intent: trading_intent): void;
    undo_drawing(): boolean;
    redo_drawing(): boolean;
    can_undo_drawing(): boolean;
    can_redo_drawing(): boolean;
    set_drawing_tool(tool: drawing_kind | null, options?: Partial<drawing_options>, pane_index?: number): void;
    private notify_drawing_tool;
    /** Report an engine-side disarm (one-shot commit, Escape, secondary click) to listeners. */
    private sync_drawing_tool;
    active_drawing_tool(): drawing_kind | null;
    set_drawing_tool_listener(listener: ((tool: drawing_kind | null) => void) | null): void;
    subscribe_drawing_tool_change(handler: drawing_tool_change_handler): void;
    unsubscribe_drawing_tool_change(handler: drawing_tool_change_handler): void;
    subscribe_drawing_created(handler: drawing_created_handler): void;
    unsubscribe_drawing_created(handler: drawing_created_handler): void;
    selected_drawing(): drawing_api | null;
    selected_drawings(): drawing_api[];
    set_selected_drawings(drawings: readonly (drawing_api | number)[]): void;
    remove_selected_drawings(): number;
    apply_selected_drawings_options(options: Partial<drawing_options>): number;
    set_selected_drawings_locked(locked: boolean): number;
    set_selected_drawings_visibility(visible: boolean): number;
    clone_selected_drawings(logical_offset?: number, price_offset?: number): drawing_api[];
    private drawing_handles;
    /** Whether an interactive drawing tool is armed (the recognizer routes pane clicks to creation). */
    creation_armed(): boolean;
    /** Whether an interactive creation is mid-placement/capture in the engine controller. */
    creation_active(): boolean;
    /** Whether the current creation owns a captured pointer stream (freehand today, extensible). */
    creation_capture_active(): boolean;
    /** Whether the armed placement is a variable sequence requiring explicit finish. */
    creation_sequence_active(): boolean;
    private drawing_created;
    /** Forward an armed-tool pointer press. Returns true only when placement committed on press. */
    creation_pointer_down(x: number, y: number, magnet?: boolean, straighten?: boolean): boolean;
    /** Forward an armed-tool pointer move; `pressed` identifies an active captured drag stream. */
    creation_pointer_move(x: number, y: number, magnet?: boolean, straighten?: boolean, pressed?: boolean): boolean;
    /** Forward pointer release; returns true when the release committed a drawing. */
    creation_pointer_up(x: number, y: number, magnet?: boolean, straighten?: boolean): boolean;
    /** Route one click/tap activation into the canonical placement state machine. */
    creation_click(x: number, y: number, magnet?: boolean, straighten?: boolean): boolean;
    /** Commit any active variable-sequence drawing (double-click/Enter). */
    creation_finish(): boolean;
    /** Remove the latest placed vertex from an active variable-sequence drawing. */
    creation_pop_anchor(): boolean;
    /** Abort an interrupted pointer capture while leaving the selected tool armed. */
    cancel_active_drawing_creation(): void;
    /** Escape: disarm the tool (cancelling any pending creation) and deselect any drawing. */
    cancel_drawing_interaction(): void;
    /**
     * Open the typing-mode editor for a text drawing (the public reference's overlay-caret model): a
     * borderless wrap whose glyphs are TRANSPARENT — the engine keeps painting both the label
     * and the focus border underneath, so entering edit cannot lift the text or shift the
     * outline. Each keystroke pushes the text into the engine live; Shift+Enter breaks the line,
     * Enter/blur commits, Escape restores the pre-edit text. Leaving empty removes the drawing.
     */
    open_text_editor(drawing: drawing_api): void;
    private open_inline_editor;
    /**
     * Close the typing-mode editor. Commit keeps the live-pushed text (or removes if empty);
     * cancel restores the pre-edit snapshot. Empty result removes the drawing.
     */
    close_text_editor(commit: boolean): void;
    apply_options(options: deep_partial<chart_options>): void;
    /** reference `autoSize`: the engine owns container sizing only while this option is active. */
    private set_auto_size;
    /**
     * Forward a key pressed on an accessibility focus target (`price-axis:<pane>`, `time-axis`,
     * `separator:<pane>`, `drawing:<id>`, `general-row:<series>:<row>`,
     * `layout-item:<series>:<element>:<row>`) to the engine input
     * controller. Returns whether it was
     * consumed.
     */
    input_target_key(target: string, event: KeyboardEvent): boolean;
    /** The open keyboard drawing edit: the drawing and its focused anchor (`null` moves it all). */
    drawing_edit_session(): {
        id: number;
        anchor: number | null;
    } | null;
    /** DPR-only display transitions do not reliably resize CSS bounds on every WebKit host. */
    private bind_dpr_watcher;
    private unbind_dpr_watcher;
    auto_size_active(): boolean;
    chart_element(): HTMLElement;
    format_time_label(value: time): string;
    /** Tooltip colors for the theme the engine paints, so DOM and engine tooltips always agree. */
    popover_palette(): popover_palette;
    /** Install the host price/time formatters (reference `localization`). Callbacks cross into wasm. */
    apply_localization(loc: localization_options): void;
    /** Resolve and store the gesture toggles; the recognizer reads the result live. */
    apply_gesture_options(scroll?: boolean | handle_scroll_options, scale?: boolean | handle_scale_options, kinetic?: boolean | kinetic_scroll_options, tracking?: tracking_mode_options): void;
    private sync_touch_action;
    /** Resolve the reference `layout.panes.enableResize` toggle (separator drag + hover cursor). */
    apply_panes_resize(enabled: boolean): void;
    /** Mirror the engine's master interaction switch: off only when every scroll+scale flag is. */
    private sync_interaction_disabled;
    /** Update the aria-live region with a compact description of the point under the cursor. */
    announce(x: number, y: number): void;
    /** Whether the user has requested reduced motion (gates kinetic scroll). */
    /** Read on every pointer move, so the live `MediaQueryList` is created once and reused. */
    prefers_reduced_motion(): boolean;
    /** Current resolved gesture toggles (read by the gesture recognizer). */
    gesture_config(): resolved_gestures;
    has_chart_context_subscribers(): boolean;
    /** Perform platform effects requested by the bounded engine input controller. */
    consume_input_events(complete_press?: boolean): void;
    options(): unknown;
    /**
     * Hide the engine-drawn general tooltip while at least one DOM replacement is attached, then
     * restore the setting the first one found. A no-op after `remove()`, so detaching a component
     * after its chart is safe.
     */
    retain_dom_general_tooltip(retain: boolean): void;
    private set_engine_general_tooltip;
    backend(): "webgpu" | "canvas2d";
    backend_status(): Readonly<backend_status>;
    /**
     * Read the engine's last-frame record into the per-chart scratch array and shape it as
     * `frame_stats`. The scratch array is the whole point: the engine writes f64 slots in place,
     * so a host polling this every frame for minutes never grows the JS heap and never triggers a
     * GC pause that would itself corrupt the measurement.
     */
    frame_stats(): frame_stats;
    time_scale(): time_scale_api;
    price_scale(price_scale_id?: string, pane_index?: number): price_scale_api;
    add_price_scale(options: price_scale_create_options, pane_index?: number): price_scale_api;
    price_scales(pane_index?: number): price_scale_info[];
    move_price_scale(id: string, side: "left" | "right", order: number, pane_index?: number): void;
    remove_price_scale(id: string, pane_index?: number): void;
    panes(): pane_api[];
    add_pane(options?: boolean | general_pane_options): pane_api;
    add_axis(options: general_axis_options): general_axis_api;
    add_layout_series(kind: layout_series_kind, options: layout_series_options, data: layout_input): layout_series_api;
    layout_series(pane?: number): layout_series_api[];
    private layout_series_handle;
    layout_hit_test(pane: number, x: number, y: number): layout_hit | null;
    layout_selected_hit(): layout_hit | null;
    set_layout_accessibility_focus(hit: Pick<layout_hit, "series" | "element" | "row">, color: string, size: number): boolean;
    clear_layout_accessibility_focus(): void;
    validate_general_pane_proposal(pane: number, axes: readonly (Omit<general_axis_options, "pane"> & {
        pane?: number;
    })[], series: readonly {
        kind: general_series_kind;
        options: Omit<general_series_options, "pane"> & {
            pane?: number;
        };
        data: readonly (general_xy_row | bubble_row | range_area_row | error_bar_row | box_plot_row | waterfall_row | bullet_row | heatmap_grid_row | histogram_row | matrix_row)[];
    }[], references: readonly general_reference_options[]): void;
    compose_general_pane(composition: general_pane_composition): general_pane_composition_result;
    axis(id: string): general_axis_api | null;
    axes(pane?: number): general_axis_api[];
    remove_axis(id: string): boolean;
    add_general_reference(options: general_reference_options): general_reference_api;
    general_references(pane?: number): general_reference_api[];
    general_legend_snapshot(pane?: number): general_legend_snapshot;
    subscribe_general_legend_change(handler: () => void): void;
    unsubscribe_general_legend_change(handler: () => void): void;
    notify_general_legend_change(): void;
    general_shared_tooltip(series: number | general_series_api, row: number): general_shared_tooltip_snapshot | null;
    set_general_brush(axis: string | general_axis_api, from_coordinate: number, to_coordinate: number): general_brush_snapshot;
    set_general_brush_range(axis: string | general_axis_api, range: general_brush_range): general_brush_snapshot;
    set_general_brush_input_axis(axis?: string | general_axis_api): void;
    select_full_general_brush(axis: string | general_axis_api): general_brush_snapshot;
    nudge_general_brush(endpoint: "start" | "end", direction: -1 | 1): general_brush_snapshot;
    general_brush_snapshot(): general_brush_snapshot | null;
    clear_general_brush(axis?: string | general_axis_api): void;
    subscribe_general_brush_change(handler: () => void): void;
    unsubscribe_general_brush_change(handler: () => void): void;
    private notify_general_brush_change;
    general_hit_test(pane: number, x: number, y: number, max_distance?: number): general_series_hit | null;
    general_selected_hit(): general_series_hit | null;
    pane_is_financial_time(pane_index: number): boolean;
    general_accessibility_focused_hit(): general_series_hit | null;
    set_general_accessibility_focus(series: number, row: number): boolean;
    clear_general_accessibility_focus(): void;
    remove_pane(index: number): boolean;
    swap_panes(first: number, second: number): boolean;
    reset_style_to_defaults(): void;
    reset_view(): void;
    price_to_coordinate(price: number): number | null;
    coordinate_to_price(y: number): number | null;
    set_crosshair_position(price: number, time: time, series: series_api): void;
    clear_crosshair_position(): void;
    crosshair_sync_position(): crosshair_sync_position | null;
    apply_external_crosshair(position: crosshair_sync_position | null): void;
    set_sync_price_key(key: string | null): void;
    take_sync_events(): chart_sync_event[];
    apply_external_sync_event(event: chart_sync_event): boolean;
    resize(width: number, height: number, dpr?: number): void;
    private apply_size;
    render(): void;
    take_screenshot(add_top_layer?: boolean, include_crosshair?: boolean): HTMLCanvasElement;
    overlay_el(): HTMLCanvasElement;
    remove(): void;
}
export {};
