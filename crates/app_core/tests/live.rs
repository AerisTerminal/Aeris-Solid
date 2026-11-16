//! Live smoke test: the composition root against the registered public
//! provider. Ignored by default; run with `cargo test -p aeris_app_core -- --ignored`.

use aeris_app_core::AppCore;
use aeris_app_protocol::{Command, CommandResult, IntervalLabel, binary};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
#[ignore = "live network smoke test"]
fn live_book_and_bars_frames_reach_the_outbound_channel() {
    let (outbound, frames) = mpsc::sync_channel(1_024);
    let core = AppCore::start(outbound);

    let CommandResult::Providers { providers } =
        core.execute(Command::ListProviders).expect("list")
    else {
        panic!("providers result expected");
    };
    assert!(!providers.is_empty());

    let CommandResult::Instruments { instruments } = core
        .execute(Command::SearchInstruments {
            query: String::new(),
            limit: 5,
        })
        .expect("search")
    else {
        panic!("instruments result expected");
    };
    let instrument = instruments.first().expect("at least one instrument");

    core.execute(Command::Subscribe {
        instrument_id: instrument.instrument_id.clone(),
        interval: IntervalLabel("1m".to_string()),
    })
    .expect("subscribe");

    let deadline = Instant::now() + Duration::from_secs(45);
    let mut book_ready = false;
    let mut bars_seeded = false;
    let mut session_live = false;
    while Instant::now() < deadline && !(book_ready && bars_seeded && session_live) {
        let Ok(bytes) = frames.recv_timeout(Duration::from_millis(200)) else {
            continue;
        };
        let Some(frame) = binary::parse_frame(&bytes) else {
            continue;
        };
        match frame.kind {
            binary::KIND_BOOK_SNAPSHOT => {
                if let Some(snapshot) = binary::decode_book_snapshot(frame.payload) {
                    book_ready |= snapshot.head.state == 1
                        && !snapshot.bids.is_empty()
                        && !snapshot.asks.is_empty();
                }
            }
            binary::KIND_BARS_SNAPSHOT => {
                if let Some(snapshot) = binary::decode_bars_snapshot(frame.payload) {
                    bars_seeded |= !snapshot.closed.is_empty();
                }
            }
            binary::KIND_SESSION => {
                if let Some(session) = binary::decode_session(frame.payload) {
                    session_live |= session.state == 1;
                }
            }
            _ => {}
        }
    }

    core.execute(Command::Unsubscribe {
        instrument_id: instrument.instrument_id.clone(),
    })
    .expect("unsubscribe");
    core.shutdown();

    assert!(session_live, "session never went live");
    assert!(book_ready, "no ready book image arrived");
    assert!(bars_seeded, "no seeded history arrived");
}
