/** Browser event translation for the engine-owned chart input controller. */
import type { chart_impl } from "./impl.js";
/** One pointer ABI shared by DOM and worker hosts: device | modifiers | options. */
export declare function controller_pointer_flags(device: number, modifiers: number, options: number): number;
export declare function install_gestures(chart: chart_impl): {
    detach: () => void;
    sync: () => void;
};
