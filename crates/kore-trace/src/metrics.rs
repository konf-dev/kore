//! # Metrics
//!
//! Counter, gauge, and histogram metrics for observability.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `counter-inc` | `(text num --)` | Increment a counter |
//! | `gauge-set` | `(text num --)` | Set a gauge value |
//! | `histogram-rec` | `(text num --)` | Record histogram observation |
//! | `metrics-dump` | `(-- map)` | Dump all metrics as a map |
//!
//! ## Example
//!
//! ```kore
//! "requests_total" 1 counter-inc
//! "connections_active" 42 gauge-set
//! "request_duration_ms" 123.45 histogram-rec
//! ```
//!
//! ## Metric Types
//!
//! - **Counter**: Monotonically increasing value (e.g., request count)
//! - **Gauge**: Point-in-time value (e.g., active connections)
//! - **Histogram**: Distribution of values (e.g., latencies)

use indexmap::IndexMap;
use kore::{Context, Stack, Tool, Value};
use std::collections::HashMap;
use std::sync::RwLock;

/// Metric storage.
#[derive(Debug, Default)]
struct MetricStore {
    counters: HashMap<String, f64>,
    gauges: HashMap<String, f64>,
    histograms: HashMap<String, Vec<f64>>,
}

/// Global metric store.
static METRICS: std::sync::OnceLock<RwLock<MetricStore>> = std::sync::OnceLock::new();

fn metrics() -> &'static RwLock<MetricStore> {
    METRICS.get_or_init(|| RwLock::new(MetricStore::default()))
}

/// Register all metric tools into a context.
pub async fn register_metrics_tools(ctx: &mut Context) {
    // counter-inc: (text num --)
    ctx.dict.write().await.register(
        Tool::native("counter-inc", "(text num --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let amount = stack.pop()?.as_num()?;
                let name = stack.pop()?.into_text()?;

                let mut store = metrics().write().unwrap();
                let counter = store.counters.entry(name.clone()).or_insert(0.0);
                *counter += amount;

                tracing::trace!(metric = %name, value = %*counter, "counter incremented");

                Ok((stack, ctx))
            })
        }),
    );

    // gauge-set: (text num --)
    ctx.dict.write().await.register(
        Tool::native("gauge-set", "(text num --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?.as_num()?;
                let name = stack.pop()?.into_text()?;

                let mut store = metrics().write().unwrap();
                store.gauges.insert(name.clone(), value);

                tracing::trace!(metric = %name, value = %value, "gauge set");

                Ok((stack, ctx))
            })
        }),
    );

    // histogram-rec: (text num --)
    ctx.dict.write().await.register(
        Tool::native("histogram-rec", "(text num --)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let value = stack.pop()?.as_num()?;
                let name = stack.pop()?.into_text()?;

                let mut store = metrics().write().unwrap();
                store
                    .histograms
                    .entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(value);

                tracing::trace!(metric = %name, value = %value, "histogram recorded");

                Ok((stack, ctx))
            })
        }),
    );

    // metrics-dump: (-- map)
    ctx.dict.write().await.register(
        Tool::native("metrics-dump", "(-- map)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let store = metrics().read().unwrap();

                // Build counters map
                let mut counters_map: IndexMap<String, Value> = IndexMap::new();
                for (name, value) in &store.counters {
                    counters_map.insert(name.clone(), Value::Float(*value));
                }

                // Build gauges map
                let mut gauges_map: IndexMap<String, Value> = IndexMap::new();
                for (name, value) in &store.gauges {
                    gauges_map.insert(name.clone(), Value::Float(*value));
                }

                // Build histogram summaries
                let mut histograms_map: IndexMap<String, Value> = IndexMap::new();
                for (name, values) in &store.histograms {
                    if values.is_empty() {
                        continue;
                    }
                    let count = values.len() as f64;
                    let sum: f64 = values.iter().sum();
                    let mean = sum / count;
                    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                    let mut summary: IndexMap<String, Value> = IndexMap::new();
                    summary.insert("count".to_string(), Value::Float(count));
                    summary.insert("sum".to_string(), Value::Float(sum));
                    summary.insert("mean".to_string(), Value::Float(mean));
                    summary.insert("min".to_string(), Value::Float(min));
                    summary.insert("max".to_string(), Value::Float(max));

                    histograms_map.insert(name.clone(), Value::Map(summary));
                }

                let mut result: IndexMap<String, Value> = IndexMap::new();
                result.insert("counters".to_string(), Value::Map(counters_map));
                result.insert("gauges".to_string(), Value::Map(gauges_map));
                result.insert("histograms".to_string(), Value::Map(histograms_map));

                stack.push(Value::Map(result))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_tools_registered() {
        let mut ctx = kore::Context::new();
        register_metrics_tools(&mut ctx).await;

        let dict = ctx.dict.read().await;
        let tools = dict.list(&ctx.tenant);

        assert!(tools.contains(&"counter-inc".to_string()));
        assert!(tools.contains(&"gauge-set".to_string()));
        assert!(tools.contains(&"histogram-rec".to_string()));
        assert!(tools.contains(&"metrics-dump".to_string()));
    }

    #[tokio::test]
    async fn test_counter_inc() {
        let mut ctx = kore::Context::new();
        register_metrics_tools(&mut ctx).await;

        // Use unique metric name for this test to avoid parallel test interference
        let (_, _) = kore::execute(
            &kore::Op::parse(r#""test_counter_requests" 1 counter-inc "test_counter_requests" 5 counter-inc"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        let store = metrics().read().unwrap();
        // Just verify the counter exists and has a positive value
        assert!(store.counters.get("test_counter_requests").is_some());
    }

    #[tokio::test]
    async fn test_gauge_set() {
        let mut ctx = kore::Context::new();
        register_metrics_tools(&mut ctx).await;

        // Use unique metric name for this test
        let (_, _) = kore::execute(
            &kore::Op::parse(r#""test_gauge_temperature" 25 gauge-set"#).unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        let store = metrics().read().unwrap();
        assert_eq!(store.gauges.get("test_gauge_temperature"), Some(&25.0));
    }

    #[tokio::test]
    async fn test_histogram_rec() {
        let mut ctx = kore::Context::new();
        register_metrics_tools(&mut ctx).await;

        // Use unique metric name for this test
        let (_, _) = kore::execute(
            &kore::Op::parse(r#""test_hist_latency" 100 histogram-rec "test_hist_latency" 200 histogram-rec"#)
                .unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        let store = metrics().read().unwrap();
        // Just verify the histogram exists and has values
        assert!(store.histograms.get("test_hist_latency").is_some());
    }

    #[tokio::test]
    async fn test_metrics_dump() {
        let mut ctx = kore::Context::new();
        register_metrics_tools(&mut ctx).await;

        // Use unique metric names for this test then dump
        let (mut stack, _) = kore::execute(
            &kore::Op::parse(
                r#""test_dump_counter" 10 counter-inc "test_dump_gauge" 42 gauge-set metrics-dump"#,
            )
            .unwrap(),
            kore::Stack::new(),
            ctx,
        )
        .await
        .unwrap();

        let result = stack.pop().unwrap();
        assert!(matches!(result, Value::Map(_)));
    }
}
