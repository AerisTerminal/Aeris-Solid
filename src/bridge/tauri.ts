// The only file that imports @tauri-apps/api: typed commands over invoke,
// binary frames over a raw-byte Channel.

import { Channel, invoke } from "@tauri-apps/api/core";
import type { Command } from "../protocol/Command";
import type { CommandResult } from "../protocol/CommandResult";
import { type Backend, dispatchFrame, toCommandError } from "./backend";

export function createTauriBackend(): Backend {
  return {
    native: true,
    async execute(command: Command): Promise<CommandResult> {
      try {
        return await invoke<CommandResult>("market_command", { command });
      } catch (error) {
        throw toCommandError(error);
      }
    },
    async openStream(): Promise<void> {
      const channel = new Channel<ArrayBuffer>();
      channel.onmessage = (frame) => {
        dispatchFrame(frame);
      };
      try {
        await invoke("open_market_stream", { onFrame: channel });
      } catch (error) {
        throw toCommandError(error);
      }
    },
  };
}
