use std::collections::HashSet;
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::mpsc;
use tracing::warn;
use wiki_domain::response::DeploymentEvent;

const CHANNEL_CAPACITY: usize = 128;

pub enum SubscriberScope {
    Global,
    Projects(HashSet<String>),
}

impl SubscriberScope {
    fn allows(&self, project_id: &str) -> bool {
        match self {
            SubscriberScope::Global => true,
            SubscriberScope::Projects(set) => set.contains(project_id),
        }
    }
}

struct Subscription {
    token: Weak<()>,
    sender: mpsc::Sender<DeploymentEvent>,
    scope: SubscriberScope,
}

#[derive(Default)]
pub struct ConnectionManager {
    subscriptions: Mutex<Vec<Subscription>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, scope: SubscriberScope) -> Subscriber {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let token = Arc::new(());
        self.subscriptions
            .lock()
            .expect("connection manager mutex poisoned")
            .push(Subscription {
                token: Arc::downgrade(&token),
                sender: tx,
                scope,
            });
        Subscriber {
            _token: token,
            receiver: rx,
        }
    }

    pub fn broadcast(&self, project_id: &str, event: DeploymentEvent) {
        let mut subs = self
            .subscriptions
            .lock()
            .expect("connection manager mutex poisoned");
        subs.retain(|sub| {
            if sub.token.strong_count() == 0 {
                return false;
            }
            if !sub.scope.allows(project_id) {
                return true;
            }
            match sub.sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!("dropping deployment event: subscriber lagging");
                    true
                }
            }
        });
    }
}

pub struct Subscriber {
    _token: Arc<()>,
    pub receiver: mpsc::Receiver<DeploymentEvent>,
}
