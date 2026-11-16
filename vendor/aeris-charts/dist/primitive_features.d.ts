/** Primitive and compatibility helpers built on Aeris's existing engine and extension boundaries. */
import type { chart_api, drawing_api, drawing_options, drawing_point, mouse_event_params, price_line_api, price_line_options, price_scale_api, series_api, time } from "./types.js";
export interface detachable_feature {
    detach(): void;
}
export interface anchored_text_options {
    vert_align?: "top" | "middle" | "bottom";
    /** Official plugin spelling; `vert_align` remains the Aeris alias. */
    vertAlign?: "top" | "middle" | "bottom";
    horz_align?: "left" | "middle" | "right";
    /** Official plugin spelling; `horz_align` remains the Aeris alias. */
    horzAlign?: "left" | "middle" | "right";
    text: string;
    line_height?: number;
    /** Official plugin spelling; `line_height` remains the Aeris alias. */
    lineHeight?: number;
    /** CSS font shorthand containing a pixel size, for example `italic bold 54px Arial`. */
    font?: string;
    color?: string;
}
export interface anchored_text_api extends detachable_feature {
    apply_options(options: Partial<anchored_text_options>): void;
}
/** Official viewport-anchored text primitive, rendered by the shared Rust frame. */
export declare function create_anchored_text(series: series_api, options: anchored_text_options): anchored_text_api;
export interface bands_indicator_options {
    line_color?: string;
    fill_color?: string;
    line_width?: number;
}
export interface bands_indicator_api extends detachable_feature {
    apply_options(options: Partial<bands_indicator_options>): void;
}
/** Series primitive for ±10% source-price bands; distinct from the built-in indicator catalog. */
export declare function create_bands_indicator(source: series_api, options?: bands_indicator_options): bands_indicator_api;
export interface rectangle_drawing_tool_options extends Partial<drawing_options> {
    fill_color?: string;
    preview_fill_color?: string;
    label_color?: string;
    /** Omit to select black or white from the effective label background. */
    label_text_color?: string;
    show_labels?: boolean;
}
export interface rectangle_drawing_tool_api {
    start_drawing(): void;
    stop_drawing(): void;
    is_drawing(): boolean;
    apply_options(options: rectangle_drawing_tool_options): void;
    remove(): void;
}
/**
 * Convenience wrapper over the canonical `drawing_kind = "rectangle"` object. This does not define
 * a second rectangle implementation; pane/axis geometry and lifecycle remain engine-owned.
 */
export declare function create_rectangle_drawing(chart: chart_api, points: [drawing_point, drawing_point], options?: rectangle_drawing_tool_options, pane_index?: number): drawing_api;
/**
 * Convenience controller over the canonical engine rectangle tool. Pointer placement, preview,
 * commit, snapping, selection, dragging, axis geometry, and persistence stay in Rust; this wrapper
 * owns only its optional DOM toolbar and the lifecycle of rectangles it created.
 */
export declare function create_rectangle_drawing_tool(chart: chart_api, series: series_api, toolbar_container?: HTMLElement, options?: rectangle_drawing_tool_options): rectangle_drawing_tool_api;
export interface trend_line_point {
    time: time;
    price: number;
}
export interface trend_line_options {
    line_color?: string;
    width?: number;
    show_labels?: boolean;
    label_background_color?: string;
    label_text_color?: string;
}
/**
 * Engine-owned series primitive with endpoint labels and autoscale participation. This is distinct
 * from the canonical interactive `drawing_kind = "trend_line"` tool.
 */
export declare function create_trend_line(series: series_api, points: [trend_line_point, trend_line_point], options?: trend_line_options): detachable_feature;
export interface vertical_line_options {
    color?: string;
    label_text?: string;
    width?: number;
    label_background_color?: string;
    /** Omit to select black or white from the effective label background. */
    label_text_color?: string;
    show_label?: boolean;
}
/**
 * Engine-owned series primitive with an optional time-axis label. This is distinct from the
 * canonical interactive `drawing_kind = "vertical_line"` tool.
 */
export declare function create_vertical_line(series: series_api, line_time: time, options?: vertical_line_options): detachable_feature;
/** Engine-owned user price line. */
export declare function create_user_price_line(series: series_api, options: price_line_options): price_line_api;
/** Move a series to the pane's independent overlay scale and return that scale. */
export declare function use_overlay_price_scale(series: series_api): price_scale_api;
export interface overlay_price_scale_options {
    /** Tick-label color. Omit to follow the chart layout text color. */
    text_color?: string;
    side?: "left" | "right";
}
export interface overlay_price_scale_api extends detachable_feature {
    apply_options(options: Partial<overlay_price_scale_options>): void;
    price_scale(): price_scale_api;
}
/** In-pane overlay price-scale text, backed by the attached series' engine scale. */
export declare function create_overlay_price_scale(series: series_api, options?: overlay_price_scale_options): overlay_price_scale_api;
/**
 * Compatibility helper for the now-default native partial live-price line.
 * Prefer `series.apply_options({ price_line_extent: "partial" })` for new code.
 */
export declare function create_partial_price_line(series: series_api): detachable_feature;
export interface session_highlighting_options {
    start_hour_utc?: number;
    end_hour_utc?: number;
    weekday_color?: string;
    weekend_color?: string;
}
/** Official per-source-bar highlighter callback. Returned colors become engine-owned records. */
export type session_highlighter = (date: time) => string;
export declare function create_session_highlighting(series: series_api, highlighter: session_highlighter, options?: session_highlighting_options): detachable_feature;
export declare function create_session_highlighting(series: series_api, options?: session_highlighting_options): detachable_feature;
export interface highlight_bar_crosshair_options {
    color?: string;
}
/** Highlight the complete bar slot under the chart crosshair. */
export declare function create_highlight_bar_crosshair(chart: chart_api, series: series_api, options?: highlight_bar_crosshair_options): detachable_feature;
export interface image_watermark_options {
    max_width?: number;
    /** Official plugin spelling; `max_width` remains the Aeris alias. */
    maxWidth?: number;
    max_height?: number;
    /** Official plugin spelling; `max_height` remains the Aeris alias. */
    maxHeight?: number;
    padding?: number;
    alpha?: number;
}
/** Engine-owned official centered, aspect-preserving image watermark. */
export declare function create_image_watermark(series: series_api, image: CanvasImageSource | string, options?: image_watermark_options): detachable_feature;
export interface tooltip_options {
    series?: series_api;
    /** Optional timestamp-aligned scalar series rendered as the Volume row. */
    volume_series?: series_api;
    class_name?: string;
    title?: string;
    /** Vertical-guide color. Omit to use a contrasting tint for the chart surface. */
    line_color?: string;
    follow_mode?: "top" | "tracking";
    horizontal_deadzone_width?: number;
    vertical_deadzone_height?: number;
    vertical_spacing?: number;
    top_offset?: number;
    format?: (event: mouse_event_params, value: number | null) => string;
}
export interface tooltip_handle extends detachable_feature {
    apply_options(options: Partial<tooltip_options>): void;
}
/** Structured OHLC market-data tooltip; source lookup and its vertical guide are engine-owned. */
export declare function create_tooltip(chart: chart_api, options?: tooltip_options): tooltip_handle;
export interface delta_tooltip_active_range {
    from: number;
    to: number;
    positive: boolean;
}
export interface delta_tooltip_options {
    series: series_api;
    /** Vertical-guide color. Omit to use a contrasting tint for the chart surface. */
    line_color?: string;
    show_time?: boolean;
    top_offset?: number;
    /** Require Shift+primary-drag for mouse/pen activation, preserving ordinary grab-to-pan. */
    requires_shift_drag?: boolean;
    on_active_range_change?: (range: delta_tooltip_active_range | null) => void;
}
export interface delta_tooltip_handle extends detachable_feature {
    active_range(): delta_tooltip_active_range | null;
    /** Clear the committed comparison without detaching the interaction. */
    clear(): void;
}
/**
 * Official one/two-pointer delta tooltip, with all bar lookup and geometry owned by Rust.
 * Candlestick series intentionally reject this interaction; use `create_tooltip` there.
 */
export declare function create_delta_tooltip(chart: chart_api, options: delta_tooltip_options): delta_tooltip_handle;
export interface volume_profile_point {
    price: number;
    vol: number;
}
export interface volume_profile_data {
    time: time;
    profile: readonly volume_profile_point[];
    /** Profile width in time-scale bar slots. */
    width: number;
}
export interface volume_profile_options {
    background_color?: string;
    color?: string;
}
export interface volume_profile_handle extends detachable_feature {
    set_data(data: volume_profile_data): void;
}
/** Official time-anchored price-by-volume profile, binned upstream and rendered by Rust. */
export declare function create_volume_profile(series: series_api, initial_data: volume_profile_data, options?: volume_profile_options): volume_profile_handle;
