// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P9): `DeleteSubnetGroup`, which upstream answered 501.
//!
//! "Deletes a subnet group. You cannot delete a subnet group if it is associated with any DAX
//! clusters." (`DeleteSubnetGroup`). The rows differ along the state of the group: free (deleted,
//! then gone for `DescribeSubnetGroups`), named in another case (resolves case-insensitively),
//! used by a cluster (`SubnetGroupInUseFault`, group kept) and absent
//! (`SubnetGroupNotFoundFault`). Every DAX fault goes out with HTTP 400 on this wire (the
//! modelled 404 of the not-found family is inert, see `dax_client_fault_wire_status`).

use serde_json::json;

use crate::dax_account_network::{account_subnet_ids, call, emulator, error_code, REGION};

#[tokio::test]
async fn a_subnet_group_is_deleted_unless_a_cluster_uses_it() {
    let router = emulator();
    let subnets = account_subnet_ids(&router, REGION).await;
    for name in ["free-group", "busy-group"] {
        let (status, body) = call(
            &router,
            "CreateSubnetGroup",
            json!({ "SubnetGroupName": name, "SubnetIds": [subnets[0], subnets[1]] }),
        )
        .await;
        assert_eq!(status, 200, "case {name:?}: CreateSubnetGroup got {body:?}");
    }
    let (status, body) = call(
        &router,
        "CreateCluster",
        json!({
            "ClusterName": "busy",
            "NodeType": "dax.r4.large",
            "ReplicationFactor": 1,
            "IamRoleArn": "arn:aws:iam::000000000000:role/DAXRole",
            "SubnetGroupName": "busy-group",
        }),
    )
    .await;
    assert_eq!(status, 200, "case {:?}: CreateCluster got {body:?}", "busy cluster");

    // A free group, named in another case, is deleted and no longer described.
    let (status, body) = call(&router, "DeleteSubnetGroup", json!({ "SubnetGroupName": "FREE-GROUP" })).await;
    assert_eq!(status, 200, "case {:?}: DeleteSubnetGroup got {body:?}", "free group");
    let (status, body) = call(&router, "DescribeSubnetGroups", json!({ "SubnetGroupNames": ["free-group"] })).await;
    assert_eq!(
        (status, error_code(&body).as_str()),
        (400, "SubnetGroupNotFoundFault"),
        "case {:?}: the deleted group must be gone, got {status} {body:?}",
        "free group"
    );

    // A group a cluster uses is refused and kept.
    let (status, body) = call(&router, "DeleteSubnetGroup", json!({ "SubnetGroupName": "busy-group" })).await;
    assert_eq!(
        (status, error_code(&body).as_str()),
        (400, "SubnetGroupInUseFault"),
        "case {:?}: expected SubnetGroupInUseFault, got {status} {body:?}",
        "busy group"
    );
    let (status, body) = call(&router, "DescribeSubnetGroups", json!({ "SubnetGroupNames": ["busy-group"] })).await;
    assert_eq!(status, 200, "case {:?}: the refused group must be kept, got {body:?}", "busy group");

    // An absent group is SubnetGroupNotFoundFault.
    let (status, body) = call(&router, "DeleteSubnetGroup", json!({ "SubnetGroupName": "never-created" })).await;
    assert_eq!(
        (status, error_code(&body).as_str()),
        (400, "SubnetGroupNotFoundFault"),
        "case {:?}: expected SubnetGroupNotFoundFault, got {status} {body:?}",
        "absent group"
    );
}
