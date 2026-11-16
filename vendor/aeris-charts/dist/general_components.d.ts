import type { chart_api, general_axis_api, general_series_api, layout_series_api, general_tooltip_snapshot, pane_api } from "./types.js";
export interface general_legend_options {
    pane?: number | pane_api;
    format_title?: (title: string, series: general_series_api) => string;
    format_layout_title?: (title: string, series: layout_series_api) => string;
    label?: string;
    lang?: string;
    format_action?: (visible: boolean, title: string) => string;
}
export interface general_brush_controls_options {
    axis: string | general_axis_api;
    label?: string;
    locale?: string;
    labels?: Partial<{
        select_all: string;
        start_lower: string;
        start_higher: string;
        end_lower: string;
        end_higher: string;
        clear: string;
        no_range: string;
    }>;
}
/** Keyboard and touch controls for the engine's semantic brush and pointer gesture. */
export declare function create_general_brush_controls(chart: chart_api, container: HTMLElement, options: general_brush_controls_options): general_component_handle;
export interface general_component_handle {
    /** Re-read engine state after external style, visibility, data, or view changes. */
    refresh(): void;
    detach(): void;
}
/** Accessible default legend. Its buttons toggle engine visibility by stable series identity. */
export declare function create_general_legend(chart: chart_api, container: HTMLElement, options?: general_legend_options): general_component_handle;
export interface general_tooltip_options {
    mode?: "item" | "shared";
    /** Restrict pointer tooltips to one pane; omitted uses every general or layout pane. */
    pane?: number | pane_api;
    format_value?: (value: number) => string;
    format_detail?: (item: general_tooltip_snapshot) => string | undefined;
    lang?: string;
}
/** Default pointer tooltip driven by engine hits and semantic item/shared snapshots. */
export declare function create_general_tooltip(chart: chart_api, chart_container: HTMLElement, options?: general_tooltip_options): general_component_handle;
