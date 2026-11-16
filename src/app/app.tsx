// The shell: composes the chart and order-book features, owns instrument and
// interval selection, and shows session lifecycle evidence. No feature logic.

import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { setFrameHandler, toCommandError } from "../bridge/backend";
import { createTauriBackend } from "../bridge/tauri";
import { ChartPane } from "../features/chart/chart-pane";
import { OrderBook } from "../features/order-book/order-book";
import type { CommandError } from "../protocol/CommandError";
import type { InstrumentView } from "../protocol/InstrumentView";
import type { IntervalLabel } from "../protocol/IntervalLabel";
import type { ProviderView } from "../protocol/ProviderView";
import type { SessionStateView } from "../protocol/SessionStateView";
import "./app.css";

const backend = createTauriBackend();

function sessionLabel(session: SessionStateView): string {
  switch (session.state) {
    case "connecting":
      return "Connecting";
    case "live":
      return "Live";
    case "reconnecting":
      return `Reconnecting (${session.attempt})`;
    case "failed":
      return "Failed";
    case "stopped":
      return "Stopped";
  }
}

function sessionTitle(session: SessionStateView): string | undefined {
  return session.state === "failed" ? session.message : undefined;
}

export function App() {
  const [provider, setProvider] = createSignal<ProviderView | null>(null);
  const [instruments, setInstruments] = createSignal<InstrumentView[]>([]);
  const [instrument, setInstrument] = createSignal<InstrumentView | null>(null);
  const [interval, setInterval] = createSignal<IntervalLabel>("1m");
  const [session, setSession] = createSignal<SessionStateView | null>(null);
  const [failure, setFailure] = createSignal<CommandError | null>(null);

  onMount(() => {
    void (async () => {
      try {
        await backend.openStream();
        const listed = await backend.execute({ command: "listProviders" });
        if (listed.result !== "providers") {
          return;
        }
        const first = listed.providers[0];
        if (first === undefined) {
          setFailure({ kind: "providerUnavailable", message: "no provider is registered" });
          return;
        }
        setProvider(first);
        setFrameHandler(`session:${first.providerId}`, (frame) => {
          if (frame.kind !== 5) {
            return;
          }
          if (frame.state === 2) {
            setSession({ state: "reconnecting", attempt: frame.attempt });
          } else if (frame.state === 3) {
            setSession({ state: "failed", message: frame.message });
          } else if (frame.state === 1) {
            setSession({ state: "live" });
          } else if (frame.state === 4) {
            setSession({ state: "stopped" });
          } else {
            setSession({ state: "connecting" });
          }
        });
        const found = await backend.execute({ command: "searchInstruments", query: "", limit: 20 });
        if (found.result !== "instruments") {
          return;
        }
        setInstruments(found.instruments);
        const pick = found.instruments[0];
        if (pick !== undefined) {
          setInstrument(pick);
        }
        const preferred = first.historyIntervals.includes("1m")
          ? "1m"
          : (first.historyIntervals[0] ?? "1m");
        setInterval(preferred);
      } catch (error) {
        setFailure(toCommandError(error));
      }
    })();
  });

  createEffect(() => {
    const picked = instrument();
    const label = interval();
    if (picked === null) {
      return;
    }
    const instrumentId = picked.instrumentId;
    let active = true;
    void (async () => {
      try {
        await backend.execute({ command: "subscribe", instrumentId, interval: label });
        if (active) {
          setFailure(null);
        }
      } catch (error) {
        if (active) {
          setFailure(toCommandError(error));
        }
      }
    })();
    onCleanup(() => {
      active = false;
      void backend.execute({ command: "unsubscribe", instrumentId }).catch(() => {
        // Unsubscribing a lease that never opened is fine.
      });
    });
  });

  return (
    <div class="app-shell" data-platform={backend.native ? "desktop" : "web"}>
      <header class="app-header">
        <span class="app-title">Aeris</span>
        <select
          class="picker numeric"
          value={instrument()?.instrumentId ?? ""}
          onChange={(event) => {
            const next = instruments().find(
              (candidate) => candidate.instrumentId === event.currentTarget.value,
            );
            if (next !== undefined) {
              setInstrument(next);
            }
          }}
        >
          <For each={instruments()}>
            {(candidate) => <option value={candidate.instrumentId}>{candidate.display}</option>}
          </For>
        </select>
        <select
          class="picker numeric"
          value={interval()}
          onChange={(event) => {
            setInterval(event.currentTarget.value as IntervalLabel);
          }}
        >
          <For each={provider()?.historyIntervals ?? []}>
            {(label) => <option value={label}>{label}</option>}
          </For>
        </select>
        <Show when={session()}>
          {(state) => (
            <span class={`session-chip session-${state().state}`} title={sessionTitle(state())}>
              {sessionLabel(state())}
            </span>
          )}
        </Show>
      </header>
      <Show when={failure()}>
        {(error) => (
          <div class="app-failure">
            <span class="failure-kind">{error().kind}</span>
            <span>{error().message}</span>
          </div>
        )}
      </Show>
      <main class="app-main">
        <Show when={instrument()} keyed>
          {(picked) => (
            <>
              <section class="pane pane-chart">
                <ChartPane instrument={picked} interval={interval()} />
              </section>
              <section class="pane pane-book">
                <OrderBook instrument={picked} />
              </section>
            </>
          )}
        </Show>
      </main>
    </div>
  );
}
