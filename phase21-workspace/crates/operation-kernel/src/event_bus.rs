use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use aethercore_contracts::v1::{EventEnvelope, EventKind};
use chrono::Utc;

const SUBSCRIBER_CAPACITY: usize = 256;
const MAX_OWNER_STREAMS: usize = 128;

#[derive(Clone, Debug)]
pub struct PublishedEvent {
    pub owner_principal_key: String,
    pub envelope: EventEnvelope,
}

#[derive(Default)]
struct OwnerStream {
    last_activity_tick: u64,
    sequence: u64,
    dropped_through_sequence: u64,
    subscriber_lag_total: u64,
    subscriber_disconnect_total: u64,
    replay: VecDeque<EventEnvelope>,
    subscribers: Vec<Subscriber>,
}

#[derive(Default)]
struct BusState {
    activity_clock: u64,
    owners: HashMap<String, OwnerStream>,
}

fn owner_stream_mut<'a>(state: &'a mut BusState, owner_principal_key: &str) -> &'a mut OwnerStream {
    if !state.owners.contains_key(owner_principal_key) {
        // Keep replay state bounded across months of distinct Windows logon identities. Active
        // subscriptions are never evicted. If an old inactive owner returns after eviction, its
        // prior sequence is beyond the fresh stream and the normal StreamReset + hydration path
        // re-establishes deterministic state rather than pretending replay continuity.
        while state.owners.len() >= MAX_OWNER_STREAMS {
            let candidate = state
                .owners
                .iter()
                .filter(|(_, stream)| stream.subscribers.is_empty())
                .min_by_key(|(_, stream)| stream.last_activity_tick)
                .map(|(owner, _)| owner.clone());
            let Some(candidate) = candidate else { break };
            state.owners.remove(&candidate);
        }
    }
    state.activity_clock = state.activity_clock.saturating_add(1);
    let tick = state.activity_clock;
    let stream = state
        .owners
        .entry(owner_principal_key.to_owned())
        .or_default();
    stream.last_activity_tick = tick;
    stream
}

struct Subscriber {
    id: u64,
    tx: mpsc::SyncSender<EventEnvelope>,
    lagged: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerEventBusMetrics {
    pub published_total: u64,
    pub subscriber_lag_total: u64,
    pub subscriber_disconnect_total: u64,
    pub subscribers: usize,
    pub replay_events: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventBusMetrics {
    pub published_total: u64,
    pub subscriber_lag_total: u64,
    pub subscriber_disconnect_total: u64,
    pub owner_streams: usize,
    pub subscribers: usize,
    pub replay_events: usize,
}

#[derive(Clone)]
pub struct EventBus {
    state: Arc<Mutex<BusState>>,
    replay_capacity: usize,
    published_total: Arc<AtomicU64>,
    subscriber_lag_total: Arc<AtomicU64>,
    subscriber_disconnect_total: Arc<AtomicU64>,
    next_subscriber_id: Arc<AtomicU64>,
}

pub struct ReplayBatch {
    pub current_sequence: u64,
    pub replay_floor_sequence: u64,
    pub complete: bool,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug)]
pub enum SubscriptionItem {
    Event(Box<EventEnvelope>),
    Lagged,
    Timeout,
    Disconnected,
}

pub struct EventSubscription {
    rx: mpsc::Receiver<EventEnvelope>,
    lagged: Arc<AtomicBool>,
    state: Weak<Mutex<BusState>>,
    owner_principal_key: String,
    subscriber_id: u64,
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(stream) = state.owners.get_mut(&self.owner_principal_key) {
            stream
                .subscribers
                .retain(|subscriber| subscriber.id != self.subscriber_id);
        }
    }
}

impl EventSubscription {
    fn drain_after_lag(&self) {
        while self.rx.try_recv().is_ok() {}
    }

    pub fn recv_timeout(&self, timeout: Duration) -> SubscriptionItem {
        if self.lagged.swap(false, Ordering::AcqRel) {
            self.drain_after_lag();
            return SubscriptionItem::Lagged;
        }
        match self.rx.recv_timeout(timeout) {
            Ok(event) => SubscriptionItem::Event(Box::new(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if self.lagged.swap(false, Ordering::AcqRel) {
                    self.drain_after_lag();
                    SubscriptionItem::Lagged
                } else {
                    SubscriptionItem::Timeout
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => SubscriptionItem::Disconnected,
        }
    }

    pub fn try_recv(&self) -> SubscriptionItem {
        if self.lagged.swap(false, Ordering::AcqRel) {
            self.drain_after_lag();
            return SubscriptionItem::Lagged;
        }
        match self.rx.try_recv() {
            Ok(event) => SubscriptionItem::Event(Box::new(event)),
            Err(mpsc::TryRecvError::Empty) => SubscriptionItem::Timeout,
            Err(mpsc::TryRecvError::Disconnected) => SubscriptionItem::Disconnected,
        }
    }
}

impl EventBus {
    pub fn new(replay_capacity: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(BusState::default())),
            replay_capacity: replay_capacity.max(1),
            published_total: Arc::new(AtomicU64::new(0)),
            subscriber_lag_total: Arc::new(AtomicU64::new(0)),
            subscriber_disconnect_total: Arc::new(AtomicU64::new(0)),
            next_subscriber_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn current_sequence(&self, owner_principal_key: &str) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .owners
            .get(owner_principal_key)
            .map(|stream| stream.sequence)
            .unwrap_or(0)
    }

    pub fn replay_floor_sequence(&self, owner_principal_key: &str) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .owners
            .get(owner_principal_key)
            .map(replay_floor)
            .unwrap_or(0)
    }

    pub fn metrics(&self) -> EventBusMetrics {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        EventBusMetrics {
            published_total: self.published_total.load(Ordering::Relaxed),
            subscriber_lag_total: self.subscriber_lag_total.load(Ordering::Relaxed),
            subscriber_disconnect_total: self.subscriber_disconnect_total.load(Ordering::Relaxed),
            owner_streams: state.owners.len(),
            subscribers: state
                .owners
                .values()
                .map(|stream| stream.subscribers.len())
                .sum(),
            replay_events: state
                .owners
                .values()
                .map(|stream| stream.replay.len())
                .sum(),
        }
    }

    pub fn metrics_for_owner(&self, owner_principal_key: &str) -> OwnerEventBusMetrics {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state
            .owners
            .get(owner_principal_key)
            .map(|stream| OwnerEventBusMetrics {
                published_total: stream.sequence,
                subscriber_lag_total: stream.subscriber_lag_total,
                subscriber_disconnect_total: stream.subscriber_disconnect_total,
                subscribers: stream.subscribers.len(),
                replay_events: stream.replay.len(),
            })
            .unwrap_or_default()
    }

    pub fn publish(
        &self,
        owner_principal_key: &str,
        kind: EventKind,
        plan_id: impl Into<String>,
        payload: Option<aethercore_contracts::v1::event_envelope::Payload>,
    ) -> EventEnvelope {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let stream = owner_stream_mut(&mut state, owner_principal_key);
        stream.sequence = stream.sequence.saturating_add(1);
        let event = EventEnvelope {
            sequence: stream.sequence,
            emitted_unix_ms: Utc::now().timestamp_millis(),
            kind: kind as i32,
            plan_id: plan_id.into(),
            payload,
        };

        self.published_total.fetch_add(1, Ordering::Relaxed);
        stream.replay.push_back(event.clone());
        while stream.replay.len() > self.replay_capacity {
            if let Some(dropped) = stream.replay.pop_front() {
                stream.dropped_through_sequence = dropped.sequence;
            }
        }

        // A bounded stream must never lose an event silently. If a client cannot keep up, mark the
        // subscription as lagged. The session emits StreamReset and republishes a complete typed
        // hydration image through this same ordered stream.
        let mut lag_transitions = 0u64;
        let mut disconnects = 0u64;
        stream
            .subscribers
            .retain(|subscriber| match subscriber.tx.try_send(event.clone()) {
                Ok(()) => true,
                Err(mpsc::TrySendError::Full(_)) => {
                    if !subscriber.lagged.swap(true, Ordering::AcqRel) {
                        lag_transitions = lag_transitions.saturating_add(1);
                    }
                    true
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    disconnects = disconnects.saturating_add(1);
                    false
                }
            });
        stream.subscriber_lag_total = stream.subscriber_lag_total.saturating_add(lag_transitions);
        stream.subscriber_disconnect_total = stream
            .subscriber_disconnect_total
            .saturating_add(disconnects);
        self.subscriber_lag_total
            .fetch_add(lag_transitions, Ordering::Relaxed);
        self.subscriber_disconnect_total
            .fetch_add(disconnects, Ordering::Relaxed);
        event
    }

    /// Subscription installation and replay capture occur under the same lock. An event is either
    /// present in `ReplayBatch.events` or queued to the subscriber; it cannot fall into a race gap.
    pub fn subscribe(
        &self,
        owner_principal_key: &str,
        replay_after_sequence: u64,
    ) -> (EventSubscription, ReplayBatch) {
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_CAPACITY);
        let lagged = Arc::new(AtomicBool::new(false));
        let subscriber_id = self.next_subscriber_id.fetch_add(1, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let stream = owner_stream_mut(&mut state, owner_principal_key);

        // Sequence space is in-memory per principal. If a client presents a sequence beyond the
        // current stream (service restart/kernel reset), replay cannot be continuous and a
        // StreamReset + full typed hydration is required.
        let complete = replay_after_sequence <= stream.sequence
            && replay_after_sequence >= stream.dropped_through_sequence;
        let events = if complete {
            stream
                .replay
                .iter()
                .filter(|event| event.sequence > replay_after_sequence)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let batch = ReplayBatch {
            current_sequence: stream.sequence,
            replay_floor_sequence: replay_floor(stream),
            complete,
            events,
        };
        stream.subscribers.push(Subscriber {
            id: subscriber_id,
            tx,
            lagged: lagged.clone(),
        });
        (
            EventSubscription {
                rx,
                lagged,
                state: Arc::downgrade(&self.state),
                owner_principal_key: owner_principal_key.to_owned(),
                subscriber_id,
            },
            batch,
        )
    }
}

fn replay_floor(stream: &OwnerStream) -> u64 {
    stream
        .replay
        .front()
        .map(|event| event.sequence)
        .unwrap_or_else(|| stream.sequence.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_is_monotonic_per_owner_and_does_not_leak_cross_user_activity() {
        let bus = EventBus::new(8);
        let a1 = bus.publish("owner-a", EventKind::PlanChanged, "p1", None);
        let b1 = bus.publish("owner-b", EventKind::PlanChanged, "p2", None);
        let a2 = bus.publish("owner-a", EventKind::PlanChanged, "p3", None);
        assert_eq!(a1.sequence, 1);
        assert_eq!(b1.sequence, 1);
        assert_eq!(a2.sequence, 2);
        let (_, replay) = bus.subscribe("owner-a", 0);
        assert!(replay.complete);
        assert_eq!(replay.events.len(), 2);
        assert!(replay.events.iter().all(|e| e.plan_id != "p2"));
    }

    #[test]
    fn replay_window_exhaustion_is_explicit() {
        let bus = EventBus::new(2);
        for index in 0..4 {
            bus.publish("owner-a", EventKind::PlanChanged, format!("p{index}"), None);
        }
        let (_, replay) = bus.subscribe("owner-a", 1);
        assert!(!replay.complete);
        assert!(replay.events.is_empty());
        assert_eq!(replay.current_sequence, 4);
        assert_eq!(replay.replay_floor_sequence, 3);
    }

    #[test]
    fn subscription_install_has_no_publish_replay_race() {
        let bus = EventBus::new(8);
        bus.publish("owner-a", EventKind::PlanChanged, "p1", None);
        let (subscription, replay) = bus.subscribe("owner-a", 0);
        assert_eq!(replay.events.len(), 1);
        bus.publish("owner-a", EventKind::PlanChanged, "p2", None);
        let event = subscription
            .rx
            .recv_timeout(Duration::from_millis(50))
            .unwrap();
        assert_eq!(event.sequence, 2);
    }
    #[test]
    fn sequence_from_previous_service_epoch_requires_reset() {
        let bus = EventBus::new(8);
        let (_, replay) = bus.subscribe("owner-a", 42);
        assert!(!replay.complete);
        assert_eq!(replay.current_sequence, 0);
    }

    #[test]
    fn quiet_subscription_timeout_is_not_disconnect() {
        let bus = EventBus::new(8);
        let (subscription, _) = bus.subscribe("owner-a", 0);
        assert!(matches!(
            subscription.recv_timeout(Duration::from_millis(1)),
            SubscriptionItem::Timeout
        ));
    }

    #[test]
    fn bounded_subscriber_overflow_is_reported_as_lag_not_silent_loss() {
        let bus = EventBus::new(SUBSCRIBER_CAPACITY * 2);
        let (subscription, replay) = bus.subscribe("owner-a", 0);
        assert!(replay.complete);
        for index in 0..=SUBSCRIBER_CAPACITY {
            bus.publish(
                "owner-a",
                EventKind::ProgressTelemetry,
                format!("p{index}"),
                None,
            );
        }
        assert!(matches!(subscription.try_recv(), SubscriptionItem::Lagged));
        // Lag handling drains queued stale telemetry before the server emits StreamReset + hydration.
        assert!(matches!(subscription.try_recv(), SubscriptionItem::Timeout));
    }

    #[test]
    fn dropping_quiet_subscription_releases_registry_immediately() {
        let bus = EventBus::new(8);
        let (subscription, _) = bus.subscribe("owner-a", 0);
        assert_eq!(bus.metrics_for_owner("owner-a").subscribers, 1);
        drop(subscription);
        assert_eq!(bus.metrics_for_owner("owner-a").subscribers, 0);
    }

    #[test]
    fn inactive_owner_replay_state_is_bounded_and_reconnect_resets_safely() {
        let bus = EventBus::new(4);
        for index in 0..(MAX_OWNER_STREAMS + 32) {
            bus.publish(
                &format!("owner-{index}"),
                EventKind::PlanChanged,
                format!("plan-{index}"),
                None,
            );
        }
        assert_eq!(bus.metrics().owner_streams, MAX_OWNER_STREAMS);
        assert_eq!(bus.metrics_for_owner("owner-0").published_total, 0);
        let (_, replay) = bus.subscribe("owner-0", 1);
        assert!(!replay.complete);
        assert_eq!(replay.current_sequence, 0);
    }

    #[test]
    fn active_owner_stream_is_never_evicted_under_retention_pressure() {
        let bus = EventBus::new(4);
        let (subscription, _) = bus.subscribe("active-owner", 0);
        for index in 0..(MAX_OWNER_STREAMS + 32) {
            bus.publish(
                &format!("transient-{index}"),
                EventKind::PlanChanged,
                format!("plan-{index}"),
                None,
            );
        }
        assert_eq!(bus.metrics_for_owner("active-owner").subscribers, 1);
        assert!(bus.metrics().owner_streams <= MAX_OWNER_STREAMS);
        drop(subscription);
    }

    #[test]
    fn metrics_expose_backpressure_without_principal_content() {
        let bus = EventBus::new(SUBSCRIBER_CAPACITY * 2);
        let (subscription, _) = bus.subscribe("owner-sensitive", 0);
        for index in 0..=SUBSCRIBER_CAPACITY {
            bus.publish(
                "owner-sensitive",
                EventKind::ProgressTelemetry,
                format!("plan-{index}"),
                None,
            );
        }
        assert!(matches!(subscription.try_recv(), SubscriptionItem::Lagged));
        bus.publish("owner-other", EventKind::PlanChanged, "foreign", None);
        let metrics = bus.metrics();
        assert_eq!(metrics.published_total, (SUBSCRIBER_CAPACITY + 2) as u64);
        assert_eq!(metrics.subscriber_lag_total, 1);
        assert_eq!(metrics.owner_streams, 2);
        let owner = bus.metrics_for_owner("owner-sensitive");
        assert_eq!(owner.published_total, (SUBSCRIBER_CAPACITY + 1) as u64);
        assert_eq!(owner.subscriber_lag_total, 1);
        assert_eq!(owner.subscribers, 1);
        assert_eq!(owner.replay_events, SUBSCRIBER_CAPACITY + 1);
        assert_eq!(bus.metrics_for_owner("owner-other").published_total, 1);
    }
}
