/**
 * Central shortcut registry. Every discrete keyboard shortcut the package installs lives in
 * ONE editable table below — add future shortcuts here (id, default combo, a one-line
 * description) instead of scattering key handling across the engine. Platforms re-key any
 * binding through the host feature's option (e.g. `create_chart_grid(..., { shortcuts: {
 * "grid.split_vertical": "ctrl+shift+x" } })` — no engine rebuild needed for them; editing
 * DEFAULT_SHORTCUTS + rebuilding re-keys the engine's own defaults.
 */
/** Action ids the package ships shortcuts for. */
export type shortcut_action = "grid.split_horizontal" | "grid.split_vertical" | "drawing.undo" | "drawing.redo";
export interface shortcut_binding {
    /** Default key combo: `"ctrl+h"`, `"ctrl+shift+x"`, `"alt+1"` (ctrl ≡ ctrl/cmd on any OS). */
    combo: string;
    description: string;
}
/** The engine's default key table — edit here and rebuild to re-key globally. */
export declare const DEFAULT_SHORTCUTS: Record<shortcut_action, shortcut_binding>;
/** A resolved shortcut ready to install: parsed combo plus its handler. */
export interface resolved_shortcut {
    combo: string;
    run: (event: KeyboardEvent) => void;
}
/**
 * Install a set of shortcuts on the document: each fires when its combo matches exactly
 * (modifiers equal, key equal, no auto-repeat, not while editing). The first match wins and
 * gets `preventDefault` (browser history/paste would otherwise eat Ctrl+H/Ctrl+V). Returns
 * the detach function.
 */
export declare function install_shortcuts(bindings: resolved_shortcut[]): () => void;
