// The chart pane: Aeris Charts draws the candles; this feature only feeds it
// the runtime's bar stream and maps themes.md tokens onto the chart palette.

import {
  type chart_api,
  type chart_theme,
  create_chart,
  init_wasm,
  type series_api,
  theme_options,
} from "@aeristerminal/aeris-charts";
import wasmUrl from "@aeristerminal/aeris-charts/wasm?url";
import { createEffect, onCleanup, onMount } from "solid-js";
import { setFrameHandler } from "../../bridge/backend";
import type { Bar, DecodedFrame } from "../../bridge/decode";
import { fixedToNumber, nanosToSeconds } from "../../lib/format";
import { observeTheme, resolveTokenColor } from "../../lib/tokens";
import type { InstrumentView } from "../../protocol/InstrumentView";
import type { IntervalLabel } from "../../protocol/IntervalLabel";
import "./chart-pane.css";

const MAXIMUM_BARS = 5000;

let wasmReady: Promise<unknown> | null = null;

function ensureWasm(): Promise<unknown> {
  wasmReady ??= init_wasm(wasmUrl);
  return wasmReady;
}

function chartPalette(): chart_theme {
  return {
    background: resolveTokenColor("--surface"),
    foreground: resolveTokenColor("--text-default"),
    primary: resolveTokenColor("--primary"),
    primary_foreground: resolveTokenColor("--primary-foreground"),
    primary_hover: resolveTokenColor("--primary-hover"),
    muted: resolveTokenColor("--surface-secondary"),
    muted_foreground: resolveTokenColor("--text-secondary"),
    accent: resolveTokenColor("--primary"),
    border: resolveTokenColor("--border"),
    muted_border: resolveTokenColor("--border-subtle"),
    ring: resolveTokenColor("--primary"),
    crosshair_line: resolveTokenColor("--border-secondary"),
    crosshair_label: resolveTokenColor("--surface-secondary"),
    bullish: resolveTokenColor("--positive"),
    bearish: resolveTokenColor("--negative"),
  };
}

function columns(bars: Bar[], priceScale: number) {
  const count = bars.length;
  const times = new Float64Array(count);
  const open = new Float64Array(count);
  const high = new Float64Array(count);
  const low = new Float64Array(count);
  const close = new Float64Array(count);
  for (let index = 0; index < count; index += 1) {
    const bar = bars[index];
    times[index] = nanosToSeconds(bar.openUnixNanos);
    open[index] = fixedToNumber(bar.open, priceScale);
    high[index] = fixedToNumber(bar.high, priceScale);
    low[index] = fixedToNumber(bar.low, priceScale);
    close[index] = fixedToNumber(bar.close, priceScale);
  }
  return { times, open, high, low, close };
}

export function ChartPane(props: { instrument: InstrumentView; interval: IntervalLabel }) {
  let container!: HTMLDivElement;
  let chart: chart_api | null = null;
  let series: series_api | null = null;
  let disposed = false;
  let closed: Bar[] = [];
  let forming: Bar | null = null;

  function install(): void {
    if (series === null) {
      return;
    }
    const bars = forming === null ? closed : [...closed, forming];
    series.set_data_typed(columns(bars, props.instrument.priceScale));
  }

  function handleFrame(frame: DecodedFrame): void {
    if (frame.kind === 3) {
      closed = frame.closed.slice(-MAXIMUM_BARS);
      forming = frame.forming;
      install();
      chart?.time_scale().fit_content();
      return;
    }
    if (frame.kind !== 4) {
      return;
    }
    if (frame.completed.length > 0) {
      closed = [...closed, ...frame.completed].slice(-MAXIMUM_BARS);
    }
    if (frame.clearForming) {
      forming = null;
      install();
      return;
    }
    if (frame.forming !== null) {
      forming = frame.forming;
    }
    if (series === null) {
      return;
    }
    const batch = frame.forming === null ? frame.completed : [...frame.completed, frame.forming];
    if (batch.length > 0) {
      series.update_typed(columns(batch, props.instrument.priceScale));
    }
  }

  createEffect(() => {
    const key = `bars:${props.instrument.instrumentId}:${props.interval}`;
    closed = [];
    forming = null;
    if (series !== null) {
      series.set_data_typed(columns([], props.instrument.priceScale));
    }
    setFrameHandler(key, handleFrame);
    onCleanup(() => {
      setFrameHandler(key, null);
    });
  });

  onMount(() => {
    const stopTheme = observeTheme(() => {
      chart?.apply_options(theme_options(chartPalette()));
    });
    onCleanup(() => {
      stopTheme();
      disposed = true;
      chart?.remove();
      chart = null;
      series = null;
    });
    void (async () => {
      await ensureWasm();
      if (disposed) {
        return;
      }
      const created = await create_chart(container, {
        autoSize: true,
        layout: {
          fontFamily: "Inter, system-ui, sans-serif",
          fontSize: 11,
        },
      });
      if (disposed) {
        created.remove();
        return;
      }
      chart = created;
      created.apply_options(theme_options(chartPalette()));
      created.timeScale().apply_options({
        time_visible: true,
        seconds_visible: false,
        right_offset: 4,
      });
      series = created.add_series("candlestick", { max_points: MAXIMUM_BARS });
      install();
    })();
  });

  return <div ref={container} class="chart-pane" />;
}
