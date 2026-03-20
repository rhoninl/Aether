//! NATS JetStream event bus with trait abstraction.
//!
//! Provides an `EventBus` trait for publish/subscribe operations across services.
//! The real implementation uses `async_nats`; a mock is provided for unit tests.

use async_trait::async_trait;

use crate::error::PersistenceError;

/// A received message from a subscription.
#[derive(Debug, Clone)]
pub struct EventMessage {
    pub subject: String,
    pub payload: Vec<u8>,
}

/// Abstraction over a subscription that yields messages.
#[async_trait]
pub trait EventSubscription: Send + Sync {
    /// Wait for the next message. Returns `None` if the subscription is closed.
    async fn next_message(&mut self) -> Option<EventMessage>;
}

/// Abstraction over a pub/sub event bus.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish a message to the given subject.
    async fn publish(&self, subject: &str, payload: &[u8]) -> Result<(), PersistenceError>;

    /// Subscribe to a subject and return a subscription handle.
    async fn subscribe(
        &self,
        subject: &str,
    ) -> Result<Box<dyn EventSubscription>, PersistenceError>;

    /// Check if the NATS connection is alive.
    async fn is_healthy(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Real implementation (behind "nats" feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "nats")]
mod real {
    use super::*;
    use crate::pool::ConnectionConfig;

    /// NATS client backed by `async_nats::Client`.
    pub struct NatsClient {
        client: async_nats::Client,
    }

    impl NatsClient {
        /// Connect to a NATS server using the URL from `ConnectionConfig`.
        pub async fn connect(config: &ConnectionConfig) -> Result<Self, PersistenceError> {
            let client = async_nats::connect(&config.nats_url)
                .await
                .map_err(|e| PersistenceError::ConnectionFailed(e.to_string()))?;
            Ok(Self { client })
        }

        /// Expose the inner client for advanced JetStream usage.
        pub fn client(&self) -> &async_nats::Client {
            &self.client
        }
    }

    struct NatsSubscription {
        subscriber: async_nats::Subscriber,
    }

    #[async_trait]
    impl EventSubscription for NatsSubscription {
        async fn next_message(&mut self) -> Option<EventMessage> {
            use futures::StreamExt;
            let msg = self.subscriber.next().await?;
            Some(EventMessage {
                subject: msg.subject.to_string(),
                payload: msg.payload.to_vec(),
            })
        }
    }

    #[async_trait]
    impl EventBus for NatsClient {
        async fn publish(&self, subject: &str, payload: &[u8]) -> Result<(), PersistenceError> {
            self.client
                .publish(subject.to_string(), payload.to_vec().into())
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            Ok(())
        }

        async fn subscribe(
            &self,
            subject: &str,
        ) -> Result<Box<dyn EventSubscription>, PersistenceError> {
            let subscriber = self
                .client
                .subscribe(subject.to_string())
                .await
                .map_err(|e| PersistenceError::QueryFailed(e.to_string()))?;
            Ok(Box::new(NatsSubscription { subscriber }))
        }

        async fn is_healthy(&self) -> bool {
            matches!(
                self.client.connection_state(),
                async_nats::connection::State::Connected
            )
        }
    }
}

#[cfg(feature = "nats")]
pub use real::NatsClient;

// ---------------------------------------------------------------------------
// Mock implementation (always available)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Default channel capacity for mock subscriptions.
const MOCK_CHANNEL_CAPACITY: usize = 256;

/// Mock event bus for unit testing.
///
/// Messages published to a subject are delivered to all active subscribers of that subject.
pub struct MockEventBus {
    healthy: bool,
    channels: Arc<Mutex<HashMap<String, broadcast::Sender<EventMessage>>>>,
}

impl MockEventBus {
    /// Create a healthy mock event bus.
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create an unhealthy mock event bus.
    pub fn unhealthy() -> Self {
        Self {
            healthy: false,
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn get_or_create_channel(&self, subject: &str) -> broadcast::Sender<EventMessage> {
        let mut channels = self.channels.lock().unwrap();
        channels
            .entry(subject.to_string())
            .or_insert_with(|| broadcast::channel(MOCK_CHANNEL_CAPACITY).0)
            .clone()
    }
}

struct MockSubscription {
    receiver: broadcast::Receiver<EventMessage>,
}

#[async_trait]
impl EventSubscription for MockSubscription {
    async fn next_message(&mut self) -> Option<EventMessage> {
        self.receiver.recv().await.ok()
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish(&self, subject: &str, payload: &[u8]) -> Result<(), PersistenceError> {
        if !self.healthy {
            return Err(PersistenceError::NotConnected);
        }
        let sender = self.get_or_create_channel(subject);
        let msg = EventMessage {
            subject: subject.to_string(),
            payload: payload.to_vec(),
        };
        // Ignore send errors (no receivers is fine for pub/sub).
        let _ = sender.send(msg);
        Ok(())
    }

    async fn subscribe(
        &self,
        subject: &str,
    ) -> Result<Box<dyn EventSubscription>, PersistenceError> {
        if !self.healthy {
            return Err(PersistenceError::NotConnected);
        }
        let sender = self.get_or_create_channel(subject);
        let receiver = sender.subscribe();
        Ok(Box::new(MockSubscription { receiver }))
    }

    async fn is_healthy(&self) -> bool {
        self.healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_healthy_check() {
        let bus = MockEventBus::healthy();
        assert!(bus.is_healthy().await);
    }

    #[tokio::test]
    async fn mock_unhealthy_check() {
        let bus = MockEventBus::unhealthy();
        assert!(!bus.is_healthy().await);
    }

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = MockEventBus::healthy();
        let mut sub = bus.subscribe("test.subject").await.unwrap();

        bus.publish("test.subject", b"hello").await.unwrap();

        let msg = sub.next_message().await.unwrap();
        assert_eq!(msg.subject, "test.subject");
        assert_eq!(msg.payload, b"hello");
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_same_message() {
        let bus = MockEventBus::healthy();
        let mut sub1 = bus.subscribe("multi").await.unwrap();
        let mut sub2 = bus.subscribe("multi").await.unwrap();

        bus.publish("multi", b"data").await.unwrap();

        let msg1 = sub1.next_message().await.unwrap();
        let msg2 = sub2.next_message().await.unwrap();
        assert_eq!(msg1.payload, b"data");
        assert_eq!(msg2.payload, b"data");
    }

    #[tokio::test]
    async fn publish_to_different_subjects_isolated() {
        let bus = MockEventBus::healthy();
        let mut sub_a = bus.subscribe("topic.a").await.unwrap();

        bus.publish("topic.b", b"wrong").await.unwrap();
        bus.publish("topic.a", b"right").await.unwrap();

        let msg = sub_a.next_message().await.unwrap();
        assert_eq!(msg.payload, b"right");
    }

    #[tokio::test]
    async fn publish_without_subscribers_succeeds() {
        let bus = MockEventBus::healthy();
        let result = bus.publish("no_listeners", b"data").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn unhealthy_publish_returns_error() {
        let bus = MockEventBus::unhealthy();
        let result = bus.publish("test", b"data").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unhealthy_subscribe_returns_error() {
        let bus = MockEventBus::unhealthy();
        let result = bus.subscribe("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn event_message_is_clone_and_debug() {
        let msg = EventMessage {
            subject: "test".to_string(),
            payload: vec![1, 2, 3],
        };
        let cloned = msg.clone();
        assert_eq!(cloned.subject, msg.subject);
        assert_eq!(cloned.payload, msg.payload);
        let debug = format!("{msg:?}");
        assert!(debug.contains("test"));
    }

    #[tokio::test]
    async fn trait_object_works() {
        let bus: Box<dyn EventBus> = Box::new(MockEventBus::healthy());
        assert!(bus.is_healthy().await);
        bus.publish("sub", b"payload").await.unwrap();
    }
}
