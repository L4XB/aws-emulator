// SPDX-License-Identifier: CC0-1.0

//! The ADDRESS a DB cluster and its instances ANNOUNCE for a client to connect to.
//!
//! `DBCluster$Endpoint` is "the connection endpoint for the primary instance of the DB
//! cluster", `Endpoint$Address` is "the DNS address of the DB instance", and both
//! `DBCluster$Port` and `Endpoint$Port` are "the port that the database engine is listening
//! on": the pair is the one thing a client DIALS to reach the graph, because the Neptune
//! data plane is addressed at the cluster's own endpoint and nowhere else. A pair naming an
//! address this process does not answer on therefore makes the whole data plane
//! unreachable.
//!
//! IaC-Eval fork (P4): the PORT announced is the one the request asked for, else the
//! documented default 8182, in every case; only the HOST still follows the rule below.
//!
//! So the pair announced is the address this emulator is REACHED at whenever it has one,
//! and the AWS-shaped DNS host with the documented engine port only on an emulator that is
//! served on no port at all — an in-process call with no socket behind it, where there is
//! no address of this process to announce.
//!
//! WHICH address is this emulator's own is the tree's one reading of that question
//! (`service_common::data_plane_addressing::reached_emulator_host_and_port`), so this
//! service writes no second rule for it.

/// The hosts and the port one cluster announces: `DBCluster$Endpoint`,
/// `DBCluster$ReaderEndpoint` and `DBCluster$Port`, resolved once when the cluster is
/// created and stored on its record, so every later read reports the same values.
#[derive(Debug, Clone)]
pub struct ClusterAddress {
    pub endpoint: String,
    pub reader_endpoint: String,
    pub port: i32,
}

/// The `host` and `port` this emulator is reached at, or `None` when no request addressed
/// it directly and it is served on no port at all — the one case where there is no address
/// of this process to announce.
pub fn served_address(
    ctx: &::smithy_server_runtime::OperationContext<'_>,
) -> Option<(String, i32)> {
    ::service_common::data_plane_addressing::reached_emulator_host_and_port(
        ctx.parts(),
        ctx.endpoint().as_str(),
    )
    .map(|(host, port)| (host, i32::from(port)))
}

/// The address a CLUSTER announces.
///
/// One process serves every cluster's data plane, so on a served emulator the writer and
/// the reader endpoint are the same host — it is the only address a client can reach — and
/// the port is the one this process listens on, whatever port the request asked for, because
/// that is where the engine answers.
pub fn announced_cluster_address(
    ctx: &::smithy_server_runtime::OperationContext<'_>,
    identifier: &str,
    endpoint_token: &str,
    region: &str,
    requested_port: Option<i32>,
) -> ClusterAddress {
    match served_address(ctx) {
        // IaC-Eval fork (P4): the PORT is the one the request asked for, else the documented
        // engine default — `CreateDBCluster$Port`: "The port number on which the instances in
        // the DB cluster accept connections. Default: 8182" — not this emulator's own API
        // port, which no `CreateDBCluster` ever asked for. The host is still the address this
        // emulator is reached at (upstream's choice), so `<Endpoint>:<Port>` no longer
        // dials the emulator's data plane.
        Some((host, _served_port)) => ClusterAddress {
            endpoint: host.clone(),
            reader_endpoint: host,
            port: requested_port.unwrap_or(super::DEFAULT_PORT),
        },
        None => ClusterAddress {
            endpoint: cluster_dns_name(identifier, endpoint_token, region, "cluster-"),
            reader_endpoint: cluster_dns_name(identifier, endpoint_token, region, "cluster-ro-"),
            port: requested_port.unwrap_or(super::DEFAULT_PORT),
        },
    }
}

/// The address an INSTANCE of a cluster announces: the same address its cluster is reached
/// at, since one process serves both.
pub fn announced_instance_address(
    ctx: &::smithy_server_runtime::OperationContext<'_>,
    identifier: &str,
    endpoint_token: &str,
    region: &str,
    requested_port: Option<i32>,
    cluster_port: i32,
) -> (String, i32) {
    match served_address(ctx) {
        // IaC-Eval fork (P4): the instance reports its cluster's port —
        // `CreateDBInstance$Port`: "Not applicable. The port is managed by the DB cluster."
        // — rather than this emulator's own API port.
        Some((host, _served_port)) => (host, cluster_port),
        None => (
            // An instance in a cluster inherits the cluster's endpoint shard label; only
            // its own DNS label differs.
            format!("{identifier}.{endpoint_token}.{region}.neptune.amazonaws.com"),
            requested_port.unwrap_or(cluster_port),
        ),
    }
}

/// One of a cluster's AWS-shaped endpoint host names: the identifier, the shard label the
/// cluster minted behind `role` (`cluster-` for the writer, `cluster-ro-` for the reader),
/// and the region's Neptune DNS suffix.
fn cluster_dns_name(identifier: &str, endpoint_token: &str, region: &str, role: &str) -> String {
    format!("{identifier}.{role}{endpoint_token}.{region}.neptune.amazonaws.com")
}
