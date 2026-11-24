// SPDX-License-Identifier: CC0-1.0

//! What a client is told when it asks a cluster's Kubernetes API server whether it
//! MAY do something.
//!
//! The RULE under test: the API server serves `authorization.k8s.io/v1` — its
//! discovery documents and the `selfsubjectaccessreviews` resource — and a review
//! created there is answered by the SAME authorizer that admits a request:
//! "SelfSubjectAccessReview checks whether or not the current user can perform an
//! action", so `status.allowed` is true exactly when the request the review
//! describes would be admitted for the identity that asked. That is what
//! `kubectl auth can-i` and every client that pre-flights its permissions send, and
//! the answer is `201 Created` with the review object.
//!
//! The suites drive that over a TABLE whose rows differ along what the rule
//! generalises over: the verb (a read and several writes), the API group (the core
//! one and `apps`), the scope (a namespaced action and a cluster-scoped one), the
//! kind of attributes (a resource action and a non-resource path), and the
//! permissions the asking identity holds (the cluster's own creator, an identity
//! holding only the read-only access policy, and that policy scoped to one
//! namespace).
//!
//! Each row asserts the review AGAINST the request it describes: the request is
//! made too, and its answer has to agree with the decision the review reported — a
//! review that cannot disagree with the server is the whole point of the resource.

use service_com_amazonaws_eks_awswesleyfrontend::state::{
    access_policy_arn, ACCESS_SCOPE_TYPE_CLUSTER, ACCESS_SCOPE_TYPE_NAMESPACE,
};
use smithy_server_runtime::MultiServiceRouter;

use crate::certificate_authority::REGION;
use crate::kubernetes_api::{dial, kubeconfig_of, poll_until_active, ApiAnswer, Kubeconfig};
use crate::wire::{call, emulator, encode_label, Answer};

/// The access policy AWS publishes as granting read-only access to a cluster.
const VIEW_POLICY: &str = "AmazonEKSViewPolicy";

/// The path the review is created at.
const REVIEWS_PATH: &str = "/apis/authorization.k8s.io/v1/selfsubjectaccessreviews";

/// The namespace a namespace-scoped association is confined to.
const SCOPED_NAMESPACE: &str = "default";

/// Another namespace, which that association does not reach.
const OTHER_NAMESPACE: &str = "kube-system";

/// Create a cluster whose Kubernetes API this suite dials, with the creator's
/// bootstrap admin permissions as the case asks for them.
async fn provisioned_cluster(
    router: &MultiServiceRouter,
    name: &str,
    bootstrap_creator_admin: bool,
) -> Kubeconfig {
    let body = serde_json::json!({
        "name": name,
        "roleArn": "arn:aws:iam::000000000000:role/eksClusterRole",
        "resourcesVpcConfig": { "subnetIds": ["subnet-aaaa1111", "subnet-bbbb2222"] },
        "accessConfig": {
            "authenticationMode": "API",
            "bootstrapClusterCreatorAdminPermissions": bootstrap_creator_admin,
        },
    });
    let created = call(router, REGION, "POST", "/clusters", &body.to_string()).await;
    assert_eq!(
        created.status, 200,
        "setup: expected 200 from CreateCluster({name}), got {created:?}"
    );
    let described = poll_until_active(router, REGION, name).await;
    kubeconfig_of(name, &described)
}

/// `CreateAccessEntry` for the IAM principal the dialling credential speaks for.
async fn create_access_entry(
    router: &MultiServiceRouter,
    cluster: &str,
    principal_arn: &str,
) -> Answer {
    let body = serde_json::json!({ "principalArn": principal_arn });
    call(
        router,
        REGION,
        "POST",
        &format!("/clusters/{}/access-entries", encode_label(cluster)),
        &body.to_string(),
    )
    .await
}

/// `AssociateAccessPolicy`: grant `policy_name` over the whole cluster, or over
/// `namespaces` when the case scopes it to some.
async fn associate_policy(
    router: &MultiServiceRouter,
    cluster: &str,
    principal_arn: &str,
    policy_name: &str,
    namespaces: Option<&[&str]>,
) -> Answer {
    let scope = match namespaces {
        None => serde_json::json!({ "type": ACCESS_SCOPE_TYPE_CLUSTER }),
        Some(namespaces) => serde_json::json!({
            "type": ACCESS_SCOPE_TYPE_NAMESPACE,
            "namespaces": namespaces,
        }),
    };
    let body = serde_json::json!({
        "policyArn": access_policy_arn(policy_name),
        "accessScope": scope,
    });
    call(
        router,
        REGION,
        "POST",
        &format!(
            "/clusters/{}/access-entries/{}/access-policies",
            encode_label(cluster),
            encode_label(principal_arn)
        ),
        &body.to_string(),
    )
    .await
}

/// One row: the action a review describes, and the request that performs it.
struct ActionCase {
    label: &'static str,
    /// `spec.resourceAttributes` of the review.
    attributes: serde_json::Value,
    method: &'static str,
    path: String,
    body: Option<serde_json::Value>,
    /// Whether the read-only access policy admits the action.
    allowed_to_a_reader: bool,
}

/// The table of actions: a read and four writes, across two API groups and both
/// scopes.
fn action_cases() -> Vec<ActionCase> {
    vec![
        ActionCase {
            label: "listing the pods of a namespace",
            attributes: serde_json::json!({
                "resourceAttributes": {
                    "verb": "list",
                    "group": "",
                    "resource": "pods",
                    "namespace": SCOPED_NAMESPACE,
                }
            }),
            method: "GET",
            path: format!("/api/v1/namespaces/{SCOPED_NAMESPACE}/pods"),
            body: None,
            allowed_to_a_reader: true,
        },
        ActionCase {
            label: "reading one namespace",
            attributes: serde_json::json!({
                "resourceAttributes": {
                    "verb": "get",
                    "group": "",
                    "resource": "namespaces",
                    "namespace": OTHER_NAMESPACE,
                    "name": OTHER_NAMESPACE,
                }
            }),
            method: "GET",
            path: format!("/api/v1/namespaces/{OTHER_NAMESPACE}"),
            body: None,
            allowed_to_a_reader: true,
        },
        ActionCase {
            label: "creating a namespace at the cluster scope",
            attributes: serde_json::json!({
                "resourceAttributes": {
                    "verb": "create",
                    "group": "",
                    "resource": "namespaces",
                }
            }),
            method: "POST",
            path: "/api/v1/namespaces".to_string(),
            body: Some(serde_json::json!({ "metadata": { "name": "review-made-this" } })),
            allowed_to_a_reader: false,
        },
        ActionCase {
            label: "creating a config map in a namespace",
            attributes: serde_json::json!({
                "resourceAttributes": {
                    "verb": "create",
                    "group": "",
                    "resource": "configmaps",
                    "namespace": SCOPED_NAMESPACE,
                }
            }),
            method: "POST",
            path: format!("/api/v1/namespaces/{SCOPED_NAMESPACE}/configmaps"),
            body: Some(serde_json::json!({
                "metadata": { "name": "review-config" },
                "data": { "key": "value" },
            })),
            allowed_to_a_reader: false,
        },
        ActionCase {
            label: "creating a deployment in another API group",
            attributes: serde_json::json!({
                "resourceAttributes": {
                    "verb": "create",
                    "group": "apps",
                    "resource": "deployments",
                    "namespace": SCOPED_NAMESPACE,
                }
            }),
            method: "POST",
            path: format!("/apis/apps/v1/namespaces/{SCOPED_NAMESPACE}/deployments"),
            body: Some(serde_json::json!({
                "metadata": { "name": "review-deployment" },
                "spec": {
                    "selector": { "matchLabels": { "app": "review" } },
                    "template": {
                        "metadata": { "labels": { "app": "review" } },
                        "spec": { "containers": [{ "name": "app", "image": "app:1" }] },
                    },
                },
            })),
            allowed_to_a_reader: false,
        },
    ]
}

/// Create the review of `attributes` and answer the decision it reported.
async fn review(config: &Kubeconfig, case: &str, attributes: &serde_json::Value) -> bool {
    let answered = dial(
        config,
        case,
        "POST",
        REVIEWS_PATH,
        Some(&serde_json::json!({
            "apiVersion": "authorization.k8s.io/v1",
            "kind": "SelfSubjectAccessReview",
            "spec": attributes,
        })),
    )
    .await;
    assert_eq!(
        answered.status, 201,
        "case {case:?}: a review must be created, got {answered:?}"
    );
    assert_eq!(
        answered.body["kind"],
        serde_json::json!("SelfSubjectAccessReview"),
        "case {case:?}: the answer is the review object, got {answered:?}"
    );
    assert_eq!(
        answered.body["spec"],
        *attributes,
        "case {case:?}: the review echoes the spec it was asked about, got {answered:?}"
    );
    answered.body["status"]["allowed"]
        .as_bool()
        .unwrap_or_else(|| panic!("case {case:?}: expected status.allowed, got {answered:?}"))
}

/// Make the request the row describes, and assert its answer AGREES with
/// `allowed`: a decision the review reported as denied is the API server's 403, and
/// one it reported as allowed is anything but.
async fn assert_agrees(config: &Kubeconfig, case: &str, allowed: bool, row: &ActionCase) {
    let answered: ApiAnswer = dial(config, case, row.method, &row.path, row.body.as_ref()).await;
    if allowed {
        assert_ne!(
            answered.status, 403,
            "case {case:?}: the review allowed the action, so the request is not forbidden, got \
             {answered:?}"
        );
    } else {
        assert_eq!(
            answered.status, 403,
            "case {case:?}: the review denied the action, so the request is forbidden, got \
             {answered:?}"
        );
    }
}

/// RULE: the group is served — it is advertised in the group list and its resource
/// list names the review resource.
#[tokio::test]
async fn the_authorization_group_is_advertised_and_serves_its_resource() {
    let router = emulator();
    let config = provisioned_cluster(&router, "review-discovery", true).await;

    let groups = dial(&config, "the group list", "GET", "/apis", None).await;
    assert_eq!(
        groups.status, 200,
        "the group list must be served, got {groups:?}"
    );
    let named = groups.body["groups"]
        .as_array()
        .unwrap_or_else(|| panic!("the group list carries groups, got {groups:?}"))
        .iter()
        .any(|group| group["name"] == serde_json::json!("authorization.k8s.io"));
    assert!(
        named,
        "the group list must advertise authorization.k8s.io, got {groups:?}"
    );

    let resources = dial(
        &config,
        "the group's resources",
        "GET",
        "/apis/authorization.k8s.io/v1",
        None,
    )
    .await;
    assert_eq!(
        resources.status, 200,
        "the group's resource list must be served, got {resources:?}"
    );
    let row = resources.body["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("expected resources, got {resources:?}"))
        .iter()
        .find(|row| row["name"] == serde_json::json!("selfsubjectaccessreviews"))
        .cloned()
        .unwrap_or_else(|| panic!("expected the review resource, got {resources:?}"));
    assert_eq!(
        row["kind"],
        serde_json::json!("SelfSubjectAccessReview"),
        "the resource carries its kind, got {row:?}"
    );
    assert_eq!(
        row["namespaced"],
        serde_json::json!(false),
        "a self subject access review is not namespaced, got {row:?}"
    );

    // A review that describes no action at all is nothing to decide, and is
    // refused the way the API server refuses an invalid object.
    let empty = dial(
        &config,
        "a review describing no action",
        "POST",
        REVIEWS_PATH,
        Some(&serde_json::json!({
            "apiVersion": "authorization.k8s.io/v1",
            "kind": "SelfSubjectAccessReview",
            "spec": {},
        })),
    )
    .await;
    assert_eq!(
        empty.status, 422,
        "case a review describing no action: expected the invalid Status, got {empty:?}"
    );
}

/// RULE: for the cluster's own creator — an identity holding the cluster-admin
/// access policy — every action is allowed, and the requests agree.
#[tokio::test]
async fn the_cluster_creator_is_allowed_every_action_it_reviews() {
    let router = emulator();
    let config = provisioned_cluster(&router, "review-admin", true).await;

    for row in action_cases() {
        let case = format!("the cluster creator {}", row.label);
        let allowed = review(&config, &case, &row.attributes).await;
        assert!(
            allowed,
            "case {case:?}: the cluster-admin policy admits every action, got allowed={allowed}"
        );
        assert_agrees(&config, &case, allowed, &row).await;
    }

    // A non-resource path is reviewed the same way: every authenticated client may
    // read the server's own paths, which is what `system:discovery` grants.
    let case = "the cluster creator reading the server's version path";
    let allowed = review(
        &config,
        case,
        &serde_json::json!({ "nonResourceAttributes": { "verb": "get", "path": "/version" } }),
    )
    .await;
    assert!(
        allowed,
        "case {case:?}: an authenticated client may read /version, got allowed={allowed}"
    );
}

/// RULE: an identity holding only the read-only access policy is allowed the reads
/// and denied the writes — and the requests agree, so a view-only principal cannot
/// mutate the cluster.
#[tokio::test]
async fn a_read_only_access_policy_allows_reads_and_denies_writes() {
    let router = emulator();
    let cluster = "review-view-only";
    // No bootstrap entry: the only mapping of the dialling credential is the entry
    // created below, so the decision is the view policy's alone.
    let config = provisioned_cluster(&router, cluster, false).await;
    let principal_arn = service_common::caller_principal::ambient_user_arn();
    let entry = create_access_entry(&router, cluster, &principal_arn).await;
    assert_eq!(
        entry.status, 200,
        "setup: expected 200 from CreateAccessEntry, got {entry:?}"
    );
    let associated = associate_policy(&router, cluster, &principal_arn, VIEW_POLICY, None).await;
    assert_eq!(
        associated.status, 200,
        "setup: expected 200 from AssociateAccessPolicy, got {associated:?}"
    );

    for row in action_cases() {
        let case = format!("a view-only identity {}", row.label);
        let allowed = review(&config, &case, &row.attributes).await;
        assert_eq!(
            allowed, row.allowed_to_a_reader,
            "case {case:?}: expected allowed={}, got allowed={allowed}",
            row.allowed_to_a_reader
        );
        assert_agrees(&config, &case, allowed, &row).await;
    }
}

/// RULE: an access policy scoped to namespaces is confined to them — the same
/// action is allowed in a namespace the association names and denied in one it does
/// not.
#[tokio::test]
async fn a_namespace_scoped_access_policy_is_confined_to_its_namespaces() {
    let router = emulator();
    let cluster = "review-namespace-scoped";
    let config = provisioned_cluster(&router, cluster, false).await;
    let principal_arn = service_common::caller_principal::ambient_user_arn();
    let entry = create_access_entry(&router, cluster, &principal_arn).await;
    assert_eq!(
        entry.status, 200,
        "setup: expected 200 from CreateAccessEntry, got {entry:?}"
    );
    let associated = associate_policy(
        &router,
        cluster,
        &principal_arn,
        VIEW_POLICY,
        Some(&[SCOPED_NAMESPACE]),
    )
    .await;
    assert_eq!(
        associated.status, 200,
        "setup: expected 200 from AssociateAccessPolicy, got {associated:?}"
    );

    // The same read, in the namespace the association names and in one it does
    // not.
    for (namespace, expected) in [(SCOPED_NAMESPACE, true), (OTHER_NAMESPACE, false)] {
        let case = format!("listing pods in {namespace}");
        let allowed = review(
            &config,
            &case,
            &serde_json::json!({
                "resourceAttributes": {
                    "verb": "list",
                    "group": "",
                    "resource": "pods",
                    "namespace": namespace,
                }
            }),
        )
        .await;
        assert_eq!(
            allowed, expected,
            "case {case:?}: expected allowed={expected}, got allowed={allowed}"
        );
        let answered = dial(
            &config,
            &case,
            "GET",
            &format!("/api/v1/namespaces/{namespace}/pods"),
            None,
        )
        .await;
        if expected {
            assert_eq!(
                answered.status, 200,
                "case {case:?}: the review allowed the read, so it is served, got {answered:?}"
            );
        } else {
            assert_eq!(
                answered.status, 403,
                "case {case:?}: the review denied the read, so it is forbidden, got {answered:?}"
            );
        }
    }
}

/// The endpoint cannot choose one of two different actions on behalf of a caller.
#[tokio::test]
async fn ambiguous_action_attributes_are_refused_before_deciding_access() {
    let router = emulator();
    let config = provisioned_cluster(&router, "review-one-action", true).await;
    for spec in [
        serde_json::json!({}),
        serde_json::json!({
            "resourceAttributes": {"namespace": SCOPED_NAMESPACE, "verb": "list", "resource": "pods"},
            "nonResourceAttributes": {"verb": "get", "path": "/version"},
        }),
    ] {
        let request = serde_json::json!({
            "kind": "SelfSubjectAccessReview", "apiVersion": "authorization.k8s.io/v1", "spec": spec,
        });
        let answered = dial(
            &config,
            "invalid action choice",
            "POST",
            REVIEWS_PATH,
            Some(&request),
        )
        .await;
        assert_eq!(answered.status, 422);
        assert!(answered.body.pointer("/status/allowed").is_none());
    }
}
