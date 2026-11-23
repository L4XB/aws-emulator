// SPDX-License-Identifier: CC0-1.0

//! Whether a request may reach a collection's data plane at all, and whether the principal
//! it is signed by may perform what it asks.
//!
//! Three decisions answer that, and AWS's own order is the order they are read in here:
//!
//! 1. the `network` security policies, which say whether the endpoint is reachable — a
//!    collection no network policy covers answers nothing to anybody
//!    ([`crate::state::network_access`]);
//! 2. the SigV4 credential SCOPE the request presents, which has to name this service's own
//!    signing name for the credential to be one for this endpoint at all ([`scoped_for`]);
//! 3. the `data` access policies, which say what the signing principal may do to the
//!    collection and its indexes ([`crate::state::data_access`]).
//!
//! The first and the last live in those two state modules, because a data-access grant is
//! also asked about by a service that resolves a vector store it was handed. What lives
//! HERE is what only the REST surface knows: which signing name and which principal a
//! request's credential speaks for, and which permission the verb and route it asks for
//! takes.
//!
//! # Which signing name a credential is for
//!
//! A SigV4 credential is bound to the service it was scoped to: the signing name is part of
//! the scope the signature is computed over, so a credential (or a presigned request) minted
//! for one service is not a credential for another. A client of a collection endpoint signs
//! with this service's own signing name ([`crate::state::ARN_NAMESPACE`], the `sigv4Name` of
//! `smithy_query(query="service_info")`), which is the signing name both of its published
//! endpoints are reached under — the FIPS endpoint is the same service at another host, not
//! another signing name. A request that presents a scope naming any other service is
//! therefore refused at the front door, before any index, document or query is addressed,
//! and with the same [`forbidden`] document a denial of this endpoint carries. A request
//! that presents no SigV4 scope at all presents nothing to compare, and is left to the two
//! policy decisions above.
//!
//! # Which permission a request takes
//!
//! A data access policy grants named permissions on a `collection/<name>` or an
//! `index/<collection>/<index>` resource, so every route of this surface is one of those
//! resources plus one permission ([`needed`]). The mapping is AWS's own "Supported policy
//! permissions" table read against the OpenSearch REST route each permission describes —
//! `aoss:CreateIndex` for the index create, `aoss:DescribeIndex` for a read of an index's
//! own definition, `aoss:ReadDocument` for a search, a count or a document read,
//! `aoss:WriteDocument` for a document write, a bulk write or a refresh — so no route is
//! authorized under a permission the policy language does not have. A route this surface
//! serves nothing for takes no permission: it is answered the way the engine answers a path
//! it has no handler for, and nothing is authorized for it.
//!
//! # Which principal a request speaks for
//!
//! A data access policy names IAM principals, so the access key id the request signed with
//! has to be resolved to one: a key the account's IAM holds names the user that holds it,
//! and a key this emulator's STS minted for a role session names that session AND the role
//! it is a session of — the two spellings a grant may name it by, as every other reading of
//! a principal in this tree takes them.
//!
//! Three callers are NOT bounded by the data access policies, for the reasons every IAM
//! gate in this tree leaves them unbounded: the account's own published credential (the
//! position an AWS account root credential is in, which holds no IAM records at all), a
//! request this emulator can resolve no identity registry for (an engine carrying no STS or
//! IAM store), and an unsigned request, which presents no credential to resolve. The
//! NETWORK decision above is not about a principal and applies to every request, so an
//! unbounded caller is still refused at a collection no network policy covers.
//!
//! # How a refusal is worded
//!
//! Each decision answers the status its own layer refuses with, and WHICH layer took the
//! decision is what decides whether there is a body at all:
//!
//! * the NETWORK decision is taken at the endpoint's front door, before the collection's
//!   cluster is addressed at all, so nothing has built an error document yet: a request no
//!   network policy admits is answered [`NETWORK_REFUSAL_STATUS`] and ZERO BYTES. That is the
//!   recorded answer of a live collection whose only network policy was deleted — the read and
//!   the write that had just been served, correctly scoped and granted by its data access
//!   policy, were both answered that status with no JSON object at all, repeatedly and with
//!   the collection still ACTIVE. A client of the endpoint reads that status as an
//!   authentication failure, and the empty body is what tells this refusal class apart from
//!   the two that carry one;
//! * the CREDENTIAL SCOPE and the DATA ACCESS decisions are the endpoint's own authorization
//!   answering, so each carries that layer's OpenSearch error document: 403 with the fixed
//!   error type `forbidden` ([`forbidden`]), as recorded for a request presenting a
//!   credential scoped to another service, for a role absent from every data access policy,
//!   and for one whose policy withholds the permission the route takes. The fixed
//!   `forbidden` type is what tells this answer apart from the one OpenSearch's own
//!   permission layer gives, which names itself instead.
//!
//! None of them is an error of the ENGINE: the request is refused before an index, a document
//! or a query is read.

use ::aws_smithy_runtime_api::http::Response as RtResponse;

use crate::state::{
    data_access, ComAmazonawsOpensearchserverlessOpensearchserverlessState as ServiceState,
};

/// The status a request the network policies do not admit is refused with: a client of the
/// endpoint reads it as an authentication failure, which is what the recorded refusal of a
/// collection no network policy covers is.
///
/// It is answered with NO body (see the module docs): the network front door refuses before
/// the collection's cluster is reached, so no error document of the collection's own
/// authorization or engine layer exists to send.
const NETWORK_REFUSAL_STATUS: u16 = 401;

/// The error type a request this endpoint does not authorize is refused under — a credential
/// scoped to another signing service, or a principal no data access policy grants: the fixed
/// text the endpoint's own authorization layer answers a denial with ([`forbidden`]).
const FORBIDDEN: &str = "forbidden";

/// The reason that refusal states: the status line of the answer it carries, which is what
/// an authorization layer that has not asked the engine anything can say about it.
const FORBIDDEN_REASON: &str = "403 Forbidden";

/// The document a request this endpoint does not authorize is refused with: the status, and
/// the fixed `forbidden` error type a denial on this endpoint carries.
///
/// Not one of the ENGINE's errors ([`super::error`]): no index, document or query was read,
/// because the request was refused before the collection's own records were touched at all,
/// so the document carries the authorization layer's own error and no `root_cause` of the
/// engine's.
fn forbidden() -> RtResponse {
    super::rendered(
        403,
        &::serde_json::json!({
            "error": { "type": FORBIDDEN, "reason": FORBIDDEN_REASON },
            "status": 403,
        }),
    )
}

/// Which resource of a collection a request addresses.
#[derive(Debug, PartialEq, Eq)]
enum Addressed {
    /// The collection itself.
    Collection,
    /// One named index of it.
    Index(String),
    /// Every index of it, which is what a collection-wide read or write reaches.
    EveryIndex,
}

impl Addressed {
    /// The `Resource` pattern a data access policy names this by.
    fn resource(&self, collection: &str) -> String {
        match self {
            Self::Collection => data_access::collection_resource(collection),
            Self::Index(index) => data_access::index_resource(collection, index),
            Self::EveryIndex => data_access::index_resource(collection, "*"),
        }
    }
}

/// Who a request speaks for: every spelling of the principal a grant may name it by,
/// whether the data access policies bound it at all (see the module docs), and the signing
/// name the credential it presented was scoped to.
#[derive(Debug, Default)]
pub(super) struct Caller {
    /// The principal spellings, most specific first. Empty for an unbounded caller, and for
    /// a credential that speaks for nobody.
    principals: Vec<String>,
    /// Set for a caller the data access policies do not bound.
    unbounded: bool,
    /// The signing name the presented SigV4 credential is scoped to, `None` for a request
    /// that presents no SigV4 scope at all.
    scoped_for: Option<String>,
}

impl Caller {
    /// Whether this request's credential was scoped to a signing name that is NOT this
    /// endpoint's own — a credential for another service, which is no credential for this
    /// one (see the module docs). `false` for a request presenting no scope to compare.
    fn scoped_for_another_service(&self) -> bool {
        self.scoped_for
            .as_deref()
            .is_some_and(|name| name != crate::state::ARN_NAMESPACE)
    }
}

/// The `Authorization` header `authorization` is, as a header map, which is the shape the
/// emulator's own readers of that header's grammar take — so this surface states no part of
/// it.
fn authorization_headers(authorization: Option<&str>) -> Option<::http::HeaderMap> {
    let value = ::http::HeaderValue::from_str(authorization?).ok()?;
    let mut headers = ::http::HeaderMap::new();
    headers.insert(::http::header::AUTHORIZATION, value);
    Some(headers)
}

/// The signing name the SigV4 credential a request presents is scoped to — read from the
/// `Authorization` header, or from the credential a presigned request carries in its query
/// instead — and `None` for a request presenting no SigV4 scope at all.
///
/// Both are read by the emulator's own readers of the SigV4 scope, the same ones that route
/// a request by the service its credential names, so the signing name compared here is the
/// one the request would have been routed to.
fn scoped_for(authorization: Option<&str>, query: &str) -> Option<String> {
    if let Some(from_header) = authorization_headers(authorization)
        .as_ref()
        .and_then(::emulator_server::service_from_sigv4)
    {
        return Some(from_header);
    }
    let uri: ::http::Uri = format!("/?{query}").parse().ok()?;
    ::emulator_server::service_from_sigv4_query(&uri)
}

/// The access key id `authorization` presents, read by the emulator's own reading of a
/// SigV4 `Authorization` header so this surface states no part of that header's grammar.
fn access_key_id(authorization: Option<&str>) -> Option<String> {
    let headers = authorization_headers(authorization)?;
    Some(::engine::extract_akid(&headers))
}

/// Who the request signed with `authorization` speaks for, and which signing name its
/// credential is scoped to — resolved against the account's own identity records in the
/// SAME shared engine.
///
/// Call with NO guard of this service's store held: the STS and IAM stores are read here
/// and this service's is locked by its caller afterwards, so no two of them are ever held
/// at once.
pub(super) fn caller<Eng: Send + Sync + 'static>(
    engine: &Eng,
    authorization: Option<&str>,
    query: &str,
) -> Caller {
    Caller {
        scoped_for: scoped_for(authorization, query),
        ..identity(engine, authorization)
    }
}

/// Who the request signed with `authorization` speaks for, which is every part of a
/// [`Caller`] but the signing name its credential was scoped to ([`caller`]).
fn identity<Eng: Send + Sync + 'static>(engine: &Eng, authorization: Option<&str>) -> Caller {
    use ::service_com_amazonaws_sts_awssecuritytokenservicev20110615::state as sts_store;

    let unbounded = Caller {
        principals: Vec::new(),
        unbounded: true,
        scoped_for: None,
    };
    let Some(access_key_id) = access_key_id(authorization) else {
        // An unsigned request presents no credential to resolve.
        return unbounded;
    };
    let Some(iam) = sts_store::iam_state(engine) else {
        // No IAM store on this engine: no principal can hold a grant at all.
        return unbounded;
    };
    // A key this emulator's STS minted for a role session speaks for that session; one it
    // did not speaks for the IAM user that holds it, which is the identity STS itself
    // reports for it. Both are read through STS's own accessors, so this plane sees exactly
    // what `sts:GetCallerIdentity` would answer.
    if let Some(sts) = sts_store::service_state(engine) {
        let store = sts.lock();
        if let Some(session) = store.session(&access_key_id) {
            if session.is_expired(::std::time::SystemTime::now()) {
                // An expired session speaks for nobody: no grant names it.
                return Caller::default();
            }
            let mut principals = vec![session.identity.arn.clone()];
            if let Some(role_arn) = &session.role_arn {
                principals.push(role_arn.clone());
            }
            return Caller {
                principals,
                unbounded: false,
                scoped_for: None,
            };
        }
    }
    let identity = sts_store::long_term_identity(Some(&iam.lock()), &access_key_id);
    if identity.arn == sts_store::ambient_identity().arn {
        // The account's own published credential, which holds no IAM records.
        return unbounded;
    }
    Caller {
        principals: vec![identity.arn],
        unbounded: false,
        scoped_for: None,
    }
}

/// Why the collection `collection_name` answers `request` nothing, or `None` when its
/// endpoint is reachable, the credential is one for this endpoint and its own policies admit
/// what the request asks for.
///
/// Read with the store's guard held, before the request is served, so a policy just written
/// decides the very next request and one just deleted stops deciding immediately.
pub(super) fn refusal(
    store: &ServiceState,
    collection_name: &str,
    caller: &Caller,
    request: &super::Request<'_>,
) -> Option<RtResponse> {
    if !store.network_policy_admits(collection_name) {
        return Some(super::status_only(NETWORK_REFUSAL_STATUS));
    }
    // A credential scoped to another service is no credential for this endpoint, whichever
    // principal it belongs to and whatever route it asks for — so this is read before the
    // grants are, and for the callers the data access policies do not bound as well.
    if caller.scoped_for_another_service() {
        return Some(forbidden());
    }
    if caller.unbounded {
        return None;
    }
    let (addressed, permission) = needed(request)?;
    let resource = addressed.resource(collection_name);
    if store.data_access_grants(&caller.principals, &resource, permission) {
        return None;
    }
    Some(forbidden())
}

/// The collection-wide grant check establishes that some index pattern is visible.
/// Check the concrete resource again before including any index metadata.
pub(super) fn can_describe_index(
    store: &ServiceState,
    collection: &str,
    caller: &Caller,
    index: &str,
) -> bool {
    caller.unbounded
        || store.data_access_grants(
            &caller.principals,
            &data_access::index_resource(collection, index),
            data_access::permission::DESCRIBE_INDEX,
        )
}

/// The resource `request` addresses and the permission it takes, or `None` for a route this
/// surface serves nothing for.
///
/// One arm per route the surface dispatches ([`super::serve_resolved`]), so a route that is
/// served is a route that is authorized.
fn needed(request: &super::Request<'_>) -> Option<(Addressed, &'static str)> {
    /// Whether `segment` names an index rather than one of the engine's own reserved
    /// endpoints, as the dispatch reads it.
    fn an_index(segment: &str) -> bool {
        !segment.starts_with('_')
    }

    /// The indexes a collection-wide or wildcarded read addresses.
    fn indexes(segment: &str) -> Addressed {
        if an_index(segment) {
            Addressed::Index(segment.to_string())
        } else {
            Addressed::EveryIndex
        }
    }

    let segments = request.segments();
    let reading = matches!(request.method, "GET" | "HEAD");
    match segments.as_slice() {
        [] => Some((
            Addressed::Collection,
            data_access::permission::DESCRIBE_COLLECTION_ITEMS,
        )),
        ["_cat", "indices"] => Some((
            Addressed::EveryIndex,
            data_access::permission::DESCRIBE_INDEX,
        )),
        ["_bulk"] => Some((
            Addressed::EveryIndex,
            data_access::permission::WRITE_DOCUMENT,
        )),
        ["_refresh"] => Some((
            Addressed::EveryIndex,
            data_access::permission::WRITE_DOCUMENT,
        )),
        ["_search"] | ["_count"] => Some((
            Addressed::EveryIndex,
            data_access::permission::READ_DOCUMENT,
        )),
        [index] if an_index(index) => {
            let permission = match request.method {
                "PUT" | "POST" => data_access::permission::CREATE_INDEX,
                "DELETE" => data_access::permission::DELETE_INDEX,
                _ => data_access::permission::DESCRIBE_INDEX,
            };
            Some((Addressed::Index((*index).to_string()), permission))
        }
        [index, "_doc"] if an_index(index) => Some((
            Addressed::Index((*index).to_string()),
            data_access::permission::WRITE_DOCUMENT,
        )),
        [index, "_doc", _id] | [index, "_create", _id] if an_index(index) => {
            let permission = if reading {
                data_access::permission::READ_DOCUMENT
            } else {
                data_access::permission::WRITE_DOCUMENT
            };
            Some((Addressed::Index((*index).to_string()), permission))
        }
        [index, "_mapping" | "_mappings" | "_settings"] if an_index(index) => {
            let permission = if reading {
                data_access::permission::DESCRIBE_INDEX
            } else {
                data_access::permission::UPDATE_INDEX
            };
            Some((Addressed::Index((*index).to_string()), permission))
        }
        [index, "_search" | "_count"] => Some((
            indexes(index),
            data_access::permission::READ_DOCUMENT,
        )),
        [index, "_refresh"] => Some((
            indexes(index),
            data_access::permission::WRITE_DOCUMENT,
        )),
        [index, "_bulk"] if an_index(index) => Some((
            Addressed::Index((*index).to_string()),
            data_access::permission::WRITE_DOCUMENT,
        )),
        _ => None,
    }
}
