import { describe, expect, test } from "bun:test";
import { decodeFrame } from "./decode";

// The shared cross-language fixture: session_frame("fixture", 1,
// Reconnecting { attempt: 1 }) from crates/app_protocol/src/binary.rs.
const SESSION_FIXTURE = new Uint8Array([
  0x01, 0x05, 0x0f, 0x00, 0x73, 0x65, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x3a, 0x66, 0x69, 0x78, 0x74,
  0x75, 0x72, 0x65, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01, 0x00, 0x00, 0x00,
  0x00, 0x00,
]);

class Writer {
  private readonly bytes: number[] = [];

  u8(value: number): void {
    this.bytes.push(value & 0xff);
  }

  u16(value: number): void {
    this.bytes.push(value & 0xff, (value >> 8) & 0xff);
  }

  u32(value: number): void {
    for (let shift = 0; shift < 32; shift += 8) {
      this.bytes.push((value >>> shift) & 0xff);
    }
  }

  u64(value: bigint): void {
    for (let shift = 0n; shift < 64n; shift += 8n) {
      this.bytes.push(Number((value >> shift) & 0xffn));
    }
  }

  i64(value: bigint): void {
    this.u64(BigInt.asUintN(64, value));
  }

  text(value: string): void {
    const encoded = new TextEncoder().encode(value);
    this.u16(encoded.length);
    for (const byte of encoded) {
      this.bytes.push(byte);
    }
  }

  finish(): ArrayBuffer {
    return new Uint8Array(this.bytes).buffer;
  }
}

function bookSnapshotFixture(): ArrayBuffer {
  const writer = new Writer();
  writer.u8(1); // version
  writer.u8(1); // book snapshot
  writer.text("book:instrument:fixture:BTC");
  writer.u64(3n); // generation
  writer.u64(9n); // revision
  writer.u64(40n); // source watermark
  writer.u8(1); // ready
  writer.u8(0); // reason
  writer.u8(0b110); // has bid + has ask
  writer.i64(100n); // bid price
  writer.i64(3n); // bid quantity
  writer.u32(0xffffffff); // bid count absent
  writer.i64(102n); // ask price
  writer.i64(-9223372036854775808n); // ask quantity absent
  writer.u32(2); // ask count
  writer.u64(7n); // bbo watermark
  writer.u64(12n); // trade watermark
  writer.u16(1); // bid levels
  writer.u16(1); // ask levels
  writer.i64(100n);
  writer.i64(3n);
  writer.u32(0xffffffff);
  writer.i64(102n);
  writer.i64(4n);
  writer.u32(0xffffffff);
  writer.u16(1); // volume rows
  writer.i64(100n);
  writer.i64(11n);
  writer.i64(7n);
  return writer.finish();
}

describe("decodeFrame", () => {
  test("decodes the shared session fixture", () => {
    const frame = decodeFrame(SESSION_FIXTURE.buffer);
    expect(frame).not.toBeNull();
    if (frame === null || frame.kind !== 5) {
      throw new Error("session frame expected");
    }
    expect(frame.key).toBe("session:fixture");
    expect(frame.generation).toBe(1n);
    expect(frame.state).toBe(2);
    expect(frame.attempt).toBe(1);
    expect(frame.message).toBe("");
  });

  test("decodes a book snapshot with absent sentinels", () => {
    const frame = decodeFrame(bookSnapshotFixture());
    if (frame === null || frame.kind !== 1) {
      throw new Error("book snapshot expected");
    }
    expect(frame.key).toBe("book:instrument:fixture:BTC");
    expect(frame.generation).toBe(3n);
    expect(frame.head.revision).toBe(9n);
    expect(frame.head.state).toBe(1);
    expect(frame.head.bestBid?.quantity).toBe(3n);
    expect(frame.head.bestBid?.orderCount).toBeNull();
    expect(frame.head.bestAsk?.quantity).toBeNull();
    expect(frame.head.bestAsk?.orderCount).toBe(2);
    expect(frame.bids).toEqual([{ price: 100n, quantity: 3n, orderCount: null }]);
    expect(frame.asks).toEqual([{ price: 102n, quantity: 4n, orderCount: null }]);
    expect(frame.volumes).toEqual([{ price: 100n, buy: 11n, sell: 7n }]);
  });

  test("fails closed on truncation and unknown versions", () => {
    expect(decodeFrame(new ArrayBuffer(0))).toBeNull();
    expect(decodeFrame(SESSION_FIXTURE.slice(0, 10).buffer)).toBeNull();
    expect(decodeFrame(SESSION_FIXTURE.slice(0, SESSION_FIXTURE.length - 1).buffer)).toBeNull();
    const unknown = new Uint8Array(SESSION_FIXTURE);
    unknown[0] = 0xff;
    expect(decodeFrame(unknown.buffer)).toBeNull();
    const snapshot = new Uint8Array(bookSnapshotFixture());
    expect(decodeFrame(snapshot.slice(0, snapshot.length - 1).buffer)).toBeNull();
  });
});
