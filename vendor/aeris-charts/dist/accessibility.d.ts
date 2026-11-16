/** Chart-owned semantic keyboard and assistive-technology controller. */
import type { chart_api, series_api, series_data, time } from "./types.js";
export interface accessibility_point {
    series_position: number;
    series_count: number;
    point_position: number;
    point_count: number;
    time: string;
    value: string | null;
}
export interface accessibility_messages {
    role_description: string;
    no_value: string;
    no_data: (label: string, scope_note: string) => string;
    in_view: string;
    default_series_label: (position: number) => string;
    pane_label: (title: string, series_count: number, series_label: string | null) => string;
    description: (multi_series: boolean) => string;
    help: (multi_series: boolean, page_step: number) => string;
    shortcuts_hint: string;
    shortcuts_title: string;
    point: (point: accessibility_point, label: string, values: string) => string;
    series_position: (label: string, position: number, total: number, point: string) => string;
    summary: (summary: accessibility_summary) => string;
    series_update: (label: string, count: number, scope_note: string, latest: string) => string;
    data_updated: (summaries: readonly string[], total: number, shown_max: number) => string;
}
export interface accessibility_summary {
    label: string;
    count: number;
    scope_note: string;
    first_value: string;
    first_time: string;
    last_value: string;
    last_time: string;
    direction: "up" | "down" | "unchanged";
    change_value: string;
    percent: string | null;
    low_value: string;
    low_time: string;
    high_value: string;
    high_time: string;
}
export interface accessibility_options {
    chart_title?: string | ((pane_index: number) => string);
    show_focus_indicator?: boolean;
    focus_indicator_color?: string;
    focus_indicator_size?: number;
    announce_data_updates?: boolean | "active" | ((pane_index: number) => boolean);
    page_step?: number;
    data_scope?: "all" | "visible";
    price_formatter?: (value: number) => string;
    time_formatter?: (value: time) => string;
    series_label?: (series: series_api, index: number) => string;
    describe_chart?: (points: readonly series_data[], series_label: string) => string;
    /** Compatibility hook retained from the first Aeris helper. */
    describe_point?: (point: accessibility_point) => string;
    messages?: Partial<accessibility_messages>;
    lang?: string;
    show_shortcuts?: boolean;
    high_contrast?: boolean | "auto" | (() => boolean);
    on_high_contrast_change?: (enabled: boolean) => void;
}
export interface accessibility_handle {
    detach(): void;
    focus(pane_index?: number): void;
    focus_target(target: string, pane_index?: number): void;
    refresh(): void;
    apply_options(options: accessibility_options): void;
    /** Announce a user-triggered chart action through the shared live region. */
    announce(message: string): void;
}
/**
 * Configure and return the chart-owned accessibility singleton. Financial coordinates, mutation,
 * history, and the visible focus ring remain in the shared Rust engine; this controller owns only
 * DOM semantics, keyboard translation, localisation, and browser live-region behavior.
 */
export declare function enable_accessibility(chart: chart_api, options?: accessibility_options): accessibility_handle;
