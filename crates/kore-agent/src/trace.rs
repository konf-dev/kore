//! Trace - Simple event logging
//!
//! Does ONE thing: logs events to stdout AND file.
//! No formatting for LLM. No context building. Just logging.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Start,
    Iteration,
    Think,
    Response,
    Code,
    Success,
    Error,
    Done,
}

/// A trace event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
    pub message: String,
}

/// The trace logger
pub struct Trace {
    /// Log file handle
    file: File,
    
    /// Recent events (for LLM context)
    recent_events: Vec<Event>,
    
    /// Max events to keep in memory
    max_recent: usize,
}

impl Trace {
    /// Create new trace logger
    pub fn new(log_path: &Path) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .expect("Failed to open trace log file");
        
        Self {
            file,
            recent_events: Vec::new(),
            max_recent: 20,
        }
    }
    
    /// Log an event
    fn log(&mut self, kind: EventKind, message: &str) {
        let event = Event {
            timestamp: Utc::now(),
            kind: kind.clone(),
            message: message.to_string(),
        };
        
        // Print to stdout (human readable)
        let icon = match kind {
            EventKind::Start => "🚀",
            EventKind::Iteration => "🔄",
            EventKind::Think => "🤔",
            EventKind::Response => "💬",
            EventKind::Code => "📝",
            EventKind::Success => "✅",
            EventKind::Error => "❌",
            EventKind::Done => "🎉",
        };
        
        let ts = event.timestamp.format("%H:%M:%S%.3f");
        let msg = truncate(message, 150);
        println!("[{}] {} {}", ts, icon, msg);
        
        // Write to file (JSON lines)
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = writeln!(self.file, "{}", json);
            let _ = self.file.flush();
        }
        
        // Keep in memory for recent()
        self.recent_events.push(event);
        if self.recent_events.len() > self.max_recent {
            self.recent_events.remove(0);
        }
    }
    
    // Convenience methods - each does ONE thing
    
    pub fn start(&mut self, goal: &str) {
        self.log(EventKind::Start, &format!("Goal: {}", goal));
    }
    
    pub fn iteration(&mut self, n: u32) {
        self.log(EventKind::Iteration, &format!("Iteration {}", n));
    }
    
    pub fn thinking(&mut self) {
        self.log(EventKind::Think, "Calling LLM...");
    }
    
    pub fn response(&mut self, text: &str) {
        self.log(EventKind::Response, text);
    }
    
    pub fn code(&mut self, code: &str) {
        self.log(EventKind::Code, code);
    }
    
    pub fn success(&mut self, result: &str) {
        self.log(EventKind::Success, result);
    }
    
    pub fn error(&mut self, msg: &str) {
        self.log(EventKind::Error, msg);
    }
    
    pub fn done(&mut self, msg: &str) {
        self.log(EventKind::Done, msg);
    }
    
    /// Get recent events as simple text (for LLM context)
    pub fn recent(&self, n: usize) -> String {
        let start = self.recent_events.len().saturating_sub(n);
        self.recent_events[start..]
            .iter()
            .map(|e| {
                let kind = match e.kind {
                    EventKind::Start => "START",
                    EventKind::Iteration => "ITER",
                    EventKind::Think => "THINK",
                    EventKind::Response => "RESPONSE",
                    EventKind::Code => "CODE",
                    EventKind::Success => "OK",
                    EventKind::Error => "ERROR",
                    EventKind::Done => "DONE",
                };
                format!("[{}] {}", kind, truncate(&e.message, 80))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}
