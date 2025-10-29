use metrics::gauge;
use std::net::{IpAddr, SocketAddr};
// third-party
use metrics_exporter_prometheus::PrometheusBuilder;
// use metrics_util::MetricKindMask;
use tracing::{
    // debug,
    // error,
    info,
};

pub struct Metric {
    pub name: &'static str,
    description: &'static str,
}

pub const USER_REGISTERED_REQUESTS: Metric = Metric {
    name: "user_registered_requests",
    description: "Number of RegisterUser grpc requests",
};
pub const USER_REGISTERED: Metric = Metric {
    name: "user_registered",
    description: "Number of registered users in the prover",
};
pub const SEND_TRANSACTION_REQUESTS: Metric = Metric {
    name: "send_transaction_requests",
    description: "Number of SendTransaction grpc requests",
};

pub const GET_USER_TIER_INFO_REQUESTS: Metric = Metric {
    name: "get_user_tier_info_requests",
    description: "Number of GetUserTierInfo grpc requests",
};

pub const EPOCH_SERVICE_CURRENT_EPOCH: Metric = Metric {
    name: "epoch_service_current_epoch",
    description: "Current epoch in the epoch service",
};

pub const EPOCH_SERVICE_CURRENT_EPOCH_SLICE: Metric = Metric {
    name: "epoch_service_current_epoch_slice",
    description: "Current epoch slice in the epoch service",
};

pub const EPOCH_SERVICE_DRIFT_MILLIS: Metric = Metric {
    name: "epoch_service_drift_millis",
    description: "Drift in milliseconds (when epoch service is waiting for the next epoch slice)",
};

pub const PROOF_SERVICE_PROOF_COMPUTED: Metric = Metric {
    name: "proof_service_proof_computed",
    description: "Number of computed proofs",
};

pub const PROOF_SERVICE_GEN_PROOF_TIME: Metric = Metric {
    name: "proof_service_gen_proof_time",
    description: "Generation time of a proof in seconds",
};

pub const GET_PROOFS_LISTENERS: Metric = Metric {
    name: "get_proof_listeners",
    description: "Current number of active subscription to grpc get_proofs server streaming endpoint",
};

/// Histogram metrics for the broadcast channel (used by proof service to send proofs)
pub const BROADCAST_CHANNEL_QUEUE_LEN: Metric = Metric {
    name: "broadcast_channel_queue_len",
    description: "Number of queued values",
};

/// Histogram metrics for the mpmc channel (used by proof services to receive new tx)
pub const PROOF_SERVICES_CHANNEL_QUEUE_LEN: Metric = Metric {
    name: "proof_services_channel_queue_len",
    description: "Number of queued values",
};

pub const COUNTERS: [Metric; 5] = [
    USER_REGISTERED,
    USER_REGISTERED_REQUESTS,
    SEND_TRANSACTION_REQUESTS,
    GET_USER_TIER_INFO_REQUESTS,
    PROOF_SERVICE_PROOF_COMPUTED,
];
pub const GAUGES: [Metric; 3] = [
    EPOCH_SERVICE_CURRENT_EPOCH,
    EPOCH_SERVICE_CURRENT_EPOCH_SLICE,
    GET_PROOFS_LISTENERS,
];
pub const HISTOGRAMS: [Metric; 3] = [
    EPOCH_SERVICE_DRIFT_MILLIS,
    PROOF_SERVICE_GEN_PROOF_TIME,
    BROADCAST_CHANNEL_QUEUE_LEN,
];

pub fn init_metrics(ip: IpAddr, port: &u16) {
    info!("Initializing metrics exporter (port: {})", port);

    // Install in the current tokio runtime
    PrometheusBuilder::new()
        // .idle_timeout(
        //     MetricKindMask::COUNTER | MetricKindMask::HISTOGRAM,
        //     Some(Duration::from_secs(10)),
        // )
        .with_http_listener(SocketAddr::new(
            // IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            ip,
            port.to_owned(),
        ))
        .install()
        .expect("failed to install Prometheus recorder");

    for name in COUNTERS {
        register_counter(name)
    }

    for name in GAUGES {
        register_gauge(name)
    }

    for name in HISTOGRAMS {
        register_histogram(name)
    }
}

/// Registers a counter with the given name.
fn register_counter(metric: Metric) {
    metrics::describe_counter!(metric.name, metric.description);
    let _counter = metrics::counter!(metric.name);
}

/// Registers a gauge with the given name.
fn register_gauge(metric: Metric) {
    metrics::describe_gauge!(metric.name, metric.description);
    let _gauge = ::metrics::gauge!(metric.name);
}

/// Registers a histogram with the given name.
fn register_histogram(metric: Metric) {
    metrics::describe_histogram!(metric.name, metric.description);
    let _histogram = ::metrics::histogram!(metric.name);
}

/// A Wrapper around a metric gauge
///
/// Increment the given metric gauge on a new and decrement on drop
/// Useful in a closure (or an async closure)
pub struct GaugeWrapper {
    gauge_name: &'static str,
    gauge_app: &'static str,
    gauge_label: &'static str,
}

impl GaugeWrapper {
    pub fn new(
        gauge_name: &'static str,
        gauge_app: &'static str,
        gauge_label: &'static str,
    ) -> Self {
        gauge!(gauge_name, gauge_app => gauge_label).increment(1.0);
        Self {
            gauge_name,
            gauge_app,
            gauge_label,
        }
    }
}

impl Drop for GaugeWrapper {
    fn drop(&mut self) {
        gauge!(self.gauge_name, self.gauge_app => self.gauge_label).decrement(1.0);
    }
}
