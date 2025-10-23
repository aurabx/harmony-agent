//! Performance benchmarks for wg-agent
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use wg_agent::config::{Config, NetworkConfig, PeerConfig};
use wg_agent::monitoring::{Monitor, ConnectionState};
use wg_agent::security::{validate_network_name, validate_interface_name};
use wg_agent::wireguard::{PrivateKey, PublicKey};

fn bench_key_generation(c: &mut Criterion) {
    c.bench_function("key_generation", |b| {
        b.iter(|| {
            let _key = PrivateKey::generate();
        });
    });
}

fn bench_public_key_derivation(c: &mut Criterion) {
    let private_key = PrivateKey::generate();
    
    c.bench_function("public_key_derivation", |b| {
        b.iter(|| {
            let _public = black_box(&private_key).public_key();
        });
    });
}

fn bench_config_parsing(c: &mut Criterion) {
    let toml_data = r#"
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1420

[[network.peers]]
name = "peer1"
public_key = "test-key"
endpoint = "192.168.1.1:51820"
allowed_ips = ["10.0.0.0/24"]

[[network.peers]]
name = "peer2"
public_key = "test-key-2"
endpoint = "192.168.1.2:51820"
allowed_ips = ["10.0.1.0/24"]
"#;

    c.bench_function("config_parsing_toml", |b| {
        b.iter(|| {
            let _config: Config = toml::from_str(black_box(toml_data)).unwrap();
        });
    });
}

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");
    
    group.bench_function("network_name_valid", |b| {
        b.iter(|| {
            let _ = validate_network_name(black_box("test-network-123"));
        });
    });
    
    group.bench_function("interface_name_valid", |b| {
        b.iter(|| {
            let _ = validate_interface_name(black_box("wg0"));
        });
    });
    
    group.finish();
}

fn bench_monitoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("monitoring");
    
    let monitor = Monitor::new();
    monitor.register_network("test".to_string());
    
    group.bench_function("update_state", |b| {
        b.iter(|| {
            monitor.update_state(black_box("test"), ConnectionState::Connected);
        });
    });
    
    group.bench_function("update_traffic", |b| {
        b.iter(|| {
            monitor.update_traffic(black_box("test"), 1000, 2000);
        });
    });
    
    group.bench_function("record_handshake", |b| {
        b.iter(|| {
            monitor.record_handshake(black_box("test"), true);
        });
    });
    
    group.bench_function("get_stats", |b| {
        b.iter(|| {
            let _ = monitor.get_stats(black_box("test"));
        });
    });
    
    group.bench_function("health_check", |b| {
        b.iter(|| {
            let _ = monitor.health_check();
        });
    });
    
    group.finish();
}

fn bench_metrics_export(c: &mut Criterion) {
    let monitor = Monitor::new();
    monitor.register_network("test".to_string());
    monitor.update_traffic("test", 5000, 3000);
    monitor.update_peers("test", 5, 4, 3);
    
    let metrics = monitor.metrics();
    
    let mut group = c.benchmark_group("metrics_export");
    
    group.bench_function("prometheus", |b| {
        b.iter(|| {
            let _ = metrics.export_prometheus();
        });
    });
    
    group.bench_function("json", |b| {
        b.iter(|| {
            let _ = metrics.export_json();
        });
    });
    
    group.finish();
}

fn bench_network_config_with_peers(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_with_peers");
    
    for peer_count in [1, 5, 10, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(peer_count),
            peer_count,
            |b, &count| {
                b.iter(|| {
                    let mut network = NetworkConfig::default();
                    network.name = "test".to_string();
                    network.interface = "wg0".to_string();
                    
                    for i in 0..count {
                        let mut peer = PeerConfig::default();
                        peer.name = Some(format!("peer{}", i));
                        peer.public_key = format!("key{}", i);
                        peer.allowed_ips = vec![format!("10.0.{}.0/24", i)];
                        network.peers.push(peer);
                    }
                    
                    black_box(network);
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_key_generation,
    bench_public_key_derivation,
    bench_config_parsing,
    bench_validation,
    bench_monitoring,
    bench_metrics_export,
    bench_network_config_with_peers,
);

criterion_main!(benches);
