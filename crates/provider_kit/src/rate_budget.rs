//! Shared request-weight budget: a trailing window with a foreground reserve.
//!
//! Every consumer of one provider's REST endpoint draws from one budget, so
//! together they wait for capacity instead of being rejected with HTTP 429
//! and retrying into the limit. Background work spends only below the reserve
//! so a long scan never makes a chart or the catalog wait.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Longest single sleep, so cancellation is observed promptly.
const BUDGET_POLL: Duration = Duration::from_millis(100);

/// Window shape declared by a provider capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateBudgetConfig {
    pub window: Duration,
    pub window_weight: u32,
    pub foreground_reserve: u32,
}

/// Who waits for a request's weight.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RequestPriority {
    /// A request a user is waiting on, such as a chart's history or the catalog.
    #[default]
    Interactive,
    /// Bulk work that yields to interactive requests, such as a market scan.
    Background,
}

/// A bounded wait that did not end in capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BudgetError {
    #[error("request was cancelled while waiting for budget")]
    Cancelled,
    #[error("request budget is exhausted")]
    Exhausted,
}

/// One provider's shared trailing-window weight budget.
pub struct RateBudget {
    config: RateBudgetConfig,
    window: Mutex<WeightWindow>,
}

impl RateBudget {
    #[must_use]
    pub const fn new(config: RateBudgetConfig) -> Self {
        Self {
            config,
            window: Mutex::new(WeightWindow::new()),
        }
    }

    /// Spends `weight`, waiting while earlier spending leaves no capacity for
    /// this priority, up to `maximum_wait`.
    ///
    /// # Errors
    ///
    /// Returns `Cancelled` when `stop` is raised while waiting, or `Exhausted`
    /// when the budget cannot cover the request within the bounded wait.
    pub fn spend(
        &self,
        weight: u32,
        priority: RequestPriority,
        stop: Option<&AtomicBool>,
        maximum_wait: Duration,
    ) -> Result<(), BudgetError> {
        let limit = match priority {
            RequestPriority::Interactive => self.config.window_weight,
            RequestPriority::Background => self
                .config
                .window_weight
                .saturating_sub(self.config.foreground_reserve),
        };
        let started = Instant::now();
        loop {
            let now = Instant::now();
            let Err(wait) = self
                .lock()
                .try_spend(weight, limit, now, self.config.window)
            else {
                return Ok(());
            };
            if stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
                return Err(BudgetError::Cancelled);
            }
            if now.saturating_duration_since(started) + wait > maximum_wait {
                return Err(BudgetError::Exhausted);
            }
            std::thread::sleep(wait.min(BUDGET_POLL));
        }
    }

    /// Charges weight outside the spend path, such as a returned page whose
    /// cost depends on its size.
    pub fn charge(&self, weight: u32) {
        self.lock().record(weight, Instant::now());
    }

    /// The provider rejected a request for exceeding its limit, which other
    /// software on the address may have spent: treat the window as full.
    pub fn provider_limited(&self) {
        let now = Instant::now();
        let mut window = self.lock();
        window.expire(now, self.config.window);
        let missing = self.config.window_weight.saturating_sub(window.total);
        window.record(missing, now);
    }

    fn lock(&self) -> MutexGuard<'_, WeightWindow> {
        // The window holds only counters, so a panic elsewhere leaves it usable.
        self.window.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Weight spent during the trailing window, oldest first.
struct WeightWindow {
    spent: VecDeque<(Instant, u32)>,
    total: u32,
}

impl WeightWindow {
    const fn new() -> Self {
        Self {
            spent: VecDeque::new(),
            total: 0,
        }
    }

    /// Spends `weight` at `now` when the window total stays at or below
    /// `limit`, or returns how long until enough earlier spending expires.
    fn try_spend(
        &mut self,
        weight: u32,
        limit: u32,
        now: Instant,
        window: Duration,
    ) -> Result<(), Duration> {
        self.expire(now, window);
        if self.total.saturating_add(weight) <= limit {
            self.record(weight, now);
            return Ok(());
        }
        let mut remaining = self.total;
        for &(at, spent) in &self.spent {
            remaining = remaining.saturating_sub(spent);
            if remaining.saturating_add(weight) <= limit {
                return Err((at + window).saturating_duration_since(now));
            }
        }
        Err(window)
    }

    fn record(&mut self, weight: u32, now: Instant) {
        if weight > 0 {
            self.spent.push_back((now, weight));
            self.total = self.total.saturating_add(weight);
        }
    }

    fn expire(&mut self, now: Instant, window: Duration) {
        while let Some(&(at, weight)) = self.spent.front() {
            if now.saturating_duration_since(at) < window {
                break;
            }
            self.spent.pop_front();
            self.total = self.total.saturating_sub(weight);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: RateBudgetConfig = RateBudgetConfig {
        window: Duration::from_secs(60),
        window_weight: 1_000,
        foreground_reserve: 300,
    };
    const UNIT: u32 = 20;

    #[test]
    fn a_full_window_waits_for_its_oldest_spend_to_expire() {
        let start = Instant::now();
        let mut window = WeightWindow::new();
        for _ in 0..CONFIG.window_weight / UNIT {
            assert_eq!(
                window.try_spend(UNIT, CONFIG.window_weight, start, CONFIG.window),
                Ok(())
            );
        }
        let later = start + Duration::from_secs(10);
        assert_eq!(
            window.try_spend(UNIT, CONFIG.window_weight, later, CONFIG.window),
            Err(Duration::from_secs(50))
        );
        assert_eq!(
            window.try_spend(
                UNIT,
                CONFIG.window_weight,
                start + CONFIG.window,
                CONFIG.window
            ),
            Ok(())
        );
        assert_eq!(window.total, UNIT);
    }

    #[test]
    fn provider_rejections_hold_the_window() {
        let budget = RateBudget::new(CONFIG);
        budget.provider_limited();
        assert_eq!(
            budget.spend(UNIT, RequestPriority::Interactive, None, Duration::ZERO),
            Err(BudgetError::Exhausted)
        );
    }

    #[test]
    fn background_requests_leave_the_foreground_reserve_unspent() {
        let start = Instant::now();
        let mut window = WeightWindow::new();
        let background = CONFIG.window_weight - CONFIG.foreground_reserve;
        for _ in 0..background / UNIT {
            assert_eq!(
                window.try_spend(UNIT, background, start, CONFIG.window),
                Ok(())
            );
        }
        let later = start + Duration::from_secs(5);
        assert!(
            window
                .try_spend(UNIT, background, later, CONFIG.window)
                .is_err()
        );
        for _ in 0..CONFIG.foreground_reserve / UNIT {
            assert_eq!(
                window.try_spend(UNIT, CONFIG.window_weight, later, CONFIG.window),
                Ok(())
            );
        }
        assert!(
            window
                .try_spend(UNIT, CONFIG.window_weight, later, CONFIG.window)
                .is_err()
        );
    }

    #[test]
    fn waiting_ends_on_cancellation_or_when_the_wait_exceeds_its_bound() {
        let budget = RateBudget::new(CONFIG);
        budget.provider_limited();
        let stop = AtomicBool::new(true);
        assert_eq!(
            budget.spend(
                UNIT,
                RequestPriority::Interactive,
                Some(&stop),
                CONFIG.window
            ),
            Err(BudgetError::Cancelled)
        );
        assert_eq!(
            budget.spend(
                UNIT,
                RequestPriority::Interactive,
                None,
                Duration::from_secs(1)
            ),
            Err(BudgetError::Exhausted)
        );
    }
}
