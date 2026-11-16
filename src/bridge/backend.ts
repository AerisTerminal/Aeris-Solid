// The one backend interface the UI talks to. `tauri.ts` implements it for
// desktop; a worker implementation for the web plugs in here later.

import type { Command } from "../protocol/Command";
import type { CommandError } from "../protocol/CommandError";
import type { CommandErrorKind } from "../protocol/CommandErrorKind";
import type { CommandResult } from "../protocol/CommandResult";
import { type DecodedFrame, decodeFrame } from "./decode";

export interface Backend {
  /** True when running inside the native desktop shell. */
  readonly native: boolean;
  execute(command: Command): Promise<CommandResult>;
  /** Attaches the binary frame stream; resolves once the shell accepted it. */
  openStream(): Promise<void>;
}

export type FrameHandler = (frame: DecodedFrame) => void;

// One handler per stream key; a new handler replaces the previous one.
const handlers = new Map<string, FrameHandler>();

export function setFrameHandler(key: string, handler: FrameHandler | null): void {
  if (handler === null) {
    handlers.delete(key);
  } else {
    handlers.set(key, handler);
  }
}

/** Decodes one raw frame and routes it to its stream's handler. */
export function dispatchFrame(buffer: ArrayBuffer): void {
  const frame = decodeFrame(buffer);
  if (frame === null) {
    return;
  }
  handlers.get(frame.key)?.(frame);
}

const ERROR_KINDS: ReadonlySet<string> = new Set<CommandErrorKind>([
  "invalidRequest",
  "unknownInstrument",
  "unsupportedInterval",
  "providerUnavailable",
  "rateLimited",
  "internal",
]);

/** Normalizes any rejection into the typed command error. */
export function toCommandError(value: unknown): CommandError {
  if (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof value.kind === "string" &&
    ERROR_KINDS.has(value.kind) &&
    typeof value.message === "string"
  ) {
    return { kind: value.kind as CommandErrorKind, message: value.message };
  }
  return { kind: "internal", message: "the backend call failed" };
}
