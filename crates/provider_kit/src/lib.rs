//! Shared adapter building blocks so adapters stay mostly translation.
//!
//! Transport is injected: this kit provides the native WebSocket and HTTP
//! clients; a browser build substitutes the platform's own transports behind
//! the same adapter contract.

pub mod clock;
pub mod http;
pub mod rate_budget;
pub mod ws;

pub use clock::unix_nanos_now;
pub use rate_budget::{BudgetError, RateBudget, RateBudgetConfig, RequestPriority};
pub use ws::{WebSocket, WsMessage};
