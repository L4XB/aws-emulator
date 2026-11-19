// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P5a): `ListTags` on a Studio DOMAIN's ARN, straight after `CreateDomain`.
//!
//! `CreateDomain$Tags`: "Tags to associated with the Domain. Each tag consists of a key and
//! an optional value. Tag keys must be unique per resource." and `ListTags`: "Returns the tags
//! for the specified SageMaker resource." Upstream stored the create's tags on the domain
//! record but resolved no domain in the ARN -> tag-set lookup, so `ListTags` refused the
//! domain's own ARN as a resource that does not exist (the AWS provider for Terraform reads
//! the tags right after the create, so every `aws_sagemaker_domain` failed to apply).
//!
//! The rows differ in how many tags the create carries (none, one, several), and the last
//! check pins that an ARN naming no domain is still refused.

use serde_json::{json, Value};

use crate::inference_deployment_lifecycle::{error_code, Emulator, ACCOUNT};

/// The execution role every domain of this suite names; `CreateDomain` stores it as given.
const EXECUTION_ROLE: &str = "arn:aws:iam::123456789012:role/service-role/StudioRole";

/// The (key, value) pairs of a `Tags` list, in the order the service reported them.
fn pairs(body: &Value) -> Vec<(String, String)> {
    body.get("Tags")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("expected a Tags list in {body}"))
        .iter()
        .map(|tag| {
            (
                tag.get("Key").and_then(Value::as_str).unwrap_or_default().to_string(),
                tag.get("Value").and_then(Value::as_str).unwrap_or_default().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn list_tags_reports_the_tags_create_domain_stored() {
    let emulator = Emulator::new();
    let cases: [(&str, Vec<(&str, &str)>); 3] = [
        ("no tags", vec![]),
        ("one tag", vec![("Name", "studio")]),
        (
            "three tags",
            vec![("Name", "studio"), ("Environment", "dev"), ("Project", "iac-eval")],
        ),
    ];
    for (index, (case, tags)) in cases.iter().enumerate() {
        let request = json!({
            "DomainName": format!("tagged-domain-{index}"),
            "AuthMode": "IAM",
            "DefaultUserSettings": { "ExecutionRole": EXECUTION_ROLE },
            "Tags": tags
                .iter()
                .map(|(key, value)| json!({ "Key": key, "Value": value }))
                .collect::<Vec<_>>(),
        });
        let created = emulator.ok("CreateDomain", &request).await;
        let arn = created
            .get("DomainArn")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("case {case:?}: expected a DomainArn in {created}"))
            .to_string();

        // Straight after the create, while the domain is still provisioning.
        let (status, listed) = emulator
            .call("ListTags", &json!({ "ResourceArn": arn }))
            .await;
        assert_eq!(
            status, 200,
            "case {case:?}: ListTags on the new domain {arn} must succeed, got {status} with \
             {listed}"
        );
        let expected: Vec<(String, String)> = tags
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        assert_eq!(
            pairs(&listed),
            expected,
            "case {case:?}: ListTags must report the tags CreateDomain carried"
        );

        // And the tag writers reach the same record.
        emulator
            .ok(
                "AddTags",
                &json!({ "ResourceArn": arn, "Tags": [{ "Key": "Added", "Value": "later" }] }),
            )
            .await;
        let after_add = emulator.ok("ListTags", &json!({ "ResourceArn": arn })).await;
        assert!(
            pairs(&after_add).contains(&("Added".to_string(), "later".to_string())),
            "case {case:?}: AddTags on the domain must be visible to ListTags, got {after_add}"
        );
    }

    // An ARN in the domain form that names no domain is still refused.
    let missing = format!("arn:aws:sagemaker:us-east-1:{ACCOUNT}:domain/d-doesnotexist00");
    let (status, refused) = emulator
        .call("ListTags", &json!({ "ResourceArn": missing }))
        .await;
    assert_eq!(
        status, 400,
        "case {:?}: ListTags on a domain ARN no domain holds must be refused, got {status} \
         with {refused}",
        "missing domain"
    );
    assert!(
        error_code(&refused).contains("ValidationException"),
        "case {:?}: expected a ValidationException, got {refused}",
        "missing domain"
    );
}
