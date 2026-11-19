// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P3c, DERIVED, NOT DOCUMENTED): a domain created without Cognito still
//! reports `CognitoOptions` (`Enabled` false) in its status.
//!
//! `ElasticsearchDomainStatus$CognitoOptions` is "The CognitoOptions for the specified domain";
//! the API reference does not say the member is always present. The AWS provider for Terraform
//! dereferences it on every read of an `aws_elasticsearch_domain`
//! (internal/service/elasticsearch/flex.go v5.100.0, `flattenCognitoOptions`), and its
//! acceptance test `TestAccElasticsearchDomain_basic` reads a domain created without
//! `cognito_options` from AWS; upstream omitted the member and the provider crashed.
//!
//! The rows differ along the reader (`CreateElasticsearchDomain`'s answer,
//! `DescribeElasticsearchDomain`) and pin that a domain created WITH Cognito still reports what
//! it was given.

use serde_json::json;

use crate::harness::{call_in, router};

const REGION: &str = "us-east-1";

#[tokio::test]
async fn a_domain_without_cognito_reports_cognito_disabled() {
    let router = router();
    let created = call_in(
        &router,
        REGION,
        "POST",
        "/2015-01-01/es/domain",
        Some(&json!({
            "DomainName": "fork-no-cognito",
            "ElasticsearchVersion": "7.10",
            "EBSOptions": { "EBSEnabled": true, "VolumeSize": 10 }
        })),
    )
    .await;
    assert_eq!(created.status, 200, "case {:?}: create failed: {}", "create", created.body);
    for (case, answer) in [
        ("CreateElasticsearchDomain", created),
        (
            "DescribeElasticsearchDomain",
            call_in(&router, REGION, "GET", "/2015-01-01/es/domain/fork-no-cognito", None).await,
        ),
    ] {
        assert_eq!(answer.status, 200, "case {case:?}: expected 200, got {}", answer.body);
        assert_eq!(
            answer.at(case, "/DomainStatus/CognitoOptions/Enabled"),
            &json!(false),
            "case {case:?}: CognitoOptions must be reported, disabled"
        );
    }

    let with = call_in(
        &router,
        REGION,
        "POST",
        "/2015-01-01/es/domain",
        Some(&json!({
            "DomainName": "fork-with-cognito",
            "ElasticsearchVersion": "7.10",
            "EBSOptions": { "EBSEnabled": true, "VolumeSize": 10 },
            "CognitoOptions": {
                "Enabled": true,
                "UserPoolId": "us-east-1_abcdefghi",
                "IdentityPoolId": "us-east-1:11111111-2222-3333-4444-555555555555",
                "RoleArn": "arn:aws:iam::000000000000:role/CognitoAccessForAmazonES"
            }
        })),
    )
    .await;
    if with.status == 200 {
        assert_eq!(
            with.at("with cognito", "/DomainStatus/CognitoOptions/Enabled"),
            &json!(true),
            "case {:?}: the configured CognitoOptions must be reported as given",
            "with cognito"
        );
    }
}
