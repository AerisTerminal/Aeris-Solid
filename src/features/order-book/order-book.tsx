// The platform order book: a canvas ladder fed by the runtime's book stream.
// No DOM row per price level; the runtime owns the canonical state and this
// view only reconstructs the published image from snapshots and deltas.

import { createEffect, onCleanup, onMount } from "solid-js";
import { setFrameHandler } from "../../bridge/backend";
import type { BookHead, BookLevel, DecodedFrame, VolumeRow } from "../../bridge/decode";
import { formatFixed, incrementDecimals } from "../../lib/format";
import { observeTheme, resolveTokenColor } from "../../lib/tokens";
import type { InstrumentView } from "../../protocol/InstrumentView";
import "./order-book.css";

const ROW_HEIGHT = 22;
const HEADER_HEIGHT = 26;
const MID_HEIGHT = 26;

interface BookModel {
  head: BookHead | null;
  bids: BookLevel[];
  asks: BookLevel[];
  volumes: Map<bigint, VolumeRow>;
}

interface Palette {
  text: string;
  textSecondary: string;
  positive: string;
  negative: string;
  positiveFill: string;
  negativeFill: string;
  border: string;
  warning: string;
}

function readPalette(): Palette {
  return {
    text: resolveTokenColor("--text-default"),
    textSecondary: resolveTokenColor("--text-secondary"),
    positive: resolveTokenColor("--text-positive"),
    negative: resolveTokenColor("--text-negative"),
    positiveFill: resolveTokenColor("--positive"),
    negativeFill: resolveTokenColor("--negative"),
    border: resolveTokenColor("--border-subtle"),
    warning: resolveTokenColor("--text-warning"),
  };
}

function emptyModel(): BookModel {
  return { head: null, bids: [], asks: [], volumes: new Map() };
}

/** Upserts changed levels into a side kept sorted away from the touch. */
function applyChanges(side: BookLevel[], changes: BookLevel[], descending: boolean): BookLevel[] {
  const byPrice = new Map(side.map((level) => [level.price, level]));
  for (const change of changes) {
    if (change.quantity === 0n) {
      byPrice.delete(change.price);
    } else {
      byPrice.set(change.price, change);
    }
  }
  const levels = [...byPrice.values()];
  levels.sort((a, b) => (descending ? Number(b.price - a.price) : Number(a.price - b.price)));
  return levels;
}

function applyVolumes(volumes: Map<bigint, VolumeRow>, changes: VolumeRow[]): void {
  for (const change of changes) {
    if (change.buy === 0n && change.sell === 0n) {
      volumes.delete(change.price);
    } else {
      volumes.set(change.price, change);
    }
  }
}

export function OrderBook(props: { instrument: InstrumentView }) {
  let container!: HTMLDivElement;
  let canvas!: HTMLCanvasElement;
  let model = emptyModel();
  let palette = readPalette();
  let framePending = false;

  const priceDecimals = () =>
    incrementDecimals(
      props.instrument.priceIncrement === null ? null : BigInt(props.instrument.priceIncrement),
      props.instrument.priceScale,
    );
  const quantityDecimals = () =>
    incrementDecimals(
      props.instrument.quantityIncrement === null
        ? null
        : BigInt(props.instrument.quantityIncrement),
      props.instrument.quantityScale,
    );

  function scheduleDraw(): void {
    if (framePending) {
      return;
    }
    framePending = true;
    requestAnimationFrame(() => {
      framePending = false;
      draw();
    });
  }

  function draw(): void {
    const width = container.clientWidth;
    const height = container.clientHeight;
    if (width === 0 || height === 0) {
      return;
    }
    const ratio = window.devicePixelRatio || 1;
    if (
      canvas.width !== Math.round(width * ratio) ||
      canvas.height !== Math.round(height * ratio)
    ) {
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
    }
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      return;
    }
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.font = "500 12px Manrope, Inter, system-ui, sans-serif";
    // Tabular figures keep ladder columns aligned; the DOM lib lags the API.
    (ctx as CanvasRenderingContext2D & { fontVariantNumeric: string }).fontVariantNumeric =
      "tabular-nums";
    ctx.textBaseline = "middle";

    const priceWidth = Math.floor(width * 0.38);
    const sizeWidth = Math.floor(width * 0.32);
    const volumeLeft = priceWidth + sizeWidth;
    const volumeWidth = width - volumeLeft;

    ctx.fillStyle = palette.textSecondary;
    ctx.textAlign = "right";
    ctx.fillText("Price", priceWidth - 8, HEADER_HEIGHT / 2);
    ctx.fillText("Size", priceWidth + sizeWidth - 8, HEADER_HEIGHT / 2);
    ctx.textAlign = "left";
    ctx.fillText("Volume", volumeLeft + 8, HEADER_HEIGHT / 2);
    ctx.strokeStyle = palette.border;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, HEADER_HEIGHT - 0.5);
    ctx.lineTo(width, HEADER_HEIGHT - 0.5);
    ctx.stroke();

    if (model.head === null) {
      ctx.fillStyle = palette.textSecondary;
      ctx.textAlign = "center";
      ctx.fillText("Waiting for the book…", width / 2, height / 2);
      return;
    }

    const rows = Math.max(0, Math.floor((height - HEADER_HEIGHT - MID_HEIGHT) / ROW_HEIGHT));
    const askRows = Math.min(model.asks.length, Math.ceil(rows / 2));
    const bidRows = Math.min(model.bids.length, rows - askRows);
    const asks = model.asks.slice(0, askRows).reverse();
    const bids = model.bids.slice(0, bidRows);

    let maxVolume = 0n;
    for (const level of [...asks, ...bids]) {
      const volume = model.volumes.get(level.price);
      if (volume !== undefined && volume.buy + volume.sell > maxVolume) {
        maxVolume = volume.buy + volume.sell;
      }
    }

    const priceText = (price: bigint) =>
      formatFixed(price, props.instrument.priceScale, priceDecimals());
    const sizeText = (quantity: bigint) =>
      formatFixed(quantity, props.instrument.quantityScale, quantityDecimals());

    const drawRow = (level: BookLevel, y: number, bid: boolean) => {
      ctx.textAlign = "right";
      ctx.fillStyle = bid ? palette.positive : palette.negative;
      ctx.fillText(priceText(level.price), priceWidth - 8, y + ROW_HEIGHT / 2);
      ctx.fillStyle = palette.text;
      ctx.fillText(sizeText(level.quantity), priceWidth + sizeWidth - 8, y + ROW_HEIGHT / 2);
      const volume = model.volumes.get(level.price);
      if (volume !== undefined && maxVolume > 0n) {
        const barWidth = volumeWidth - 16;
        const total = volume.buy + volume.sell;
        const scaled = Number(total) / Number(maxVolume);
        const buyShare = total === 0n ? 0 : Number(volume.buy) / Number(total);
        const drawn = Math.max(1, Math.round(barWidth * scaled));
        const buyWidth = Math.round(drawn * buyShare);
        const top = y + 6;
        ctx.fillStyle = palette.positiveFill;
        ctx.fillRect(volumeLeft + 8, top, buyWidth, ROW_HEIGHT - 12);
        ctx.fillStyle = palette.negativeFill;
        ctx.fillRect(volumeLeft + 8 + buyWidth, top, drawn - buyWidth, ROW_HEIGHT - 12);
      }
    };

    let y = HEADER_HEIGHT;
    for (const level of asks) {
      drawRow(level, y, false);
      y += ROW_HEIGHT;
    }

    // The touch row shows the BBO exactly as the runtime published it.
    ctx.strokeStyle = palette.border;
    ctx.beginPath();
    ctx.moveTo(0, y + 0.5);
    ctx.lineTo(width, y + 0.5);
    ctx.moveTo(0, y + MID_HEIGHT - 0.5);
    ctx.lineTo(width, y + MID_HEIGHT - 0.5);
    ctx.stroke();
    ctx.textAlign = "right";
    const bid = model.head.bestBid;
    const ask = model.head.bestAsk;
    ctx.fillStyle = palette.positive;
    ctx.fillText(bid === null ? "—" : priceText(bid.price), priceWidth - 8, y + MID_HEIGHT / 2);
    ctx.fillStyle = palette.text;
    const bidSize = bid === null || bid.quantity === null ? "—" : sizeText(bid.quantity);
    ctx.fillText(bidSize, priceWidth + sizeWidth - 8, y + MID_HEIGHT / 2);
    ctx.textAlign = "left";
    ctx.fillStyle = palette.negative;
    const askText = ask === null ? "—" : priceText(ask.price);
    const askSize = ask === null || ask.quantity === null ? "—" : sizeText(ask.quantity);
    ctx.fillText(`${askText} × ${askSize}`, volumeLeft + 8, y + MID_HEIGHT / 2);
    y += MID_HEIGHT;

    for (const level of bids) {
      drawRow(level, y, true);
      y += ROW_HEIGHT;
    }

    if (model.head.state !== 1) {
      ctx.fillStyle = palette.warning;
      ctx.textAlign = "center";
      const label = model.head.state === 0 ? "Recovering" : "Stale";
      ctx.fillText(label, width / 2, height - 12);
    }
  }

  function handleFrame(frame: DecodedFrame): void {
    if (frame.kind === 1) {
      model = {
        head: frame.head,
        bids: frame.bids,
        asks: frame.asks,
        volumes: new Map(frame.volumes.map((row) => [row.price, row])),
      };
    } else if (frame.kind === 2) {
      model = {
        head: frame.head,
        bids: applyChanges(model.bids, frame.bidChanges, true),
        asks: applyChanges(model.asks, frame.askChanges, false),
        volumes: model.volumes,
      };
      applyVolumes(model.volumes, frame.volumeChanges);
    } else {
      return;
    }
    scheduleDraw();
  }

  createEffect(() => {
    const key = `book:${props.instrument.instrumentId}`;
    model = emptyModel();
    setFrameHandler(key, handleFrame);
    scheduleDraw();
    onCleanup(() => {
      setFrameHandler(key, null);
    });
  });

  onMount(() => {
    const resize = new ResizeObserver(() => {
      scheduleDraw();
    });
    resize.observe(container);
    const stopTheme = observeTheme(() => {
      palette = readPalette();
      scheduleDraw();
    });
    onCleanup(() => {
      resize.disconnect();
      stopTheme();
    });
  });

  return (
    <div ref={container} class="order-book">
      <canvas ref={canvas} class="order-book-canvas" />
    </div>
  );
}
