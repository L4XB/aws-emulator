// SPDX-License-Identifier: CC0-1.0

//! EC2 Query lifecycle and isolation for secondary IPv4 and IPv6 VPC associations.
use super::ec2_query::{call, element, emulator, error_code};
use super::wire::{ipv4_association, ipv6_association, param, Param};
use smithy_server_runtime::MultiServiceRouter;

const REGION: &str = "us-east-1";

async fn ok(router: &MultiServiceRouter, action: &str, params: &[Param]) -> String {
    let (code, body) = call(router, REGION, action, params).await;
    assert_eq!(code, 200, "{action}: {body}");
    body
}

#[tokio::test]
async fn ipv6_disassociation_preserves_the_primary_and_refuses_dependencies() {
    let router = emulator().0;
    let created = ok(
        &router,
        "CreateVpc",
        &[
            param("CidrBlock", "10.20.0.0/16"),
            param("Ipv6CidrBlock", "2001:db8:100::/56"),
            param("Ipv6Pool", "ipv6pool-ec2-0123456789abcdef0"),
        ],
    )
    .await;
    let vpc_id = element(&created, "vpcId").unwrap();
    let primary = ipv4_association("primary", &created, "associationId");
    let ipv6 = ipv6_association("owned IPv6", &created, "associationId");
    let release = [param("AssociationId", &ipv6)];
    let (status, body) = call(
        &router,
        REGION,
        "DisassociateVpcCidrBlock",
        &[param("AssociationId", &primary)],
    )
    .await;
    assert_eq!(
        (status, error_code(&body)),
        (400, "OperationNotPermitted".into())
    );
    let (status, body) = call(&router, "eu-west-1", "DisassociateVpcCidrBlock", &release).await;
    assert_eq!(
        (status, error_code(&body)),
        (400, "InvalidVpcCidrBlockAssociationID.NotFound".into())
    );

    let subnet = ok(
        &router,
        "CreateSubnet",
        &[param("VpcId", &vpc_id), param("CidrBlock", "10.20.1.0/24")],
    )
    .await;
    let subnet_id = element(&subnet, "subnetId").unwrap();
    ok(
        &router,
        "AssociateSubnetCidrBlock",
        &[
            param("SubnetId", &subnet_id),
            param("Ipv6CidrBlock", "2001:db8:100:1::/64"),
        ],
    )
    .await;
    let (status, body) = call(&router, REGION, "DisassociateVpcCidrBlock", &release).await;
    assert_eq!(
        (status, error_code(&body)),
        (400, "DependencyViolation".into())
    );
    let describe = [param("VpcId.1", &vpc_id)];
    assert!(ok(&router, "DescribeVpcs", &describe).await.contains(&ipv6));
    ok(&router, "DeleteSubnet", &[param("SubnetId", &subnet_id)]).await;

    let body = ok(&router, "DisassociateVpcCidrBlock", &release).await;
    assert_eq!(element(&body, "vpcId").as_deref(), Some(vpc_id.as_str()));
    assert_eq!(
        element(&body, "associationId").as_deref(),
        Some(ipv6.as_str())
    );
    assert_eq!(
        element(&body, "ipv6CidrBlock").as_deref(),
        Some("2001:db8:100::/56")
    );
    assert_eq!(element(&body, "state").as_deref(), Some("disassociating"));
    let after = ok(&router, "DescribeVpcs", &describe).await;
    assert!(!after.contains(&ipv6));
    assert!(after.contains(&primary));
}

#[tokio::test]
async fn secondary_ipv4_can_be_reused_after_release() {
    let router = emulator().0;
    let created = ok(&router, "CreateVpc", &[param("CidrBlock", "10.10.0.0/16")]).await;
    let vpc_id = element(&created, "vpcId").unwrap();
    let acquire = [param("VpcId", &vpc_id), param("CidrBlock", "10.11.0.0/16")];
    let acquired = ok(&router, "AssociateVpcCidrBlock", &acquire).await;
    let id = element(&acquired, "associationId").unwrap();
    let release = [param("AssociationId", &id)];
    let body = ok(&router, "DisassociateVpcCidrBlock", &release).await;
    assert_eq!(element(&body, "cidrBlock").as_deref(), Some("10.11.0.0/16"));
    assert_eq!(element(&body, "state").as_deref(), Some("disassociating"));
    assert_eq!(
        element(&body, "associationId").as_deref(),
        Some(id.as_str())
    );
    let (status, body) = call(&router, REGION, "DisassociateVpcCidrBlock", &release).await;
    assert_eq!(
        (status, error_code(&body)),
        (400, "InvalidVpcCidrBlockAssociationID.NotFound".into())
    );
    let next = ok(&router, "AssociateVpcCidrBlock", &acquire).await;
    assert_ne!(
        element(&next, "associationId").as_deref(),
        Some(id.as_str())
    );
}
