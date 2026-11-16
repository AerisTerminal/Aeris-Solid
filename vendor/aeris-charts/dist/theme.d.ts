/**
 * Aeris's public theme mapping.
 *
 * The core crate's `style_tokens.json` is the backend-consumed projection of the complete host design system in
 * `aeris_charts.css`. The Rust engine compiles its opaque colors and shared radii into defaults,
 * so WebGPU, Canvas2D, GPUI, and the TypeScript package cannot drift while host-only CSS stays out
 * of Rust.
 */
import type { chart_options, deep_partial } from "./types.js";
export interface chart_theme {
    /** Chart main background. */
    background: string;
    /** Primary non-interactive text: axes, prices, values, and boxed labels. */
    foreground: string;
    primary: string;
    primary_foreground: string;
    /** Opaque projection of the CSS primary-hover mix onto this theme's chart surface. */
    primary_hover: string;
    /** Secondary surface; named themes also use it for crosshair labels. */
    muted: string;
    muted_foreground: string;
    accent: string;
    border: string;
    muted_border: string;
    ring: string;
    /** Crosshair line color. Named themes keep the token's alpha so the line stays translucent. */
    crosshair_line: string;
    /**
     * Crosshair label surface. Both themes use the dark-theme `muted` (secondary surface).
     */
    crosshair_label: string;
    /** Candle, volume, and other up-market geometry. */
    bullish: string;
    /** Candle, volume, and other down-market geometry. */
    bearish: string;
}
export declare const light_theme: chart_theme;
export declare const dark_theme: chart_theme;
export type theme_name = "light" | "dark";
export declare const default_theme_name: theme_name;
export declare function theme_palette(name: theme_name): chart_theme;
/**
 * Floating-layer (tooltip) colors matching the platform tooltip: the `--surface-secondary` fill
 * (`muted`), the `--border-secondary` edge and `--text-secondary` text composited over that fill,
 * and `--text-default` text.
 */
export interface popover_palette {
    surface: string;
    border: string;
    foreground: string;
    muted_foreground: string;
}
export declare function popover_palette(name: theme_name): popover_palette;
/**
 * Map a theme (name or explicit palette) onto the chart-options tree. Named themes retain their
 * package-level identity so `chart.apply_options(theme_options(name))` remains compatible with
 * `reset_style_to_defaults()`. Passing an explicit palette is style-only because it has no
 * canonical light/dark identity to retain.
 */
export declare function theme_options(theme: theme_name | chart_theme): deep_partial<chart_options>;
