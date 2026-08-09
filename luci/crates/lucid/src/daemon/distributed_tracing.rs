use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Distributed trace span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub node_id: String,
    pub tags: HashMap<String, String>,
    pub status: SpanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error(String),
}

/// Distributed tracing manager
pub struct Tracer {
    node_id: String,
    traces: Vec<TraceSpan>,
}

impl Tracer {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            traces: Vec::new(),
        }
    }

    pub fn start_span(&mut self, operation_name: &str, trace_id: Option<String>) -> String {
        let trace_id = trace_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let span_id = uuid::Uuid::new_v4().to_string();
        
        let span = TraceSpan {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id: None,
            operation_name: operation_name.to_string(),
            start_time: Utc::now(),
            end_time: None,
            duration_ms: None,
            node_id: self.node_id.clone(),
            tags: HashMap::new(),
            status: SpanStatus::Ok,
        };
        
        self.traces.push(span);
        span_id
    }

    pub fn finish_span(&mut self, span_id: &str) {
        if let Some(span) = self.traces.iter_mut().find(|s| s.span_id == span_id) {
            let end_time = Utc::now();
            span.end_time = Some(end_time);
            span.duration_ms = Some((end_time - span.start_time).num_milliseconds() as u64);
        }
    }

    pub fn set_error(&mut self, span_id: &str, error: String) {
        if let Some(span) = self.traces.iter_mut().find(|s| s.span_id == span_id) {
            span.status = SpanStatus::Error(error);
        }
    }

    pub fn add_tag(&mut self, span_id: &str, key: &str, value: &str) {
        if let Some(span) = self.traces.iter_mut().find(|s| s.span_id == span_id) {
            span.tags.insert(key.to_string(), value.to_string());
        }
    }

    pub fn get_trace(&self, trace_id: &str) -> Vec<&TraceSpan> {
        self.traces.iter().filter(|s| s.trace_id == trace_id).collect()
    }

    pub fn recent_traces(&self, limit: usize) -> Vec<&TraceSpan> {
        self.traces.iter().rev().take(limit).collect()
    }
}
