/**
 * Multi-chart split grid (platform feature). The generic split tree is engine-owned and
 * headless-tested; this browser host mirrors it into flex DOM and owns one full `create_chart`
 * instance per leaf cell (its own
 * scales, series, chart type, and drawing primitives — splitting never shares state).
 *
 * Commercial limits and usage metering stay here at the host boundary; the shared chart engine
 * knows nothing about subscriptions, billing, or tiers.
 */
import type { shortcut_action } from "./shortcuts.js";
import type { chart_api, chart_options, chart_state, deep_partial, drawing_kind, drawing_options } from "./types.js";
/** `horizontal` places the two charts side by side; `vertical` stacks them. */
export type split_direction = "horizontal" | "vertical";
export type workspace_layout = {
    kind: "cell";
    id: number;
} | {
    kind: "split";
    direction: split_direction;
    ratio: number;
    a: workspace_layout;
    b: workspace_layout;
};
export interface workspace_cell_state_v1 {
    stable_cell_id: number;
    /** Opaque host-owned identity (symbol/instrument key); aeris_charts never interprets it. */
    host_chart_identity?: string;
    chart_state: chart_state;
}
/** Host-storable layout plus each chart's independently versioned persistence document. */
export interface chart_workspace_state_v1 {
    schema: "aeris_charts-workspace";
    schema_version: 1;
    layout: workspace_layout;
    active_cell?: number;
    cells: workspace_cell_state_v1[];
}
/** Host-owned usage snapshot, emitted on every topology change. */
export interface grid_usage {
    chart_count: number;
    split_count: number;
    elapsed_seconds: number;
    cells: {
        id: number;
        age_seconds: number;
    }[];
}
export interface grid_cell {
    readonly id: number;
    readonly chart: chart_api;
    /** The cell's flex slot; per-cell chrome (buttons, legends) attaches here. */
    readonly element: HTMLDivElement;
    readonly created_at: number;
    /** Optional opaque host identity included by workspace export/restore. */
    readonly host_chart_identity: string | null;
    set_host_chart_identity(identity: string | null): void;
    /**
     * Split this cell in two: the existing chart keeps its state in the first half and a fresh
     * chart (same base options) is created in the second. Resolves to the NEW cell, or `null`
     * when the host cap or veto rejects the split.
     */
    split(direction: split_direction): Promise<grid_cell | null>;
    /** Remove this cell: its sibling subtree absorbs the freed space (last cell refuses). */
    remove(): boolean;
}
export interface chart_grid_options {
    /** Base options for every chart the grid creates (the first cell and each split half). */
    chart_options?: deep_partial<chart_options>;
    /** Host-owned hard cap on live charts; `null`/absent = unlimited. */
    max_charts?: number | null;
    /** Veto hook evaluated before topology mutation: return `false` to reject. */
    on_split_request?: (next_count: number) => boolean;
    /** Usage signal sink: called with a fresh engine snapshot on every split/remove. */
    on_usage?: (usage: grid_usage) => void;
    /** Optional heartbeat (ms) re-emitting usage so time-based meters stay current. */
    usage_heartbeat_ms?: number;
    /** The split-divider line color. `null`/absent (the default) FOLLOWS the first chart's
     *  axis border color — the divider is axis chrome, so it tracks the same token (theme and
     *  explicit border changes); an explicit string pins it. */
    divider_color?: string | null;
    /** Split shortcuts: `true` uses the engine defaults (shortcuts.ts `DEFAULT_SHORTCUTS`), an
     *  object overrides individual combos (e.g. `{ "grid.split_vertical": "ctrl+shift+x" }`),
     *  `false` disables. Default `true` — splitting is native grid behavior; platforms re-key
     *  rather than enable. Keys never fire while typing in a field. */
    shortcuts?: boolean | Partial<Record<shortcut_action, string>>;
    /** Fired for every cell created by a split (button or shortcut path): the platform's hook
     *  to seed/configure the fresh chart (data, theme, chrome). */
    on_cell_added?: (cell: grid_cell) => void;
    /** Restore a fresh grid from generic topology plus composed chart persistence V1 documents. */
    initial_state?: chart_workspace_state_v1;
    /** Fired for every restored cell after its Aeris chart state has been installed. */
    on_cell_restored?: (cell: grid_cell) => void;
}
export interface chart_grid {
    cells(): grid_cell[];
    chart_count(): number;
    usage(): grid_usage;
    set_max_charts(n: number | null): void;
    /** Pin a divider color, or pass `null` to re-enter follow mode (divider = axis border). */
    set_divider_color(color: string | null): void;
    /** The active cell (the last one pointer-pressed) — the target of the split shortcuts. */
    active_cell(): grid_cell;
    /** Explicitly activate a live cell by stable identity (toolbar/programmatic routing). */
    set_active_cell(cell: grid_cell): void;
    /** Arm/disarm the drawing tool on the active chart; the grid migrates an armed tool on focus. */
    set_drawing_tool(tool: drawing_kind | null, options?: Partial<drawing_options>): void;
    active_drawing_tool(): drawing_kind | null;
    set_drawing_tool_listener(listener: ((tool: drawing_kind | null) => void) | null): void;
    undo_drawing(): boolean;
    redo_drawing(): boolean;
    /** Reset the active chart's time and price views. */
    reset_view(): void;
    /** Compose layout and each cell's existing chart persistence V1 for host-owned storage. */
    export_state(): chart_workspace_state_v1;
    /** Maximize a cell to the full container (the others + dividers hide), or pass `null` to
     *  restore. Alt+click on a cell toggles this whenever the grid shows more than one chart. */
    maximize(cell: grid_cell | null): void;
    /** The maximized cell, or `null` when the grid is in its normal layout. */
    maximized_cell(): grid_cell | null;
    destroy(): void;
}
/**
 * @experimental Split-grid topology is public but not part of the frozen stable surface.
 * Create a grid holding a single chart in `container`. Split from there via `cell.split(...)`.
 * The container becomes a flex root the grid owns entirely.
 */
export declare function create_chart_grid(container: HTMLElement, options?: chart_grid_options): Promise<chart_grid>;
