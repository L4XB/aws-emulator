// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P3b, DERIVED, NOT DOCUMENTED): `RunInstances` with an EMPTY
//! `Placement.AvailabilityZone` (`Placement.AvailabilityZone=` on the wire) launches like one
//! that names no zone.
//!
//! "If you specify neither one, Amazon EC2 automatically selects an Availability Zone for you"
//! (`Placement$AvailabilityZone`). The AWS provider for Terraform sends the empty value for every
//! `aws_instance` without `availability_zone` (internal/service/ec2/ec2_instance.go v5.100.0,
//! `buildInstanceOpts`); upstream refused it as the zone "" ("Invalid value '' for
//! availabilityZone"), while it already read an empty `Placement.GroupName` and `HostId` as
//! absent.
//!
//! The rows differ along the form of the member (empty, absent, a real zone) and pin that a
//! zone the Region does not have is still refused.

use super::ec2_query;

/// The Region every call here is signed for.
const REGION: &str = "us-west-2";

/// The image every launch names.
const AMI: &str = "ami-0123456789abcdef0";

/// One `(key, value)` request parameter.
fn p(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

#[tokio::test]
async fn an_empty_availability_zone_launches_like_an_absent_one() {
    let (router, _engine) = ec2_query::emulator();

    // What the emulator picks when the launch names no zone: the reference row.
    let launch = |extra: Vec<(String, String)>| {
        let router = &router;
        async move {
            let mut params = vec![p("ImageId", AMI), p("MinCount", "1"), p("MaxCount", "1")];
            params.extend(extra);
            ec2_query::call(router, REGION, "RunInstances", &params).await
        }
    };
    let (status, absent) = launch(vec![]).await;
    assert_eq!(status, 200, "case {:?}: expected 200, got {status} with {absent}", "absent");
    let picked = ec2_query::element(&absent, "availabilityZone")
        .unwrap_or_else(|| panic!("case {:?}: no availabilityZone in {absent}", "absent"));

    for (case, extra) in [
        ("empty zone", vec![p("Placement.AvailabilityZone", "")]),
        (
            "empty zone and empty group name, as the provider sends them",
            vec![p("Placement.AvailabilityZone", ""), p("Placement.GroupName", "")],
        ),
    ] {
        let (status, body) = launch(extra).await;
        assert_eq!(status, 200, "case {case:?}: expected 200, got {status} with {body}");
        let zone = ec2_query::element(&body, "availabilityZone");
        assert_eq!(
            zone.as_deref(),
            Some(picked.as_str()),
            "case {case:?}: expected the zone a launch naming none gets ({picked:?}), got {zone:?}"
        );
    }

    // A real zone of the Region is still honoured, and one it does not have still refused.
    let named = format!("{REGION}b");
    let (status, body) = launch(vec![p("Placement.AvailabilityZone", &named)]).await;
    assert_eq!(status, 200, "case {:?}: expected 200, got {status} with {body}", "named zone");
    assert_eq!(
        ec2_query::element(&body, "availabilityZone").as_deref(),
        Some(named.as_str()),
        "case {:?}: the named zone must be the instance's",
        "named zone"
    );
    let (status, body) = launch(vec![p("Placement.AvailabilityZone", "us-east-1a")]).await;
    assert_eq!(
        (status, ec2_query::error_code(&body).as_str()),
        (400, "InvalidParameterValue"),
        "case {:?}: a zone outside the Region must be refused, got {status} with {body}",
        "foreign zone"
    );
}
