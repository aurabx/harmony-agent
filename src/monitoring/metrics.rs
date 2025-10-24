//! Metrics collection and export
//!
//! This module provides Prometheus-style metrics collection.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricType {
    /// Bytes transmitted
    BytesTransmitted,
    /// Bytes received
    BytesReceived,
    /// Active peers
    ActivePeers,
    /// Handshake success
    HandshakeSuccess,
    /// Handshake failure
    HandshakeFailure,
    /// Connection uptime
    ConnectionUptime,
    /// Peer latency
    PeerLatency,
    /// Packet loss rate
    PacketLoss,
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BytesTransmitted => write!(f, "harmony_agent_bytes_transmitted_total"),
            Self::BytesReceived => write!(f, "harmony_agent_bytes_received_total"),
            Self::ActivePeers => write!(f, "harmony_agent_active_peers"),
            Self::HandshakeSuccess => write!(f, "harmony_agent_handshake_success_total"),
            Self::HandshakeFailure => write!(f, "harmony_agent_handshake_failure_total"),
            Self::ConnectionUptime => write!(f, "harmony_agent_connection_uptime_seconds"),
            Self::PeerLatency => write!(f, "harmony_agent_peer_latency_milliseconds"),
            Self::PacketLoss => write!(f, "harmony_agent_packet_loss_rate"),
        }
    }
}

impl MetricType {
    /// Get metric help text
    pub fn help_text(&self) -> &'static str {
        match self {
            Self::BytesTransmitted => "Total bytes transmitted through WireGuard tunnels",
            Self::BytesReceived => "Total bytes received through WireGuard tunnels",
            Self::ActivePeers => "Number of active WireGuard peers",
            Self::HandshakeSuccess => "Total successful handshakes",
            Self::HandshakeFailure => "Total failed handshakes",
            Self::ConnectionUptime => "Connection uptime in seconds",
            Self::PeerLatency => "Peer latency in milliseconds",
            Self::PacketLoss => "Packet loss rate percentage",
        }
    }

    /// Get metric type (counter, gauge, histogram)
    pub fn metric_kind(&self) -> &'static str {
        match self {
            Self::BytesTransmitted
            | Self::BytesReceived
            | Self::HandshakeSuccess
            | Self::HandshakeFailure => "counter",
            Self::ActivePeers
            | Self::ConnectionUptime
            | Self::PeerLatency
            | Self::PacketLoss => "gauge",
        }
    }
}

/// Metric value with timestamp
#[derive(Debug, Clone)]
pub struct MetricValue {
    /// Value
    pub value: f64,
    /// Timestamp when recorded
    pub timestamp: Instant,
    /// Labels (for future use)
    pub labels: HashMap<String, String>,
}

impl MetricValue {
    /// Create new metric value
    pub fn new(value: f64) -> Self {
        Self {
            value,
            timestamp: Instant::now(),
            labels: HashMap::new(),
        }
    }

    /// Age of the metric
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// Metrics storage
pub struct Metrics {
    /// Metric values by type
    values: HashMap<MetricType, MetricValue>,
}

impl Metrics {
    /// Create new metrics storage
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Record a metric value
    pub fn record(&mut self, metric_type: MetricType, value: f64) {
        self.values.insert(metric_type, MetricValue::new(value));
    }

    /// Get a metric value
    pub fn get(&self, metric_type: MetricType) -> Option<&MetricValue> {
        self.values.get(&metric_type)
    }

    /// Get all metrics
    pub fn all(&self) -> &HashMap<MetricType, MetricValue> {
        &self.values
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics collector with thread-safe access
pub struct MetricsCollector {
    /// Metrics storage
    metrics: Arc<RwLock<Metrics>>,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Metrics::new())),
        }
    }

    /// Record a metric
    pub fn record(&self, metric_type: MetricType, value: f64) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.record(metric_type, value);
    }

    /// Get a metric value
    pub fn get(&self, metric_type: MetricType) -> Option<MetricValue> {
        let metrics = self.metrics.read().unwrap();
        metrics.get(metric_type).cloned()
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        let metrics = self.metrics.read().unwrap();
        let mut output = String::new();

        for (metric_type, value) in metrics.all().iter() {
            // Add HELP line
            output.push_str(&format!(
                "# HELP {} {}\n",
                metric_type,
                metric_type.help_text()
            ));
            
            // Add TYPE line
            output.push_str(&format!(
                "# TYPE {} {}\n",
                metric_type,
                metric_type.metric_kind()
            ));
            
            // Add metric value
            output.push_str(&format!("{} {}\n", metric_type, value.value));
        }

        output
    }

    /// Get all metrics as JSON
    pub fn export_json(&self) -> serde_json::Value {
        let metrics = self.metrics.read().unwrap();
        let mut map = serde_json::Map::new();

        for (metric_type, value) in metrics.all().iter() {
            map.insert(
                metric_type.to_string(),
                serde_json::json!({
                    "value": value.value,
                    "timestamp": value.timestamp.elapsed().as_secs(),
                }),
            );
        }

        serde_json::Value::Object(map)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_type_display() {
        assert_eq!(
            MetricType::BytesTransmitted.to_string(),
            "harmony_agent_bytes_transmitted_total"
        );
    }

    #[test]
    fn test_metric_value() {
        let value = MetricValue::new(42.0);
        assert_eq!(value.value, 42.0);
        assert!(value.age().as_millis() < 100);
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();
        collector.record(MetricType::ActivePeers, 5.0);
        
        let value = collector.get(MetricType::ActivePeers).unwrap();
        assert_eq!(value.value, 5.0);
    }

    #[test]
    fn test_prometheus_export() {
        let collector = MetricsCollector::new();
        collector.record(MetricType::ActivePeers, 3.0);
        
        let output = collector.export_prometheus();
        assert!(output.contains("harmony_agent_active_peers"));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn test_json_export() {
        let collector = MetricsCollector::new();
        collector.record(MetricType::HandshakeSuccess, 10.0);
        
        let json = collector.export_json();
        assert!(json.is_object());
    }
}
