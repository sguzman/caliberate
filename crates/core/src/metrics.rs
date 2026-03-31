//! Metrics stubs and interfaces.

use crate::config::ControlPlane;
use tracing::info;

#[derive(Debug, Clone)]
pub struct MetricsHandle {
    enabled: bool,
    namespace: String,
}

impl MetricsHandle {
    pub fn counter(&self, _name: &str, _value: u64) {
        if !self.enabled {
            return;
        }
    }

    pub fn gauge(&self, _name: &str, _value: f64) {
        if !self.enabled {
            return;
        }
    }

    pub fn histogram(&self, _name: &str, _value: f64) {
        if !self.enabled {
            return;
        }
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

pub fn init(config: &ControlPlane) -> MetricsHandle {
    if config.metrics.enabled {
        info!(
            component = "metrics",
            namespace = %config.metrics.namespace,
            endpoint = %config.metrics.endpoint,
            "metrics enabled"
        );
    } else {
        info!(component = "metrics", "metrics disabled");
    }

    MetricsHandle {
        enabled: config.metrics.enabled,
        namespace: config.metrics.namespace.clone(),
    }
}
