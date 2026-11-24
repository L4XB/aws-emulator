// SPDX-License-Identifier: CC0-1.0

//! `authorization.k8s.io/v1`: what a client asks the API server it is ALLOWED to
//! do.
//!
//! A client cannot tell from its credential what the cluster will let it do — the
//! decision is the cluster's, taken from the access policies its access entry
//! associates and the RBAC objects it holds ([`super::kubernetes_rbac`]) — so it
//! asks the API server, by creating a `SelfSubjectAccessReview`:
//! "SelfSubjectAccessReview checks whether or not the current user can perform an
//! action. Not filling in a spec.namespace means 'in all namespaces'", and the
//! answer's `status.allowed` is that decision. It is what `kubectl auth can-i`
//! sends, and what any client that pre-flights its own permissions sends.
//!
//! The review is answered by the SAME authorizer that admits a request
//! ([`super::kubernetes_rbac::admits`]) over the SAME reading of what a request
//! addresses ([`super::kubernetes_rbac::RequestAttributes`]), so a review can never
//! disagree with what the server would do with the request it describes. Nothing is
//! stored: the review is computed for the request that asked, which is why
//! Kubernetes answers it `201 Created` with a null creation timestamp and no name.
//!
//! Kubernetes lets EVERY authenticated client make that request: the
//! `system:basic-user` ClusterRole — "allows a user read-only access to basic
//! information about themselves" — is bound to `system:authenticated` and carries
//! `selfsubjectaccessreviews`, so the permission is one of the published ones every
//! cluster has ([`super::kubernetes_rbac::published_group_rules`]) rather than an
//! object a caller has to create.

use super::kubernetes_objects::ClusterObjects;
use super::kubernetes_rbac::{admits, RequestAttributes, SubjectIdentity};

/// The API group a client asks about its own permissions in.
pub const AUTHORIZATION_API_GROUP: &str = "authorization.k8s.io";

/// The version of that group this API server serves.
pub const AUTHORIZATION_GROUP_VERSION: &str = "v1";

/// The `apiVersion` every object of the group carries.
pub const AUTHORIZATION_API_VERSION: &str = "authorization.k8s.io/v1";

/// The resource a client creates to be told whether it may do something.
pub const SELF_SUBJECT_ACCESS_REVIEWS_RESOURCE: &str = "selfsubjectaccessreviews";

/// The `kind` of that resource's object.
const SELF_SUBJECT_ACCESS_REVIEW_KIND: &str = "SelfSubjectAccessReview";

/// The `apiVersion` a discovery document itself carries, which is the core one
/// whatever group it describes.
const DISCOVERY_API_VERSION: &str = "v1";

/// The `status.reason` the review carries for a decision the authorizer took on the
/// permissions the identity holds, which is the sentence Kubernetes' RBAC authorizer
/// words an allow in.
const ALLOWED_BY_RBAC: &str = "RBAC: allowed by the permissions of the requesting user";

/// The `status.reason` of a review the authorizer did not allow: Kubernetes' RBAC
/// authorizer reports that no rule it holds allowed the request.
const NOT_ALLOWED_BY_RBAC: &str = "RBAC: no rule allowed the requested action";

/// One `GroupVersionForDiscovery` of this group.
fn group_version() -> ::serde_json::Value {
    ::serde_json::json!({
        "groupVersion": AUTHORIZATION_API_VERSION,
        "version": AUTHORIZATION_GROUP_VERSION,
    })
}

/// The group's entry in the `APIGroupList` served at `/apis`.
#[must_use]
pub fn api_group_entry() -> ::serde_json::Value {
    ::serde_json::json!({
        "name": AUTHORIZATION_API_GROUP,
        "versions": [group_version()],
        "preferredVersion": group_version(),
    })
}

/// The `APIGroup` discovery document of the group, which a client reads before it
/// addresses it.
#[must_use]
pub fn api_group_document() -> ::serde_json::Value {
    ::serde_json::json!({
        "kind": "APIGroup",
        "apiVersion": DISCOVERY_API_VERSION,
        "name": AUTHORIZATION_API_GROUP,
        "versions": [group_version()],
        "preferredVersion": group_version(),
    })
}

/// The `APIResourceList` of `authorization.k8s.io/v1`: the resource this server
/// serves in the group and what may be done to it.
#[must_use]
pub fn api_resource_list_document() -> ::serde_json::Value {
    ::serde_json::json!({
        "kind": "APIResourceList",
        "apiVersion": DISCOVERY_API_VERSION,
        "groupVersion": AUTHORIZATION_API_VERSION,
        "resources": [{
            "name": SELF_SUBJECT_ACCESS_REVIEWS_RESOURCE,
            "singularName": "selfsubjectaccessreview",
            "namespaced": false,
            "kind": SELF_SUBJECT_ACCESS_REVIEW_KIND,
            "verbs": ["create"],
        }],
    })
}

/// The `SelfSubjectAccessReview` the API server answers: the review the client sent,
/// with the decision the authorizer took on the action its spec describes.
///
/// The decision is taken for `identity` — who the request is being served as — over
/// `objects`, the cluster's own RBAC objects, through the authorizer that admits
/// every other request on this address. A review whose spec describes no action at
/// all (neither `resourceAttributes` nor `nonResourceAttributes`) is refused the way
/// the API server refuses it: there is nothing to decide.
#[must_use]
pub fn self_subject_access_review(
    objects: &ClusterObjects,
    identity: &SubjectIdentity,
    review: &::serde_json::Value,
    attributes: &RequestAttributes,
) -> ::serde_json::Value {
    let allowed = admits(objects, identity, attributes);
    let spec = review
        .get("spec")
        .cloned()
        .unwrap_or_else(|| ::serde_json::json!({}));
    ::serde_json::json!({
        "kind": SELF_SUBJECT_ACCESS_REVIEW_KIND,
        "apiVersion": AUTHORIZATION_API_VERSION,
        "metadata": { "creationTimestamp": ::serde_json::Value::Null },
        // "Spec holds information about the request being evaluated": echoed back as
        // the client wrote it, which is what the API server answers with.
        "spec": spec,
        "status": {
            "allowed": allowed,
            // "Denied is true if the action would be denied, otherwise false": the
            // authorizer of this server either allows an action or holds no rule
            // that allows it, which is not the same as an explicit deny.
            "denied": false,
            "reason": if allowed { ALLOWED_BY_RBAC } else { NOT_ALLOWED_BY_RBAC },
        },
    })
}

/// What the action a review's spec describes ADDRESSES, read into the same
/// attributes the authorizer decides a request by, or `None` for a spec that
/// describes no action.
///
/// "ResourceAuthorizationAttributes describes information for a resource access
/// request" — `group`, `resource`, `subresource`, `namespace`, `name` and `verb` —
/// and `NonResourceAttributes` describes one by `path` and `verb`. Both are read
/// here into [`RequestAttributes`], the one reading the authorizer takes a decision
/// over, so a review and the request it describes are decided alike. A spec that
/// names neither is nothing to decide.
#[must_use]
pub fn reviewed_attributes(review: &::serde_json::Value) -> Option<RequestAttributes> {
    let spec = review.get("spec")?;
    let member = |attributes: &::serde_json::Value, name: &str| -> String {
        attributes
            .get(name)
            .and_then(::serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let resource = spec
        .get("resourceAttributes")
        .filter(|value| !value.is_null());
    let non_resource = spec
        .get("nonResourceAttributes")
        .filter(|value| !value.is_null());
    if resource.is_some() == non_resource.is_some()
        || resource
            .or(non_resource)
            .is_some_and(|value| !value.is_object())
    {
        return None;
    }
    if let Some(resource) = resource {
        return Some(RequestAttributes {
            verb: member(resource, "verb"),
            api_group: member(resource, "group"),
            resource: member(resource, "resource"),
            subresource: member(resource, "subresource"),
            namespace: member(resource, "namespace"),
            name: member(resource, "name"),
            // A resource review addresses a resource rather than a path of the
            // server's own, so it names none.
            path: String::new(),
        });
    }
    let non_resource = non_resource?;
    Some(RequestAttributes {
        verb: member(non_resource, "verb"),
        path: member(non_resource, "path"),
        ..RequestAttributes::default()
    })
}

/// The message the API server refuses a review whose spec describes no action with:
/// "exactly one of nonResourceAttributes or resourceAttributes must be specified".
pub const NO_ATTRIBUTES_MESSAGE: &str =
    "spec.resourceAttributes: Required value: exactly one of nonResourceAttributes or \
     resourceAttributes must be specified";
