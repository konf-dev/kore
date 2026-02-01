//! Message queue for workflow communication

use crate::types::Message;
use std::collections::VecDeque;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

/// Message queue for a single workflow
#[derive(Debug)]
pub struct MessageQueue {
    /// The queue of pending messages
    queue: VecDeque<Message>,
    
    /// Maximum queue size (0 = unlimited)
    max_size: usize,
}

impl MessageQueue {
    /// Create a new message queue with default capacity
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(64),
            max_size: 1024,
        }
    }

    /// Create with a specific max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size.min(64)),
            max_size,
        }
    }

    /// Push a message onto the queue
    pub fn push(&mut self, msg: Message) -> Result<(), QueueError> {
        if self.max_size > 0 && self.queue.len() >= self.max_size {
            return Err(QueueError::Full);
        }
        self.queue.push_back(msg);
        Ok(())
    }

    /// Pop the next message (non-blocking)
    pub fn pop(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }

    /// Check if there are pending messages
    pub fn has_messages(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Drain all messages
    pub fn drain(&mut self) -> Vec<Message> {
        self.queue.drain(..).collect()
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors related to message queue operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum QueueError {
    #[error("queue is full")]
    Full,
}

/// Thread-safe wrapper for async message operations
#[derive(Clone)]
pub struct AsyncMessageQueue {
    inner: Arc<Mutex<MessageQueue>>,
    notify: Arc<tokio::sync::Notify>,
}

impl AsyncMessageQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MessageQueue::new())),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Send a message (async)
    pub async fn send(&self, msg: Message) -> Result<(), QueueError> {
        {
            let mut queue = self.inner.lock().await;
            queue.push(msg)?;
        }
        self.notify.notify_one();
        Ok(())
    }

    /// Receive a message (blocking async)
    pub async fn recv(&self) -> Message {
        loop {
            {
                let mut queue = self.inner.lock().await;
                if let Some(msg) = queue.pop() {
                    return msg;
                }
            }
            self.notify.notified().await;
        }
    }

    /// Receive with timeout
    pub async fn recv_timeout(&self, timeout: Duration) -> Option<Message> {
        tokio::time::timeout(timeout, self.recv()).await.ok()
    }

    /// Try to receive (non-blocking)
    pub async fn try_recv(&self) -> Option<Message> {
        let mut queue = self.inner.lock().await;
        queue.pop()
    }

    /// Check if there are messages
    pub async fn has_messages(&self) -> bool {
        let queue = self.inner.lock().await;
        queue.has_messages()
    }

    /// Get queue length
    pub async fn len(&self) -> usize {
        let queue = self.inner.lock().await;
        queue.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        let queue = self.inner.lock().await;
        queue.is_empty()
    }
}

impl Default for AsyncMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WorkflowHandle;
    use kore::Value;

    #[test]
    fn queue_basic_ops() {
        let sender = WorkflowHandle::new();
        let mut queue = MessageQueue::new();
        
        queue.push(Message::new(Value::Int(1), sender)).unwrap();
        queue.push(Message::new(Value::Int(2), sender)).unwrap();
        
        assert_eq!(queue.len(), 2);
        
        let msg1 = queue.pop().unwrap();
        assert_eq!(msg1.payload, Value::Int(1));
        
        let msg2 = queue.pop().unwrap();
        assert_eq!(msg2.payload, Value::Int(2));
        
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_max_size() {
        let sender = WorkflowHandle::new();
        let mut queue = MessageQueue::with_max_size(2);
        
        queue.push(Message::new(Value::Int(1), sender)).unwrap();
        queue.push(Message::new(Value::Int(2), sender)).unwrap();
        
        let result = queue.push(Message::new(Value::Int(3), sender));
        assert!(matches!(result, Err(QueueError::Full)));
    }

    #[tokio::test]
    async fn async_queue_send_recv() {
        let sender = WorkflowHandle::new();
        let queue = AsyncMessageQueue::new();
        
        queue.send(Message::new(Value::Int(42), sender)).await.unwrap();
        
        let msg = queue.recv().await;
        assert_eq!(msg.payload, Value::Int(42));
    }

    #[tokio::test]
    async fn async_queue_timeout() {
        let queue = AsyncMessageQueue::new();
        
        let result = queue.recv_timeout(Duration::from_millis(10)).await;
        assert!(result.is_none());
    }
}
