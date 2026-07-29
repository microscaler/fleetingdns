//! Usage metering seam for future billing.
//!
//! # Nothing is metered today
//!
//! This module defines *where* billing would attach and *what shape* the data
//! would take. It deliberately does not collect, persist or transmit anything:
//! the default [`NoopMeter`] discards every event. Turning metering on is a
//! product decision that requires disclosure to users and their consent — it
//! must be made deliberately, not inherited from code left lying around.
//!
//! # Why lifecycle, not traffic
//!
//! Events describe a tunnel's *lifecycle* — it opened, it closed — and never
//! the traffic flowing through it. Billing on tunnel-hours needs only these
//! two moments.
//!
//! Counting bytes is technically easy (the router's `copy_bidirectional`
//! returns totals as a by-product of moving data, without inspecting content)
//! but it is the wrong thing to build here. Per-tunnel, per-user traffic
//! counters are a durable record of when a named user worked and how much they
//! moved, and FleetingDNS's security model runs the other way: FR-EDGE-1
//! refuses per-subdomain certificates specifically to keep tunnel FQDNs out of
//! Certificate Transparency logs. Accumulating usage records for those same
//! tunnels would undo that reasoning from the other side.
//!
//! If a future plan genuinely requires traffic-based billing, it should be a
//! separate, explicit, disclosed opt-in — not an extension of this trait.
//!
//! # Recording must never break a tunnel
//!
//! [`UsageMeter::record`] is synchronous and returns nothing, so a meter cannot
//! block, fail, or slow down tunnel creation or teardown. Implementations that
//! need to do I/O should queue internally (for example, hand off to a channel
//! drained by a background task) and drop events rather than apply
//! backpressure to the request path. A billing outage must never become a
//! tunnelling outage.

use chrono::{DateTime, Utc};

/// Why a tunnel stopped, recorded when it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// The owner deleted the tunnel.
    UserRequested,
    /// The tunnel reached its expiry.
    Expired,
}

/// A billable moment in a tunnel's life.
///
/// Carries the identifiers a billing system needs to attribute the tunnel, and
/// nothing about what travelled through it.
#[derive(Debug, Clone)]
pub enum UsageEvent {
    /// A tunnel was created and is now billable.
    TunnelOpened {
        tunnel_id: String,
        /// Account the tunnel belongs to.
        user_id: String,
        at: DateTime<Utc>,
    },
    /// A tunnel stopped being billable. Paired with a preceding
    /// [`UsageEvent::TunnelOpened`], the two bound a billable interval.
    TunnelClosed {
        tunnel_id: String,
        user_id: String,
        at: DateTime<Utc>,
        reason: CloseReason,
    },
}

impl UsageEvent {
    /// The tunnel this event concerns.
    pub fn tunnel_id(&self) -> &str {
        match self {
            UsageEvent::TunnelOpened { tunnel_id, .. }
            | UsageEvent::TunnelClosed { tunnel_id, .. } => tunnel_id,
        }
    }

    /// The account this event is attributable to.
    pub fn user_id(&self) -> &str {
        match self {
            UsageEvent::TunnelOpened { user_id, .. } | UsageEvent::TunnelClosed { user_id, .. } => {
                user_id
            }
        }
    }
}

/// Sink for [`UsageEvent`]s.
///
/// Implementations must be cheap and infallible from the caller's point of
/// view — see the module docs on why `record` cannot fail or block.
pub trait UsageMeter: Send + Sync + std::fmt::Debug {
    /// Record an event. Implementations that cannot keep up should drop
    /// events rather than block the caller.
    fn record(&self, event: UsageEvent);
}

/// The default meter: discards every event.
///
/// This is what runs unless a deployment deliberately configures something
/// else, so the system records no usage data by default.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopMeter;

impl UsageMeter for NoopMeter {
    fn record(&self, _event: UsageEvent) {}
}

/// A meter that keeps events in memory.
///
/// For tests and local development only — it grows without bound and persists
/// nothing. Not a billing implementation.
#[derive(Debug, Default)]
pub struct CollectingMeter {
    events: std::sync::Mutex<Vec<UsageEvent>>,
}

impl CollectingMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every event recorded so far, in order.
    pub fn events(&self) -> Vec<UsageEvent> {
        self.events.lock().expect("meter mutex poisoned").clone()
    }

    /// Number of events recorded.
    pub fn len(&self) -> usize {
        self.events.lock().expect("meter mutex poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl UsageMeter for CollectingMeter {
    fn record(&self, event: UsageEvent) {
        self.events
            .lock()
            .expect("meter mutex poisoned")
            .push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn opened(id: &str) -> UsageEvent {
        UsageEvent::TunnelOpened {
            tunnel_id: id.to_string(),
            user_id: "user-1".to_string(),
            at: Utc::now(),
        }
    }

    /// The default must record nothing: no usage data unless a deployment
    /// deliberately opts in.
    #[test]
    fn noop_meter_records_nothing() {
        let meter = NoopMeter;
        meter.record(opened("t-1"));
        // Nothing to assert beyond the absence of any sink — the point is that
        // NoopMeter holds no state and can therefore leak nothing.
        assert_eq!(std::mem::size_of::<NoopMeter>(), 0);
    }

    #[test]
    fn collecting_meter_captures_events_in_order() {
        let meter = CollectingMeter::new();
        meter.record(opened("t-1"));
        meter.record(UsageEvent::TunnelClosed {
            tunnel_id: "t-1".to_string(),
            user_id: "user-1".to_string(),
            at: Utc::now(),
            reason: CloseReason::UserRequested,
        });

        let events = meter.events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], UsageEvent::TunnelOpened { .. }));
        assert!(matches!(
            events[1],
            UsageEvent::TunnelClosed {
                reason: CloseReason::UserRequested,
                ..
            }
        ));
    }

    #[test]
    fn accessors_expose_attribution_only() {
        let event = opened("t-42");
        assert_eq!(event.tunnel_id(), "t-42");
        assert_eq!(event.user_id(), "user-1");
    }

    /// The seam must be usable as a trait object, so a deployment can swap the
    /// implementation without threading a generic through the API state.
    #[test]
    fn meter_is_object_safe() {
        let meter: Arc<dyn UsageMeter> = Arc::new(CollectingMeter::new());
        meter.record(opened("t-dyn"));
        assert!(!format!("{meter:?}").is_empty());
    }
}
