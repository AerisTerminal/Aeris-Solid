//! Command service: routes typed UI intent to the registered providers and
//! the market runtime, owns the demand lease and history seeding.

use crate::providers::RegisteredProvider;
use crate::pump::PumpCommand;
use aeris_app_protocol::{
    BarView, Command, CommandError, CommandErrorKind, CommandResult, InstrumentView, IntervalLabel,
    ProviderView, RateBudgetView, StreamsView, TargetsView, asset_class_label,
};
use aeris_domain::{ChartInterval, InstrumentDescriptor, MarketBar};
use aeris_market_runtime::MarketRuntime;
use aeris_provider_api::{ProviderError, SignInMethod, StreamSet};
use aeris_provider_kit::unix_nanos_now;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::{Duration, Instant};

/// One UI command with its reply channel.
pub(crate) struct UiRequest {
    pub command: Command,
    pub reply: std::sync::mpsc::Sender<Result<CommandResult, CommandError>>,
}

/// Largest catalog page asked of any provider during resolution.
const RESOLUTION_PAGE: usize = 500;
/// Closed bars seeded when a stream opens.
const SEED_BARS: usize = 500;
/// Resolved instruments retained before the cache resets.
const MAXIMUM_RESOLVED: usize = 4_096;
/// Failed reseeds retried after this delay, at most 16 pending.
const RESEED_RETRY: Duration = Duration::from_secs(2);
const MAXIMUM_PENDING_RESEEDS: usize = 16;
const POLL: Duration = Duration::from_millis(50);

struct Subscription {
    provider_index: usize,
    descriptor: InstrumentDescriptor,
    interval: ChartInterval,
}

struct Service {
    providers: Vec<RegisteredProvider>,
    runtimes: HashMap<usize, Arc<MarketRuntime>>,
    subscriptions: HashMap<String, Subscription>,
    resolved: HashMap<String, (usize, InstrumentDescriptor)>,
    pending_reseeds: Vec<(String, ChartInterval, Instant)>,
    pump_commands: SyncSender<PumpCommand>,
    shutdown: Arc<AtomicBool>,
}

pub(crate) fn run(
    requests: Receiver<UiRequest>,
    reseeds: Receiver<(String, ChartInterval)>,
    pump_commands: SyncSender<PumpCommand>,
    providers: Vec<RegisteredProvider>,
    runtimes: HashMap<usize, Arc<MarketRuntime>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut service = Service {
        providers,
        runtimes,
        subscriptions: HashMap::new(),
        resolved: HashMap::new(),
        pending_reseeds: Vec::new(),
        pump_commands,
        shutdown,
    };
    loop {
        if service.shutdown.load(Ordering::Acquire) {
            break;
        }
        match requests.recv_timeout(POLL) {
            Ok(request) => {
                let result = service.handle(request.command);
                let _ = request.reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok((instrument_id, interval)) = reseeds.try_recv() {
            service.queue_reseed(instrument_id, interval, Instant::now());
        }
        service.process_due_reseeds();
    }
}

impl Service {
    fn handle(&mut self, command: Command) -> Result<CommandResult, CommandError> {
        match command {
            Command::ListProviders => Ok(CommandResult::Providers {
                providers: self.providers.iter().map(provider_view).collect(),
            }),
            Command::SearchInstruments { query, limit } => self.search(&query, limit),
            Command::Subscribe {
                instrument_id,
                interval,
            } => self.subscribe(&instrument_id, &interval),
            Command::Unsubscribe { instrument_id } => self.unsubscribe(&instrument_id),
            Command::History {
                instrument_id,
                interval,
                limit,
            } => self.history(&instrument_id, &interval, limit),
        }
    }

    fn search(&mut self, query: &str, limit: u32) -> Result<CommandResult, CommandError> {
        if limit == 0 {
            return Err(invalid("limit must be positive"));
        }
        let limit = usize::try_from(limit.min(200)).unwrap_or(200);
        let mut found: Vec<(usize, InstrumentDescriptor)> = Vec::new();
        let mut failure = None;
        for (index, provider) in self.providers.iter().enumerate() {
            let Some(catalog) = &provider.catalog else {
                continue;
            };
            match catalog.search(query, limit) {
                Ok(descriptors) => {
                    found.extend(
                        descriptors
                            .into_iter()
                            .map(|descriptor| (index, descriptor)),
                    );
                }
                Err(error) => failure = Some(error),
            }
        }
        let mut views: Vec<InstrumentView> = Vec::new();
        for (index, descriptor) in found {
            self.remember(index, &descriptor);
            views.push(InstrumentView::from(&descriptor));
        }
        if views.is_empty()
            && let Some(error) = failure
        {
            return Err(command_error(error));
        }
        views.truncate(limit);
        Ok(CommandResult::Instruments { instruments: views })
    }

    fn subscribe(
        &mut self,
        instrument_id: &str,
        label: &IntervalLabel,
    ) -> Result<CommandResult, CommandError> {
        let interval = label
            .parse()
            .ok_or_else(|| invalid("unknown chart interval"))?;
        let (index, descriptor) = self.resolve(instrument_id)?;
        let provider = &self.providers[index];
        if !provider.capabilities.history_intervals.contains(&interval) {
            return Err(CommandError::new(
                CommandErrorKind::UnsupportedInterval,
                "provider does not offer this interval",
            ));
        }
        let bars = self.fetch_history(index, &descriptor, interval, SEED_BARS)?;
        if let Some(runtime) = self.runtimes.get(&index) {
            runtime.seed_bars(instrument_id, interval, bars.clone());
        }
        self.send_pump(PumpCommand::Drop {
            instrument_id: instrument_id.to_string(),
        })?;
        self.send_pump(PumpCommand::SeedBars {
            instrument_id: instrument_id.to_string(),
            interval,
            bars,
        })?;
        self.subscriptions.insert(
            instrument_id.to_string(),
            Subscription {
                provider_index: index,
                descriptor,
                interval,
            },
        );
        self.rebuild_demand(index);
        Ok(CommandResult::Subscribed {
            instrument_id: instrument_id.to_string(),
        })
    }

    fn unsubscribe(&mut self, instrument_id: &str) -> Result<CommandResult, CommandError> {
        if let Some(subscription) = self.subscriptions.remove(instrument_id) {
            self.rebuild_demand(subscription.provider_index);
        }
        self.send_pump(PumpCommand::Drop {
            instrument_id: instrument_id.to_string(),
        })?;
        Ok(CommandResult::Unsubscribed {
            instrument_id: instrument_id.to_string(),
        })
    }

    fn history(
        &mut self,
        instrument_id: &str,
        label: &IntervalLabel,
        limit: u32,
    ) -> Result<CommandResult, CommandError> {
        if limit == 0 {
            return Err(invalid("limit must be positive"));
        }
        let interval = label
            .parse()
            .ok_or_else(|| invalid("unknown chart interval"))?;
        let (index, descriptor) = self.resolve(instrument_id)?;
        let limit = usize::try_from(limit.min(5_000)).unwrap_or(5_000);
        let bars = self.fetch_history(index, &descriptor, interval, limit)?;
        Ok(CommandResult::History {
            bars: bars.iter().map(BarView::from).collect(),
        })
    }

    fn fetch_history(
        &self,
        index: usize,
        descriptor: &InstrumentDescriptor,
        interval: ChartInterval,
        limit: usize,
    ) -> Result<Vec<MarketBar>, CommandError> {
        let history = self.providers[index]
            .history
            .as_ref()
            .ok_or_else(|| invalid("provider offers no history"))?;
        history
            .bars(descriptor, interval, 0, unix_nanos_now(), limit)
            .map_err(command_error)
    }

    fn resolve(
        &mut self,
        instrument_id: &str,
    ) -> Result<(usize, InstrumentDescriptor), CommandError> {
        if let Some((index, descriptor)) = self.resolved.get(instrument_id) {
            return Ok((*index, descriptor.clone()));
        }
        let mut failure = None;
        let mut listed: Vec<(usize, InstrumentDescriptor)> = Vec::new();
        for (index, provider) in self.providers.iter().enumerate() {
            let Some(catalog) = &provider.catalog else {
                continue;
            };
            match catalog.search("", RESOLUTION_PAGE) {
                Ok(descriptors) => {
                    listed.extend(
                        descriptors
                            .into_iter()
                            .map(|descriptor| (index, descriptor)),
                    );
                }
                Err(error) => failure = Some(error),
            }
        }
        for (index, descriptor) in listed {
            self.remember(index, &descriptor);
        }
        if let Some((index, descriptor)) = self.resolved.get(instrument_id) {
            return Ok((*index, descriptor.clone()));
        }
        if let Some(error) = failure {
            return Err(command_error(error));
        }
        Err(CommandError::new(
            CommandErrorKind::UnknownInstrument,
            "no registered catalog lists this instrument",
        ))
    }

    fn remember(&mut self, index: usize, descriptor: &InstrumentDescriptor) {
        if self.resolved.len() >= MAXIMUM_RESOLVED {
            self.resolved.clear();
        }
        self.resolved.insert(
            descriptor.instrument_id.as_str().to_string(),
            (index, descriptor.clone()),
        );
    }

    fn rebuild_demand(&self, index: usize) {
        let Some(runtime) = self.runtimes.get(&index) else {
            return;
        };
        let streams = self.providers[index].capabilities.streams;
        let subscriptions = self
            .subscriptions
            .values()
            .filter(|subscription| subscription.provider_index == index)
            .map(|subscription| aeris_provider_api::InstrumentSubscription {
                instrument: subscription.descriptor.clone(),
                streams: StreamSet {
                    trades: streams.trades,
                    quotes: streams.quotes,
                    depth: streams.aggregate_depth_levels.is_some(),
                    candle: Some(subscription.interval),
                },
            })
            .collect();
        runtime.set_demand(subscriptions);
    }

    fn send_pump(&self, command: PumpCommand) -> Result<(), CommandError> {
        let mut command = command;
        let started = Instant::now();
        loop {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(invalid("runtime is shutting down"));
            }
            match self.pump_commands.try_send(command) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(returned)) => {
                    if started.elapsed() >= Duration::from_secs(2) {
                        return Err(CommandError::new(
                            CommandErrorKind::Internal,
                            "stream pump stayed saturated",
                        ));
                    }
                    command = returned;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(CommandError::new(
                        CommandErrorKind::Internal,
                        "stream pump is gone",
                    ));
                }
            }
        }
    }

    fn queue_reseed(&mut self, instrument_id: String, interval: ChartInterval, when: Instant) {
        if self
            .pending_reseeds
            .iter()
            .any(|(id, candidate, _)| *id == instrument_id && *candidate == interval)
        {
            return;
        }
        if self.pending_reseeds.len() >= MAXIMUM_PENDING_RESEEDS {
            self.pending_reseeds.remove(0);
        }
        self.pending_reseeds.push((instrument_id, interval, when));
    }

    fn process_due_reseeds(&mut self) {
        let now = Instant::now();
        let mut due = Vec::new();
        self.pending_reseeds.retain(|(id, interval, when)| {
            if *when <= now {
                due.push((id.clone(), *interval));
                false
            } else {
                true
            }
        });
        for (instrument_id, interval) in due {
            let Some(subscription) = self.subscriptions.get(&instrument_id) else {
                continue;
            };
            let index = subscription.provider_index;
            let descriptor = subscription.descriptor.clone();
            match self.fetch_history(index, &descriptor, interval, SEED_BARS) {
                Ok(bars) => {
                    if let Some(runtime) = self.runtimes.get(&index) {
                        runtime.seed_bars(&instrument_id, interval, bars.clone());
                    }
                    if self
                        .send_pump(PumpCommand::SeedBars {
                            instrument_id: instrument_id.clone(),
                            interval,
                            bars,
                        })
                        .is_err()
                    {
                        self.queue_reseed(instrument_id, interval, Instant::now() + RESEED_RETRY);
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error.message, instrument = %instrument_id, "history reseed failed");
                    self.queue_reseed(instrument_id, interval, Instant::now() + RESEED_RETRY);
                }
            }
        }
    }
}

fn provider_view(provider: &RegisteredProvider) -> ProviderView {
    let capabilities = &provider.capabilities;
    ProviderView {
        provider_id: capabilities.provider_id.clone(),
        display_name: capabilities.display_name.clone(),
        asset_classes: capabilities
            .asset_classes
            .iter()
            .map(|class| asset_class_label(*class).to_string())
            .collect(),
        streams: StreamsView {
            trades: capabilities.streams.trades,
            quotes: capabilities.streams.quotes,
            aggregate_depth_levels: capabilities
                .streams
                .aggregate_depth_levels
                .map(|levels| u32::try_from(levels).unwrap_or(u32::MAX)),
            order_level_instrument_cap: capabilities
                .streams
                .order_level_instrument_cap
                .map(|cap| u32::try_from(cap).unwrap_or(u32::MAX)),
        },
        history_intervals: capabilities
            .history_intervals
            .iter()
            .map(|interval| IntervalLabel::of(*interval))
            .collect(),
        sign_in: match capabilities.sign_in {
            SignInMethod::None => "none".to_string(),
            SignInMethod::ApiKey => "apiKey".to_string(),
            SignInMethod::VaultLogin => "vaultLogin".to_string(),
        },
        trading_venue: capabilities.trading_venue,
        rate_budget: RateBudgetView {
            window_seconds: capabilities.rate_budget.window_seconds,
            window_weight: capabilities.rate_budget.window_weight,
            foreground_reserve: capabilities.rate_budget.foreground_reserve,
        },
        targets: TargetsView {
            native: capabilities.targets.native,
            web: capabilities.targets.web,
        },
    }
}

fn invalid(message: &str) -> CommandError {
    CommandError::new(CommandErrorKind::InvalidRequest, message)
}

fn command_error(error: ProviderError) -> CommandError {
    let kind = match error {
        ProviderError::RateLimited { .. } => CommandErrorKind::RateLimited,
        _ => CommandErrorKind::ProviderUnavailable,
    };
    CommandError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeris_domain::{AssetClass, InstrumentId};
    use aeris_provider_api::{
        BuildTargets, FeedEvent, FeedEventSink, FeedSessionSpec, MarketFeedProvider,
        ProviderCapabilities, RateBudgetDeclaration, StreamCapabilities,
    };
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    fn descriptor() -> InstrumentDescriptor {
        InstrumentDescriptor {
            instrument_id: InstrumentId::new("instrument:fixture:BTC").expect("identity"),
            provider_id: "fixture".to_string(),
            symbol: "BTC".to_string(),
            display: "BTC-USDC".to_string(),
            exchange: "Fixture".to_string(),
            asset_class: AssetClass::CryptoPerp,
            price_scale: 8,
            quantity_scale: 8,
            price_increment: Some(1),
            quantity_increment: Some(1),
        }
    }

    fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider_id: "fixture".to_string(),
            display_name: "Fixture".to_string(),
            asset_classes: vec![AssetClass::CryptoPerp],
            streams: StreamCapabilities {
                trades: true,
                quotes: true,
                aggregate_depth_levels: Some(20),
                order_level_instrument_cap: None,
            },
            history_intervals: vec![ChartInterval::Minute1],
            sign_in: SignInMethod::None,
            trading_venue: false,
            rate_budget: RateBudgetDeclaration {
                window_seconds: 60,
                window_weight: 1_000,
                foreground_reserve: 300,
            },
            targets: BuildTargets {
                native: true,
                web: false,
            },
        }
    }

    struct FakeCatalog;

    impl aeris_provider_api::Catalog for FakeCatalog {
        fn search(
            &self,
            query: &str,
            limit: usize,
        ) -> Result<Vec<InstrumentDescriptor>, ProviderError> {
            let instrument = descriptor();
            let matches = query.is_empty() || instrument.symbol.contains(query);
            Ok(if matches {
                vec![instrument].into_iter().take(limit).collect()
            } else {
                Vec::new()
            })
        }
    }

    struct FakeHistory;

    impl aeris_provider_api::History for FakeHistory {
        fn bars(
            &self,
            _instrument: &InstrumentDescriptor,
            _interval: ChartInterval,
            _start_unix_nanos: i64,
            _end_unix_nanos: i64,
            limit: usize,
        ) -> Result<Vec<MarketBar>, ProviderError> {
            Ok(vec![MarketBar {
                source_sequence: 1,
                exchange_timestamp_seconds: 60,
                exchange_timestamp_unix_nanos: 60_000_000_000,
                open: 100,
                high: 110,
                low: 90,
                close: 105,
                volume: 10,
            }]
            .into_iter()
            .take(limit)
            .collect())
        }
    }

    struct FakeFeed {
        specs: Mutex<Vec<FeedSessionSpec>>,
        ended: std::sync::atomic::AtomicUsize,
    }

    impl MarketFeedProvider for FakeFeed {
        fn run_session(&self, spec: FeedSessionSpec, sink: FeedEventSink, stop: Arc<AtomicBool>) {
            self.specs.lock().expect("specs").push(spec);
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
            }
            self.ended.fetch_add(1, Ordering::Release);
            let _ = sink.send(FeedEvent::Ended(ProviderError::Closed));
        }
    }

    fn service() -> (Service, Arc<FakeFeed>, mpsc::Receiver<PumpCommand>) {
        let feed = Arc::new(FakeFeed {
            specs: Mutex::new(Vec::new()),
            ended: std::sync::atomic::AtomicUsize::new(0),
        });
        let (publisher, _publications) = mpsc::sync_channel(64);
        let runtime = Arc::new(MarketRuntime::start(
            feed.clone(),
            aeris_market_runtime::RuntimeConfig::default(),
            publisher,
        ));
        let (pump_commands, pump_receiver) = mpsc::sync_channel(16);
        let service = Service {
            providers: vec![RegisteredProvider {
                capabilities: capabilities(),
                catalog: Some(Arc::new(FakeCatalog)),
                history: Some(Arc::new(FakeHistory)),
                feed: Some(feed.clone()),
            }],
            runtimes: HashMap::from([(0, runtime)]),
            subscriptions: HashMap::new(),
            resolved: HashMap::new(),
            pending_reseeds: Vec::new(),
            pump_commands,
            shutdown: Arc::new(AtomicBool::new(false)),
        };
        (service, feed, pump_receiver)
    }

    #[test]
    fn subscribe_seeds_history_and_opens_the_demand_lease() {
        let (mut service, feed, pump_receiver) = service();
        let result = service
            .handle(Command::Subscribe {
                instrument_id: "instrument:fixture:BTC".to_string(),
                interval: IntervalLabel("1m".to_string()),
            })
            .expect("subscribe");
        assert!(matches!(result, CommandResult::Subscribed { .. }));
        assert!(matches!(
            pump_receiver.try_recv(),
            Ok(PumpCommand::Drop { .. })
        ));
        let Ok(PumpCommand::SeedBars { bars, .. }) = pump_receiver.try_recv() else {
            panic!("seed command expected");
        };
        assert_eq!(bars.len(), 1);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let recorded = feed.specs.lock().expect("specs").len();
            if recorded >= 1 {
                break;
            }
            assert!(Instant::now() < deadline, "demand never reached the feed");
            std::thread::sleep(Duration::from_millis(10));
        }
        let specs = feed.specs.lock().expect("specs");
        let spec = specs.last().expect("spec");
        assert_eq!(spec.subscriptions.len(), 1);
        assert!(spec.subscriptions[0].streams.depth);
        assert_eq!(
            spec.subscriptions[0].streams.candle,
            Some(ChartInterval::Minute1)
        );
    }

    #[test]
    fn unknown_instruments_and_intervals_fail_with_typed_kinds() {
        let (mut service, _feed, _pump) = service();
        let error = service
            .handle(Command::Subscribe {
                instrument_id: "instrument:fixture:ETH".to_string(),
                interval: IntervalLabel("1m".to_string()),
            })
            .expect_err("unknown instrument");
        assert_eq!(error.kind, CommandErrorKind::UnknownInstrument);
        let error = service
            .handle(Command::Subscribe {
                instrument_id: "instrument:fixture:BTC".to_string(),
                interval: IntervalLabel("5m".to_string()),
            })
            .expect_err("unsupported interval");
        assert_eq!(error.kind, CommandErrorKind::UnsupportedInterval);
        let error = service
            .handle(Command::Subscribe {
                instrument_id: "instrument:fixture:BTC".to_string(),
                interval: IntervalLabel("45s".to_string()),
            })
            .expect_err("invalid label");
        assert_eq!(error.kind, CommandErrorKind::InvalidRequest);
    }

    #[test]
    fn history_and_provider_listing_use_the_declared_surface() {
        let (mut service, _feed, _pump) = service();
        let Ok(CommandResult::History { bars }) = service.handle(Command::History {
            instrument_id: "instrument:fixture:BTC".to_string(),
            interval: IntervalLabel("1m".to_string()),
            limit: 10,
        }) else {
            panic!("history result expected");
        };
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].close, 105);
        let Ok(CommandResult::Providers { providers }) = service.handle(Command::ListProviders)
        else {
            panic!("providers result expected");
        };
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].sign_in, "none");
        assert_eq!(providers[0].streams.aggregate_depth_levels, Some(20));
        assert_eq!(
            providers[0].history_intervals,
            vec![IntervalLabel("1m".to_string())]
        );
        let Ok(CommandResult::Instruments { instruments: found }) =
            service.handle(Command::SearchInstruments {
                query: "BTC".to_string(),
                limit: 10,
            })
        else {
            panic!("search result expected");
        };
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].display, "BTC-USDC");
    }

    #[test]
    fn unsubscribe_closes_the_demand_lease() {
        let (mut service, feed, pump_receiver) = service();
        service
            .handle(Command::Subscribe {
                instrument_id: "instrument:fixture:BTC".to_string(),
                interval: IntervalLabel("1m".to_string()),
            })
            .expect("subscribe");
        while pump_receiver.try_recv().is_ok() {}
        service
            .handle(Command::Unsubscribe {
                instrument_id: "instrument:fixture:BTC".to_string(),
            })
            .expect("unsubscribe");
        assert!(matches!(
            pump_receiver.try_recv(),
            Ok(PumpCommand::Drop { .. })
        ));
        assert!(service.subscriptions.is_empty());
        // Either no session ever started, or the one that started ended.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let started = feed.specs.lock().expect("specs").len();
            let ended = feed.ended.load(Ordering::Acquire);
            if started == 0 || ended >= 1 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "session never ended after unsubscribe"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
