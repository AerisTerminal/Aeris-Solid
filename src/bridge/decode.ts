// Binary frame decoder: mirrors crates/app_protocol/src/binary.rs byte for
// byte. Little-endian, fail-closed on truncation or unknown versions/kinds.

const PROTOCOL_VERSION = 1;
const KIND_BOOK_SNAPSHOT = 1;
const KIND_BOOK_DELTA = 2;
const KIND_BARS_SNAPSHOT = 3;
const KIND_BARS_UPDATE = 4;
const KIND_SESSION = 5;

const ABSENT_QUANTITY = -9223372036854775808n;
const ABSENT_ORDER_COUNT = 0xffffffff;

export interface BookQuote {
  price: bigint;
  quantity: bigint | null;
  orderCount: number | null;
}

export interface BookLevel {
  price: bigint;
  quantity: bigint;
  orderCount: number | null;
}

export interface BookHead {
  revision: bigint;
  sourceWatermark: bigint;
  /** 0 recovering, 1 ready, 2 stale. */
  state: number;
  /** Recovery reason: 0 awaiting, 1 gap, 2 crossed, 3 invalid. */
  reason: number;
  topOfBookOnly: boolean;
  bestBid: BookQuote | null;
  bestAsk: BookQuote | null;
  bboWatermark: bigint;
  tradeWatermark: bigint;
}

export interface VolumeRow {
  price: bigint;
  buy: bigint;
  sell: bigint;
}

export interface Bar {
  sequence: bigint;
  openUnixNanos: bigint;
  open: bigint;
  high: bigint;
  low: bigint;
  close: bigint;
  volume: bigint;
}

interface FrameHeader {
  kind: number;
  key: string;
  generation: bigint;
}

export type DecodedFrame =
  | (FrameHeader & {
      kind: typeof KIND_BOOK_SNAPSHOT;
      head: BookHead;
      bids: BookLevel[];
      asks: BookLevel[];
      volumes: VolumeRow[];
    })
  | (FrameHeader & {
      kind: typeof KIND_BOOK_DELTA;
      head: BookHead;
      bidChanges: BookLevel[];
      askChanges: BookLevel[];
      volumeChanges: VolumeRow[];
    })
  | (FrameHeader & {
      kind: typeof KIND_BARS_SNAPSHOT;
      closed: Bar[];
      forming: Bar | null;
    })
  | (FrameHeader & {
      kind: typeof KIND_BARS_UPDATE;
      completed: Bar[];
      forming: Bar | null;
      clearForming: boolean;
    })
  | (FrameHeader & {
      kind: typeof KIND_SESSION;
      /** 0 connecting, 1 live, 2 reconnecting, 3 failed, 4 stopped. */
      state: number;
      attempt: number;
      message: string;
    });

class Reader {
  private readonly view: DataView;
  private offset = 0;

  constructor(buffer: ArrayBuffer) {
    this.view = new DataView(buffer);
  }

  get remaining(): number {
    return this.view.byteLength - this.offset;
  }

  private take(count: number): boolean {
    if (this.remaining < count) {
      return false;
    }
    this.offset += count;
    return true;
  }

  u8(): number | null {
    if (!this.take(1)) {
      return null;
    }
    return this.view.getUint8(this.offset - 1);
  }

  u16(): number | null {
    if (!this.take(2)) {
      return null;
    }
    return this.view.getUint16(this.offset - 2, true);
  }

  u32(): number | null {
    if (!this.take(4)) {
      return null;
    }
    return this.view.getUint32(this.offset - 4, true);
  }

  u64(): bigint | null {
    if (!this.take(8)) {
      return null;
    }
    return this.view.getBigUint64(this.offset - 8, true);
  }

  i64(): bigint | null {
    if (!this.take(8)) {
      return null;
    }
    return this.view.getBigInt64(this.offset - 8, true);
  }

  text(): string | null {
    const length = this.u16();
    if (length === null || !this.take(length)) {
      return null;
    }
    const bytes = new Uint8Array(this.view.buffer, this.offset - length, length);
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch {
      return null;
    }
  }
}

function readQuote(reader: Reader): BookQuote | null {
  const price = reader.i64();
  const quantity = reader.i64();
  const orderCount = reader.u32();
  if (price === null || quantity === null || orderCount === null) {
    return null;
  }
  return {
    price,
    quantity: quantity === ABSENT_QUANTITY ? null : quantity,
    orderCount: orderCount === ABSENT_ORDER_COUNT ? null : orderCount,
  };
}

function readLevel(reader: Reader): BookLevel | null {
  const price = reader.i64();
  const quantity = reader.i64();
  const orderCount = reader.u32();
  if (price === null || quantity === null || orderCount === null) {
    return null;
  }
  return {
    price,
    quantity,
    orderCount: orderCount === ABSENT_ORDER_COUNT ? null : orderCount,
  };
}

function readLevels(reader: Reader, count: number): BookLevel[] | null {
  const levels: BookLevel[] = [];
  for (let index = 0; index < count; index += 1) {
    const level = readLevel(reader);
    if (level === null) {
      return null;
    }
    levels.push(level);
  }
  return levels;
}

function readVolumes(reader: Reader): VolumeRow[] | null {
  const count = reader.u16();
  if (count === null) {
    return null;
  }
  const volumes: VolumeRow[] = [];
  for (let index = 0; index < count; index += 1) {
    const price = reader.i64();
    const buy = reader.i64();
    const sell = reader.i64();
    if (price === null || buy === null || sell === null) {
      return null;
    }
    volumes.push({ price, buy, sell });
  }
  return volumes;
}

function readBookHead(reader: Reader): BookHead | null {
  const revision = reader.u64();
  const sourceWatermark = reader.u64();
  const state = reader.u8();
  const reason = reader.u8();
  const flags = reader.u8();
  if (
    revision === null ||
    sourceWatermark === null ||
    state === null ||
    reason === null ||
    flags === null
  ) {
    return null;
  }
  const bestBid = flags & 0b10 ? readQuote(reader) : null;
  const bestAsk = flags & 0b100 ? readQuote(reader) : null;
  if ((flags & 0b10 && bestBid === null) || (flags & 0b100 && bestAsk === null)) {
    return null;
  }
  const bboWatermark = reader.u64();
  const tradeWatermark = reader.u64();
  if (bboWatermark === null || tradeWatermark === null) {
    return null;
  }
  return {
    revision,
    sourceWatermark,
    state,
    reason,
    topOfBookOnly: (flags & 1) !== 0,
    bestBid,
    bestAsk,
    bboWatermark,
    tradeWatermark,
  };
}

function readBar(reader: Reader): Bar | null {
  const sequence = reader.u64();
  const openUnixNanos = reader.i64();
  const open = reader.i64();
  const high = reader.i64();
  const low = reader.i64();
  const close = reader.i64();
  const volume = reader.i64();
  if (
    sequence === null ||
    openUnixNanos === null ||
    open === null ||
    high === null ||
    low === null ||
    close === null ||
    volume === null
  ) {
    return null;
  }
  return { sequence, openUnixNanos, open, high, low, close, volume };
}

function readBars(reader: Reader, count: number): Bar[] | null {
  const bars: Bar[] = [];
  for (let index = 0; index < count; index += 1) {
    const bar = readBar(reader);
    if (bar === null) {
      return null;
    }
    bars.push(bar);
  }
  return bars;
}

/** Decodes one binary frame; null on any malformed or unknown input. */
export function decodeFrame(buffer: ArrayBuffer): DecodedFrame | null {
  const reader = new Reader(buffer);
  const version = reader.u8();
  const kind = reader.u8();
  const key = reader.text();
  const generation = reader.u64();
  if (version !== PROTOCOL_VERSION || kind === null || key === null || generation === null) {
    return null;
  }
  const header = { key, generation };
  switch (kind) {
    case KIND_BOOK_SNAPSHOT:
    case KIND_BOOK_DELTA: {
      const head = readBookHead(reader);
      const bidCount = reader.u16();
      const askCount = reader.u16();
      if (head === null || bidCount === null || askCount === null) {
        return null;
      }
      const bids = readLevels(reader, bidCount);
      const asks = readLevels(reader, askCount);
      const volumes = readVolumes(reader);
      if (bids === null || asks === null || volumes === null) {
        return null;
      }
      return kind === KIND_BOOK_SNAPSHOT
        ? { ...header, kind, head, bids, asks, volumes }
        : { ...header, kind, head, bidChanges: bids, askChanges: asks, volumeChanges: volumes };
    }
    case KIND_BARS_SNAPSHOT:
    case KIND_BARS_UPDATE: {
      const count = reader.u32();
      if (count === null) {
        return null;
      }
      const bars = readBars(reader, count);
      const flag = reader.u8();
      if (bars === null || flag === null) {
        return null;
      }
      const forming = flag === 1 ? readBar(reader) : null;
      if (flag === 1 && forming === null) {
        return null;
      }
      return kind === KIND_BARS_SNAPSHOT
        ? { ...header, kind, closed: bars, forming }
        : { ...header, kind, completed: bars, forming, clearForming: flag === 2 };
    }
    case KIND_SESSION: {
      const state = reader.u8();
      const attempt = reader.u32();
      const message = reader.text();
      if (state === null || attempt === null || message === null) {
        return null;
      }
      return { ...header, kind, state, attempt, message };
    }
    default:
      return null;
  }
}
