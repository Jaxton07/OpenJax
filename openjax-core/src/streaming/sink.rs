use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    DropNewest,
    RejectProducer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDispatchError {
    QueueFull,
    Closed,
}

#[derive(Debug)]
pub struct StreamDispatcher<T> {
    tx: mpsc::Sender<T>,
    policy: BackpressurePolicy,
}

impl<T> Clone for StreamDispatcher<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            policy: self.policy,
        }
    }
}

impl<T> StreamDispatcher<T> {
    pub fn new(capacity: usize, policy: BackpressurePolicy) -> (Self, mpsc::Receiver<T>) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (Self { tx, policy }, rx)
    }

    pub fn try_dispatch(&self, item: T) -> Result<(), StreamDispatchError> {
        match self.tx.try_send(item) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(StreamDispatchError::Closed),
            Err(mpsc::error::TrySendError::Full(_)) => match self.policy {
                BackpressurePolicy::DropNewest => Ok(()),
                BackpressurePolicy::RejectProducer => Err(StreamDispatchError::QueueFull),
            },
        }
    }
}
