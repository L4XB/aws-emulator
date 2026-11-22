// SPDX-License-Identifier: CC0-1.0

//! End-to-end tests for a bucket's LIFECYCLE CONFIGURATION:
//! `PutBucketLifecycleConfiguration` records it, `GetBucketLifecycleConfiguration` reports
//! it back, `DeleteBucketLifecycle` removes it, and the object operations report what the
//! recorded configuration does to the objects and the uploads it applies to.
//!
//! Every assertion drives a real HTTP request through the router (the harness is the one
//! [`super::bucket_lifecycle`] owns), so what is pinned is the wire behaviour — status,
//! bound headers, payload, error code — rather than any handler's internals.
//!
//! The rules under test, with their sources:
//!
//! * A put "creates a new lifecycle configuration for the bucket or replaces an existing
//!   lifecycle configuration", so a read answers exactly the rules the last put carried,
//!   and a delete "removes all the lifecycle configuration rules in the lifecycle
//!   subresource associated with the bucket", after which the read answers the
//!   `NoSuchLifecycleConfiguration` that operation's documentation publishes (HTTP 404).
//! * `LifecycleRule$ID` is the rule's "unique identifier", so a rule that arrives without
//!   one is given one: distinct from every other rule's and stable across reads.
//! * A configuration outside the published rules is refused — with `MalformedXML` when the
//!   document does not validate against the published schema, `InvalidArgument` when one
//!   member's value is outside what that member accepts, and `InvalidRequest` when the
//!   members cannot hold together — and leaves the bucket's current configuration as it
//!   was.
//! * "If the object expiration is configured … the response includes this header. It
//!   includes the expiry-date and rule-id key-value pairs providing object expiration
//!   information" (`GetObjectOutput$Expiration`): the matching rule in effect, the earliest
//!   one when several match, dated by adding the rule's day count to the object's creation
//!   and rounding up to the next midnight UTC — and nothing at all when no rule in effect
//!   matches the object.
//! * "If the bucket has a lifecycle rule configured with an action to abort incomplete
//!   multipart uploads and the prefix in the lifecycle rule matches the object name in the
//!   request, the response includes this header"
//!   (`CreateMultipartUploadOutput$AbortDate`), with "the x-amz-abort-rule-id header that
//!   provides the ID of the lifecycle configuration rule that defines this action".

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

use service_com_amazonaws_s3_amazons3::ops::types::{
    STORAGE_CLASS_VALUES, TRANSITION_STORAGE_CLASS_VALUES,
};
use service_com_amazonaws_s3_amazons3::state::{
    tag_key_bounds_message, DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
    LIFECYCLE_DOCUMENT_REQUIRED_ELEMENTS, LIFECYCLE_DOCUMENT_SCHEMA, MALFORMED_XML_MESSAGE,
    MAX_LIFECYCLE_RULES, MAX_LIFECYCLE_RULE_ID_CHARS, MAX_NEWER_NONCURRENT_VERSIONS,
    MAX_TAG_KEY_CHARS, NO_SUCH_LIFECYCLE_CONFIGURATION_CODE,
    NO_SUCH_LIFECYCLE_CONFIGURATION_MESSAGE,
};
use smithy_server_runtime::MultiServiceRouter;

use super::bucket_lifecycle::{router, send, Answer};
use super::object_and_tagging_roundtrip::{bucket, get_object, put_object, text};
use super::object_tagging::put_object_tagging;
use super::object_versioning::head_object;

/// Seconds in one day, the unit a lifecycle day count is measured in.
const SECONDS_PER_DAY: u64 = 86_400;

/// `PUT /<bucket>?lifecycle` — `PutBucketLifecycleConfiguration`, with the configuration as
/// the payload and whatever bound headers the case carries.
pub(crate) async fn put_lifecycle(
    router: &MultiServiceRouter,
    bucket: &str,
    headers: &[(&str, &str)],
    configuration: &str,
) -> Answer {
    send(
        router,
        "PUT",
        &format!("/{bucket}?lifecycle"),
        headers,
        configuration,
    )
    .await
}

/// `GET /<bucket>?lifecycle` — `GetBucketLifecycleConfiguration`.
async fn get_lifecycle(router: &MultiServiceRouter, bucket: &str) -> Answer {
    send(router, "GET", &format!("/{bucket}?lifecycle"), &[], "").await
}

/// `DELETE /<bucket>?lifecycle` — `DeleteBucketLifecycle`.
async fn delete_lifecycle(router: &MultiServiceRouter, bucket: &str) -> Answer {
    send(router, "DELETE", &format!("/{bucket}?lifecycle"), &[], "").await
}

/// `POST /<bucket>/<key>?uploads` — `CreateMultipartUpload`.
async fn initiate_upload(router: &MultiServiceRouter, bucket: &str, key: &str) -> Answer {
    send(router, "POST", &format!("/{bucket}/{key}?uploads"), &[], "").await
}

/// `GET /<bucket>/<key>?uploadId=<id>&x-id=ListParts` — `ListParts`. The `x-id` literal is
/// the one the model binds that operation under, which is what distinguishes it from a read
/// of the object at the same address.
async fn list_parts(
    router: &MultiServiceRouter,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> Answer {
    send(
        router,
        "GET",
        &format!("/{bucket}/{key}?uploadId={upload_id}&x-id=ListParts"),
        &[],
        "",
    )
    .await
}

/// The `rules` wrapped in the `LifecycleConfiguration` document a put carries.
pub(crate) fn configuration(rules: &[String]) -> String {
    format!(
        "<LifecycleConfiguration xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">{}\
         </LifecycleConfiguration>",
        rules.join("")
    )
}

/// One `Rule` element from its inner elements.
pub(crate) fn rule(body: &str) -> String {
    format!("<Rule>{body}</Rule>")
}

/// The instant `days` days after `at`, rounded up to the next midnight UTC, as the HTTP
/// date an `x-amz-expiration` / `x-amz-abort-date` carries.
///
/// The rule the emulator is being held to, computed here independently of it.
fn midnight_after_days(at: SystemTime, days: u64) -> String {
    let seconds = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("the test clock is after the epoch")
        .as_secs()
        + days * SECONDS_PER_DAY;
    let midnight = seconds.div_ceil(SECONDS_PER_DAY) * SECONDS_PER_DAY;
    ::aws_smithy_types::DateTime::from(SystemTime::UNIX_EPOCH + Duration::from_secs(midnight))
        .fmt(::aws_smithy_types::date_time::Format::HttpDate)
        .expect("a midnight renders as an HTTP date")
}

/// The `expiry-date="…", rule-id="…"` an object expiring `days` days after `at` under the
/// rule `rule_id` must report — for the two midnights an object written at `at` can be
/// dated from, since the write and this assertion can straddle one.
fn expiration_headers(at: SystemTime, after: SystemTime, days: u64, rule_id: &str) -> Vec<String> {
    let mut candidates: Vec<String> = [at, after]
        .into_iter()
        .map(|instant| {
            format!(
                "expiry-date=\"{}\", rule-id=\"{rule_id}\"",
                midnight_after_days(instant, days)
            )
        })
        .collect();
    candidates.dedup();
    candidates
}

/// One row of the round-trip table: the case, the rules the put carries, the
/// `x-amz-transition-default-minimum-object-size` header it carries (when any), and the
/// fragments the read-back document must carry.
struct RoundTripRow {
    case: &'static str,
    rules: Vec<String>,
    minimum_object_size: Option<&'static str>,
    expected_minimum_object_size: &'static str,
    expected_fragments: Vec<&'static str>,
}

/// The round-trip table.
///
/// The rows differ along what the store-and-report rule generalises over: which form
/// identifies the rule's objects (the legacy `Prefix`, a filter's own `Prefix`, a `Tag`, an
/// object-size bound, an `And` of several, and an empty filter that "applies to all objects
/// in the bucket"), which actions it carries (an expiration by day count, one by date, a
/// chain of transitions, the noncurrent-version pair, and an abort of incomplete multipart
/// uploads), whether the rule is in effect, how many rules the configuration carries, and
/// whether the put names a minimum-object-size behaviour of its own. Every storage class
/// below is one of `TransitionStorageClass`'s enum values
/// (`smithy_query(query="shape",shape="TransitionStorageClass")`).
fn round_trip_rows() -> Vec<RoundTripRow> {
    vec![
        RoundTripRow {
            case: "a legacy Prefix rule expiring by day count",
            rules: vec![rule(
                "<ID>legacy-prefix</ID><Prefix>logs/</Prefix><Status>Enabled</Status>\
                 <Expiration><Days>30</Days></Expiration>",
            )],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>legacy-prefix</ID>",
                "<Prefix>logs/</Prefix>",
                "<Status>Enabled</Status>",
                "<Days>30</Days>",
            ],
        },
        RoundTripRow {
            case: "a filter Prefix rule, with the put naming a minimum object size",
            rules: vec![rule(
                "<ID>filter-prefix</ID><Filter><Prefix>reports/</Prefix></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>",
            )],
            minimum_object_size: Some("varies_by_storage_class"),
            expected_minimum_object_size: "varies_by_storage_class",
            expected_fragments: vec![
                "<ID>filter-prefix</ID>",
                "<Prefix>reports/</Prefix>",
                "<Days>1</Days>",
            ],
        },
        RoundTripRow {
            case: "a tag-filtered rule",
            rules: vec![rule(
                "<ID>tagged</ID><Filter><Tag><Key>stage</Key><Value>raw</Value></Tag></Filter>\
                 <Status>Enabled</Status><Expiration><Days>7</Days></Expiration>",
            )],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>tagged</ID>",
                "<Key>stage</Key>",
                "<Value>raw</Value>",
                "<Days>7</Days>",
            ],
        },
        RoundTripRow {
            case: "an And of a prefix, two tags and both size bounds, with a transition chain",
            rules: vec![rule(
                "<ID>combined</ID><Filter><And><Prefix>mixed/</Prefix>\
                 <Tag><Key>team</Key><Value>data</Value></Tag>\
                 <Tag><Key>tier</Key><Value>cold</Value></Tag>\
                 <ObjectSizeGreaterThan>1024</ObjectSizeGreaterThan>\
                 <ObjectSizeLessThan>1048576</ObjectSizeLessThan></And></Filter>\
                 <Status>Enabled</Status>\
                 <Transition><Days>30</Days><StorageClass>STANDARD_IA</StorageClass></Transition>\
                 <Transition><Days>60</Days><StorageClass>GLACIER</StorageClass></Transition>\
                 <Expiration><Days>365</Days></Expiration>",
            )],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>combined</ID>",
                "<ObjectSizeGreaterThan>1024</ObjectSizeGreaterThan>",
                "<ObjectSizeLessThan>1048576</ObjectSizeLessThan>",
                "<StorageClass>STANDARD_IA</StorageClass>",
                "<StorageClass>GLACIER</StorageClass>",
                "<Days>365</Days>",
            ],
        },
        RoundTripRow {
            case: "an empty filter aborting incomplete multipart uploads",
            rules: vec![rule(
                "<ID>abort-everything</ID><Filter></Filter><Status>Enabled</Status>\
                 <AbortIncompleteMultipartUpload><DaysAfterInitiation>3\
                 </DaysAfterInitiation></AbortIncompleteMultipartUpload>",
            )],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>abort-everything</ID>",
                "<DaysAfterInitiation>3</DaysAfterInitiation>",
            ],
        },
        RoundTripRow {
            case: "a size-filtered rule with the noncurrent-version pair",
            rules: vec![rule(
                "<ID>noncurrent</ID>\
                 <Filter><ObjectSizeGreaterThan>128</ObjectSizeGreaterThan></Filter>\
                 <Status>Enabled</Status>\
                 <NoncurrentVersionTransition><NoncurrentDays>15</NoncurrentDays>\
                 <StorageClass>GLACIER_IR</StorageClass></NoncurrentVersionTransition>\
                 <NoncurrentVersionExpiration><NoncurrentDays>120</NoncurrentDays>\
                 <NewerNoncurrentVersions>5</NewerNoncurrentVersions>\
                 </NoncurrentVersionExpiration>",
            )],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>noncurrent</ID>",
                "<NoncurrentDays>15</NoncurrentDays>",
                "<StorageClass>GLACIER_IR</StorageClass>",
                "<NewerNoncurrentVersions>5</NewerNoncurrentVersions>",
            ],
        },
        RoundTripRow {
            case: "two rules, one date-based and one disabled",
            rules: vec![
                rule(
                    "<ID>by-date</ID><Filter><Prefix>dated/</Prefix></Filter>\
                     <Status>Enabled</Status>\
                     <Expiration><Date>2030-01-01T00:00:00Z</Date></Expiration>",
                ),
                rule(
                    "<ID>switched-off</ID><Filter><Prefix>paused/</Prefix></Filter>\
                     <Status>Disabled</Status><Expiration><Days>2</Days></Expiration>",
                ),
            ],
            minimum_object_size: None,
            expected_minimum_object_size: DEFAULT_TRANSITION_MINIMUM_OBJECT_SIZE,
            expected_fragments: vec![
                "<ID>by-date</ID>",
                "<ID>switched-off</ID>",
                "<Status>Disabled</Status>",
            ],
        },
    ]
}

#[tokio::test]
async fn a_configuration_reads_back_as_it_was_put_and_then_deletes() {
    let router = router();

    for (index, row) in round_trip_rows().into_iter().enumerate() {
        let case = row.case;
        let name = format!("lifecycle-round-trip-{index}");
        bucket(&router, &name).await;

        // A bucket that has never been configured reports no configuration at all.
        let absent = get_lifecycle(&router, &name).await;
        assert_eq!(
            absent.status, 404,
            "case {case:?}: a bucket with no lifecycle configuration must answer 404, got \
             {} with body {:?}",
            absent.status,
            text(&absent)
        );
        assert_eq!(
            absent.error_code(),
            NO_SUCH_LIFECYCLE_CONFIGURATION_CODE,
            "case {case:?}: expected the published absent-configuration code, got {:?}",
            absent.error_code()
        );
        assert_eq!(
            absent.error_message(),
            NO_SUCH_LIFECYCLE_CONFIGURATION_MESSAGE,
            "case {case:?}: expected the published absent-configuration message, got {:?}",
            absent.error_message()
        );

        let headers: Vec<(&str, &str)> = row
            .minimum_object_size
            .map(|value| vec![("x-amz-transition-default-minimum-object-size", value)])
            .unwrap_or_default();
        let put = put_lifecycle(&router, &name, &headers, &configuration(&row.rules)).await;
        assert_eq!(
            put.status, 200,
            "case {case:?}: expected HTTP 200 from the put, got {} with body {:?}",
            put.status,
            text(&put)
        );
        assert_eq!(
            put.header("x-amz-transition-default-minimum-object-size")
                .as_deref(),
            Some(row.expected_minimum_object_size),
            "case {case:?}: expected the put to report minimum object size {:?}, got {:?}",
            row.expected_minimum_object_size,
            put.header("x-amz-transition-default-minimum-object-size")
        );

        let read = get_lifecycle(&router, &name).await;
        assert_eq!(
            read.status, 200,
            "case {case:?}: expected HTTP 200 from the read-back, got {} with body {:?}",
            read.status,
            text(&read)
        );
        assert_eq!(
            read.header("x-amz-transition-default-minimum-object-size")
                .as_deref(),
            Some(row.expected_minimum_object_size),
            "case {case:?}: expected the read to report minimum object size {:?}, got {:?}",
            row.expected_minimum_object_size,
            read.header("x-amz-transition-default-minimum-object-size")
        );
        let document = text(&read);
        for fragment in &row.expected_fragments {
            assert!(
                document.contains(fragment),
                "case {case:?}: the read-back must carry {fragment:?}, got {document:?}"
            );
        }

        // A delete removes the whole subresource, and is answered by the read.
        let deleted = delete_lifecycle(&router, &name).await;
        assert_eq!(
            deleted.status, 204,
            "case {case:?}: expected HTTP 204 from the delete, got {} with body {:?}",
            deleted.status,
            text(&deleted)
        );
        let gone = get_lifecycle(&router, &name).await;
        assert_eq!(
            gone.status, 404,
            "case {case:?}: expected 404 after the delete, got {} with body {:?}",
            gone.status,
            text(&gone)
        );
        assert_eq!(
            gone.error_code(),
            NO_SUCH_LIFECYCLE_CONFIGURATION_CODE,
            "case {case:?}: expected the published absent-configuration code after the \
             delete, got {:?}",
            gone.error_code()
        );

        // And the delete is idempotent: the bucket is already in the asked-for state.
        let again = delete_lifecycle(&router, &name).await;
        assert_eq!(
            again.status, 204,
            "case {case:?}: a second delete must still answer 204, got {} with body {:?}",
            again.status,
            text(&again)
        );
    }
}

#[tokio::test]
async fn a_rule_that_omits_its_id_is_given_a_distinct_and_stable_one() {
    let router = router();

    // Three rules in one configuration, two of them naming no `ID` — and a second bucket
    // whose rules name none either, so what is pinned is that the identifiers are distinct
    // across the whole store and not merely within one document.
    let rules = vec![
        rule("<Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status><Expiration><Days>1</Days></Expiration>"),
        rule("<ID>named-by-the-request</ID><Filter><Prefix>b/</Prefix></Filter><Status>Enabled</Status><Expiration><Days>2</Days></Expiration>"),
        rule("<Filter><Prefix>c/</Prefix></Filter><Status>Disabled</Status><Expiration><Days>3</Days></Expiration>"),
    ];

    let mut per_bucket: Vec<Vec<String>> = Vec::new();
    for name in ["minted-ids-first", "minted-ids-second"] {
        bucket(&router, name).await;
        let put = put_lifecycle(&router, name, &[], &configuration(&rules)).await;
        assert_eq!(
            put.status, 200,
            "case {name:?}: the configuration must be recorded, got {} with body {:?}",
            put.status,
            text(&put)
        );

        let first = ids_of(&get_lifecycle(&router, name).await);
        let second = ids_of(&get_lifecycle(&router, name).await);
        assert_eq!(
            first.len(),
            rules.len(),
            "case {name:?}: every rule must read back with an ID, got {first:?}"
        );
        assert!(
            first.iter().all(|id| !id.is_empty()),
            "case {name:?}: no rule may read back with an empty ID, got {first:?}"
        );
        assert_eq!(
            first, second,
            "case {name:?}: two reads of an unchanged configuration must report the same \
             identifiers, got {first:?} then {second:?}"
        );
        assert!(
            first.contains(&"named-by-the-request".to_string()),
            "case {name:?}: the identifier the request named must survive, got {first:?}"
        );
        let mut distinct = first.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            first.len(),
            "case {name:?}: the identifiers must be distinct, got {first:?}"
        );
        per_bucket.push(first);
    }

    // The minted identifiers of the two buckets do not collide: only the one the requests
    // named is shared.
    let shared: Vec<&String> = per_bucket[0]
        .iter()
        .filter(|id| per_bucket[1].contains(id))
        .collect();
    assert_eq!(
        shared,
        vec![&"named-by-the-request".to_string()],
        "the only identifier two configurations may share is the one both requests named, \
         got {shared:?}"
    );
}

/// The `ID` of every rule of a read-back configuration, in document order.
fn ids_of(answer: &Answer) -> Vec<String> {
    let document = String::from_utf8_lossy(&answer.body).to_string();
    document
        .split("<ID>")
        .skip(1)
        .filter_map(|rest| rest.split_once("</ID>"))
        .map(|(id, _)| id.to_string())
        .collect()
}

/// One row of the refusal table: the case, the document the put carries, and the wire code
/// and status it must be refused with.
struct RefusalRow {
    case: &'static str,
    document: String,
    code: &'static str,
    status: u16,
    /// The exact `<Message>` the refusal must carry, where the rule that produces it fixes
    /// the text; `None` where only the code and the status are the contract.
    message: Option<String>,
}

/// The refusal table.
///
/// The rows differ along which published rule the configuration breaks: the rule count, the
/// two forms that identify a rule's objects and the one form a configuration may use, how
/// many predicates a filter names, the bounds of an object-size range, whether a rule
/// carries an action at all, the three alternatives an expiration names, the day counts the
/// four dated actions accept, the time of day a date carries, the direction and the spacing
/// of a transition chain, and the identifier's length and uniqueness.
fn refusal_rows() -> Vec<RefusalRow> {
    let one_thousand_and_one: Vec<String> = (0..=MAX_LIFECYCLE_RULES)
        .map(|index| {
            rule(&format!(
                "<ID>rule-{index}</ID><Filter><Prefix>p{index}/</Prefix></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>"
            ))
        })
        .collect();

    vec![
        RefusalRow {
            case: "one rule past the published ceiling",
            document: configuration(&one_thousand_and_one),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "a rule naming both Prefix and Filter",
            document: configuration(&[rule(
                "<ID>both</ID><Prefix>a/</Prefix><Filter><Prefix>a/</Prefix></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "a rule naming neither Prefix nor Filter",
            document: configuration(&[rule(
                "<ID>neither</ID><Status>Enabled</Status>\
                 <Expiration><Days>1</Days></Expiration>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "a filter naming two predicates directly",
            document: configuration(&[rule(
                "<ID>two-predicates</ID><Filter><Prefix>a/</Prefix>\
                 <ObjectSizeGreaterThan>1</ObjectSizeGreaterThan></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "an expiration naming both a day count and a date",
            document: configuration(&[rule(
                "<ID>days-and-date</ID><Filter><Prefix>a/</Prefix></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days>\
                 <Date>2030-01-01T00:00:00Z</Date></Expiration>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "a transition naming no storage class",
            document: configuration(&[rule(
                "<ID>nowhere</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Transition><Days>40</Days></Transition>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "a rule naming no action",
            document: configuration(&[rule(
                "<ID>no-action</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>",
            )]),
            code: "InvalidRequest",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "a legacy Prefix rule beside a Filter rule",
            document: configuration(&[
                rule(
                    "<ID>legacy</ID><Prefix>a/</Prefix><Status>Enabled</Status>\
                     <Expiration><Days>1</Days></Expiration>",
                ),
                rule(
                    "<ID>modern</ID><Filter><Prefix>b/</Prefix></Filter><Status>Enabled</Status>\
                     <Expiration><Days>1</Days></Expiration>",
                ),
            ]),
            code: "InvalidRequest",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "an object-size range no object size is inside",
            document: configuration(&[rule(
                "<ID>empty-range</ID><Filter><And><Prefix>a/</Prefix>\
                 <ObjectSizeGreaterThan>1024</ObjectSizeGreaterThan>\
                 <ObjectSizeLessThan>1024</ObjectSizeLessThan></And></Filter>\
                 <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>",
            )]),
            code: "InvalidRequest",
            status: 400,
            message: Some(
                "'ObjectSizeLessThan' has to be a value greater than 'ObjectSizeGreaterThan'."
                    .to_string(),
            ),
        },
        RefusalRow {
            case: "an expiration day count of zero",
            document: configuration(&[rule(
                "<ID>zero-days</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Expiration><Days>0</Days></Expiration>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: Some("'Days' for Expiration action must be a positive integer".to_string()),
        },
        RefusalRow {
            case: "an abort action with a day count of zero",
            document: configuration(&[rule(
                "<ID>zero-abort</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <AbortIncompleteMultipartUpload><DaysAfterInitiation>0\
                 </DaysAfterInitiation></AbortIncompleteMultipartUpload>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: Some(
                "'DaysAfterInitiation' for AbortIncompleteMultipartUpload action must be a \
                 positive integer"
                    .to_string(),
            ),
        },
        RefusalRow {
            case: "a noncurrent expiration day count of zero",
            document: configuration(&[rule(
                "<ID>zero-noncurrent</ID><Filter><Prefix>a/</Prefix></Filter>\
                 <Status>Enabled</Status><NoncurrentVersionExpiration><NoncurrentDays>0\
                 </NoncurrentDays></NoncurrentVersionExpiration>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: Some(
                "'NoncurrentDays' for NoncurrentVersionExpiration action must be a positive \
                 integer"
                    .to_string(),
            ),
        },
        RefusalRow {
            case: "an expiration date that is not midnight UTC",
            document: configuration(&[rule(
                "<ID>noon</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Expiration><Date>2030-01-01T12:00:00Z</Date></Expiration>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
        RefusalRow {
            // `TransitionStorageClass` names the six classes a rule may move an object INTO
            // (`smithy_query(query="shape",shape="TransitionStorageClass")`), and STANDARD is
            // not among them: an object is already in it. There is NO minimum-days floor
            // beside it — a transition to an infrequent-access class one day after creation
            // is accepted (see [`a_short_transition_round_trips`]), because the minimum
            // storage duration is what the object is charged for, not a bound on the
            // configuration ("the value can be 0 or any positive integer … some storage
            // classes have a minimum storage duration and … you're charged for transitioning
            // objects before their minimum storage duration", `Transition$Days`).
            case: "a transition whose target storage class is STANDARD",
            document: configuration(&[rule(
                "<ID>not-a-target</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Transition><Days>30</Days><StorageClass>STANDARD</StorageClass>\
                 </Transition>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: Some("Invalid target Storage class for Transition action".to_string()),
        },
        RefusalRow {
            case: "a transition chain running from a colder class to a warmer one",
            document: configuration(&[rule(
                "<ID>backwards</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Transition><Days>30</Days><StorageClass>GLACIER</StorageClass></Transition>\
                 <Transition><Days>60</Days><StorageClass>STANDARD_IA</StorageClass>\
                 </Transition>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "a transition on from Glacier inside its minimum storage duration",
            document: configuration(&[rule(
                "<ID>too-close</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Transition><Days>30</Days><StorageClass>GLACIER</StorageClass></Transition>\
                 <Transition><Days>60</Days><StorageClass>DEEP_ARCHIVE</StorageClass>\
                 </Transition>",
            )]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "two transitions of one rule naming the same storage class",
            document: configuration(&[rule(
                "<ID>twice</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Transition><Days>30</Days><StorageClass>GLACIER</StorageClass></Transition>\
                 <Transition><Days>200</Days><StorageClass>GLACIER</StorageClass></Transition>",
            )]),
            code: "InvalidRequest",
            status: 400,
            message: None,
        },
        RefusalRow {
            // A rule measures every action from one clock or the other, and a document mixing
            // them is one the published schema does not admit: the fault is the SHAPE of the
            // document, so it carries the fixed schema sentence rather than a clause about
            // the two actions. Pinned over the pairs of actions that can mix them by
            // [`a_rule_measuring_its_actions_from_both_clocks_is_refused_as_malformed`].
            case: "one rule mixing a date-based and a day-based action",
            document: configuration(&[rule(
                "<ID>mixed-clocks</ID><Filter><Prefix>a/</Prefix></Filter>\
                 <Status>Enabled</Status>\
                 <Transition><Date>2030-01-01T00:00:00Z</Date>\
                 <StorageClass>GLACIER</StorageClass></Transition>\
                 <Expiration><Days>400</Days></Expiration>",
            )]),
            code: "MalformedXML",
            status: 400,
            message: Some(MALFORMED_XML_MESSAGE.to_string()),
        },
        RefusalRow {
            case: "two rules carrying the same identifier",
            document: configuration(&[
                rule(
                    "<ID>same</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                     <Expiration><Days>1</Days></Expiration>",
                ),
                rule(
                    "<ID>same</ID><Filter><Prefix>b/</Prefix></Filter><Status>Enabled</Status>\
                     <Expiration><Days>1</Days></Expiration>",
                ),
            ]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "an identifier one character past the published length",
            document: configuration(&[rule(&format!(
                "<ID>{}</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
                 <Expiration><Days>1</Days></Expiration>",
                "i".repeat(MAX_LIFECYCLE_RULE_ID_CHARS + 1)
            ))]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
        RefusalRow {
            case: "one noncurrent version past the published retention count",
            document: configuration(&[rule(&format!(
                "<ID>too-many-versions</ID><Filter><Prefix>a/</Prefix></Filter>\
                 <Status>Enabled</Status><NoncurrentVersionExpiration>\
                 <NoncurrentDays>10</NoncurrentDays><NewerNoncurrentVersions>{}\
                 </NewerNoncurrentVersions></NoncurrentVersionExpiration>",
                MAX_NEWER_NONCURRENT_VERSIONS + 1
            ))]),
            code: "InvalidArgument",
            status: 400,
            message: None,
        },
    ]
}

#[tokio::test]
async fn a_configuration_outside_the_published_rules_is_refused_and_changes_nothing() {
    let router = router();
    bucket(&router, "refusals").await;

    // The configuration every refused put must leave untouched: an identifier a refusal
    // could only report if it had been overwritten.
    let stored = configuration(&[rule(
        "<ID>the-stored-rule</ID><Filter><Prefix>keep/</Prefix></Filter><Status>Enabled</Status>\
         <Expiration><Days>42</Days></Expiration>",
    )]);
    let put = put_lifecycle(&router, "refusals", &[], &stored).await;
    assert_eq!(
        put.status, 200,
        "the configuration the refusals must not disturb has to be recorded first, got {} \
         with body {:?}",
        put.status,
        text(&put)
    );

    for row in refusal_rows() {
        let case = row.case;
        let refused = put_lifecycle(&router, "refusals", &[], &row.document).await;
        assert_eq!(
            refused.status, row.status,
            "case {case:?}: expected HTTP {}, got {} with body {:?}",
            row.status,
            refused.status,
            text(&refused)
        );
        assert_eq!(
            refused.error_code(),
            row.code,
            "case {case:?}: expected error code {:?}, got {:?}",
            row.code,
            refused.error_code()
        );
        if let Some(message) = &row.message {
            assert_eq!(
                &refused.error_message(),
                message,
                "case {case:?}: expected message {message:?}, got {:?}",
                refused.error_message()
            );
        }

        // Nothing was written: the bucket still carries the configuration above.
        let read = get_lifecycle(&router, "refusals").await;
        let document = text(&read);
        assert!(
            document.contains("<ID>the-stored-rule</ID>") && document.contains("<Days>42</Days>"),
            "case {case:?}: the refused put must leave the stored configuration untouched, \
             got {document:?}"
        );
    }
}

#[tokio::test]
async fn a_lifecycle_call_naming_a_bucket_this_emulator_does_not_hold_is_refused() {
    let router = router();
    bucket(&router, "held-for-lifecycle").await;

    let document = configuration(&[rule(
        "<ID>any</ID><Filter><Prefix>a/</Prefix></Filter><Status>Enabled</Status>\
         <Expiration><Days>1</Days></Expiration>",
    )]);

    // Every one of the three operations is bucket-scoped, so each answers the same 404 for
    // a name this emulator holds no bucket under — a different name, a prefix of the held
    // one, and one differing only in case.
    for name in ["absent-bucket", "held", "Held-For-Lifecycle"] {
        for (operation, answer) in [
            ("put", put_lifecycle(&router, name, &[], &document).await),
            ("get", get_lifecycle(&router, name).await),
            ("delete", delete_lifecycle(&router, name).await),
        ] {
            assert_eq!(
                answer.status, 404,
                "case {name:?}/{operation}: expected HTTP 404 for a bucket this emulator \
                 does not hold, got {} with body {:?}",
                answer.status,
                text(&answer)
            );
            assert_eq!(
                answer.error_code(),
                "NoSuchBucket",
                "case {name:?}/{operation}: expected the service's NoSuchBucket, got {:?}",
                answer.error_code()
            );
        }
    }

    // And the bucket that IS held takes the same configuration, so the rows above failed on
    // the bucket lookup rather than on the document.
    let put = put_lifecycle(&router, "held-for-lifecycle", &[], &document).await;
    assert_eq!(
        put.status, 200,
        "the held bucket must take the configuration, got {} with body {:?}",
        put.status,
        text(&put)
    );
}

/// One row of the expiration-header table: the case, the key the object is written at, how
/// many bytes it carries, the tag set it carries, and the rule that must expire it with the
/// day count that dates it — `expected_rule_id` being `None` where no rule in effect applies
/// to the object, in which case `expected_days` says nothing.
struct ExpirationRow {
    case: &'static str,
    key: &'static str,
    bytes: usize,
    tagging: Option<&'static str>,
    expected_rule_id: Option<&'static str>,
    expected_days: u64,
}

/// One row of the abort-header table: the case, the key the upload is initiated for, and the
/// rule that must abort it with the day count that dates it — read the two `expected_`
/// fields the way [`ExpirationRow`]'s are read.
struct AbortRow {
    case: &'static str,
    key: &'static str,
    expected_rule_id: Option<&'static str>,
    expected_days: u64,
}

#[tokio::test]
async fn the_expiration_reported_for_an_object_is_the_rule_that_expires_it_first() {
    let router = router();
    bucket(&router, "expiring").await;

    // Four rules: one on a prefix, one on that same prefix AND a tag (so a tagged object
    // under it matches both and the earlier of the two is the answer), one on a size bound,
    // and one that is switched off.
    let rules = vec![
        rule(
            "<ID>logs-30</ID><Filter><Prefix>logs/</Prefix></Filter><Status>Enabled</Status>\
             <Expiration><Days>30</Days></Expiration>",
        ),
        rule(
            "<ID>logs-tagged-1</ID><Filter><And><Prefix>logs/</Prefix>\
             <Tag><Key>archive</Key><Value>yes</Value></Tag></And></Filter>\
             <Status>Enabled</Status><Expiration><Days>1</Days></Expiration>",
        ),
        rule(
            "<ID>big-10</ID><Filter><ObjectSizeGreaterThan>100</ObjectSizeGreaterThan></Filter>\
             <Status>Enabled</Status><Expiration><Days>10</Days></Expiration>",
        ),
        rule(
            "<ID>tmp-disabled</ID><Filter><Prefix>tmp/</Prefix></Filter><Status>Disabled</Status>\
             <Expiration><Days>2</Days></Expiration>",
        ),
    ];
    let put = put_lifecycle(&router, "expiring", &[], &configuration(&rules)).await;
    assert_eq!(
        put.status, 200,
        "the configuration the rows are judged against must be recorded, got {} with body {:?}",
        put.status,
        text(&put)
    );

    let rows = [
        ExpirationRow {
            case: "a prefixed object matches the prefix rule",
            key: "logs/plain.txt",
            bytes: 8,
            tagging: None,
            expected_rule_id: Some("logs-30"),
            expected_days: 30,
        },
        ExpirationRow {
            case: "a tagged object under the same prefix takes the earlier rule",
            key: "logs/tagged.txt",
            bytes: 8,
            tagging: Some("archive=yes"),
            expected_rule_id: Some("logs-tagged-1"),
            expected_days: 1,
        },
        ExpirationRow {
            case: "a large object outside every prefix matches the size rule",
            key: "data/large.bin",
            bytes: 101,
            tagging: None,
            expected_rule_id: Some("big-10"),
            expected_days: 10,
        },
        ExpirationRow {
            case: "an object at the size bound is not above it, so no rule applies",
            key: "data/at-the-bound.bin",
            bytes: 100,
            tagging: None,
            expected_rule_id: None,
            expected_days: 0,
        },
        ExpirationRow {
            case: "a disabled rule expires nothing",
            key: "tmp/scratch.txt",
            bytes: 8,
            tagging: None,
            expected_rule_id: None,
            expected_days: 0,
        },
        ExpirationRow {
            case: "an object no rule selects carries no expiration",
            key: "other/plain.txt",
            bytes: 8,
            tagging: None,
            expected_rule_id: None,
            expected_days: 0,
        },
    ];

    for row in rows {
        let case = row.case;
        let headers: Vec<(&str, &str)> = row
            .tagging
            .map(|tagging| vec![("x-amz-tagging", tagging)])
            .unwrap_or_default();
        let before = SystemTime::now();
        let written = put_object(
            &router,
            "expiring",
            row.key,
            &vec![b'x'; row.bytes],
            &headers,
        )
        .await;
        assert_eq!(
            written.status, 200,
            "case {case:?}: the object must be written, got {} with body {:?}",
            written.status,
            text(&written)
        );
        let after = SystemTime::now();

        // The write, the HEAD and the GET all report the same one answer.
        let head = head_object(&router, "expiring", row.key, "").await;
        let read = get_object(&router, "expiring", row.key, "", &[]).await;
        for (reader, answer) in [
            ("PutObject", &written),
            ("HeadObject", &head),
            ("GetObject", &read),
        ] {
            match row.expected_rule_id {
                None => assert_eq!(
                    answer.header("x-amz-expiration"),
                    None,
                    "case {case:?}/{reader}: no rule in effect applies, so no expiration \
                     may be reported, got {:?}",
                    answer.header("x-amz-expiration")
                ),
                Some(rule_id) => {
                    let candidates =
                        expiration_headers(before, after, row.expected_days, rule_id);
                    let reported = answer.header("x-amz-expiration");
                    assert!(
                        reported
                            .as_ref()
                            .is_some_and(|value| candidates.contains(value)),
                        "case {case:?}/{reader}: expected one of {candidates:?}, got {reported:?}"
                    );
                }
            }
        }
    }

    // Retagging moves an object into the tag-filtered rule and back out of it: the answer
    // follows the object's CURRENT tag set rather than the one the write carried.
    let before = SystemTime::now();
    let written = put_object(&router, "expiring", "logs/retagged.txt", b"payload", &[]).await;
    assert_eq!(
        written.status, 200,
        "the object the retagging rows move must be written, got {} with body {:?}",
        written.status,
        text(&written)
    );
    let after = SystemTime::now();

    for (case, tags, days, rule_id) in [
        ("written with no tags", Vec::new(), 30, "logs-30"),
        (
            "tagged into the rule",
            vec![("archive".to_string(), "yes".to_string())],
            1,
            "logs-tagged-1",
        ),
        (
            "tagged with another value, back out of the rule",
            vec![("archive".to_string(), "no".to_string())],
            30,
            "logs-30",
        ),
        ("tagged with nothing at all", Vec::new(), 30, "logs-30"),
    ] {
        if !tags.is_empty() || case != "written with no tags" {
            let tagged = put_object_tagging(&router, "expiring", "logs/retagged.txt", &tags).await;
            assert_eq!(
                tagged.status, 200,
                "case {case:?}: the tag set must be recorded, got {} with body {:?}",
                tagged.status,
                text(&tagged)
            );
        }

        let head = head_object(&router, "expiring", "logs/retagged.txt", "").await;
        let candidates = expiration_headers(before, after, days, rule_id);
        let reported = head.header("x-amz-expiration");
        assert!(
            reported
                .as_ref()
                .is_some_and(|value| candidates.contains(value)),
            "case {case:?}: expected one of {candidates:?}, got {reported:?}"
        );
    }

    // And once the configuration is gone, nothing expires: the header follows the bucket's
    // CURRENT configuration.
    let deleted = delete_lifecycle(&router, "expiring").await;
    assert_eq!(
        deleted.status, 204,
        "the configuration must be removable, got {} with body {:?}",
        deleted.status,
        text(&deleted)
    );
    let head = head_object(&router, "expiring", "logs/plain.txt", "").await;
    assert_eq!(
        head.header("x-amz-expiration"),
        None,
        "with no configuration on the bucket nothing expires, got {:?}",
        head.header("x-amz-expiration")
    );
}

#[tokio::test]
async fn an_incomplete_upload_reports_the_rule_that_aborts_it() {
    let router = router();
    bucket(&router, "aborting").await;

    let rules = vec![
        rule(
            "<ID>abort-staging-3</ID><Filter><Prefix>staging/</Prefix></Filter>\
             <Status>Enabled</Status><AbortIncompleteMultipartUpload><DaysAfterInitiation>3\
             </DaysAfterInitiation></AbortIncompleteMultipartUpload>",
        ),
        rule(
            "<ID>abort-off</ID><Filter><Prefix>paused/</Prefix></Filter><Status>Disabled</Status>\
             <AbortIncompleteMultipartUpload><DaysAfterInitiation>1</DaysAfterInitiation>\
             </AbortIncompleteMultipartUpload>",
        ),
        rule(
            "<ID>expire-only</ID><Filter><Prefix>other/</Prefix></Filter><Status>Enabled</Status>\
             <Expiration><Days>5</Days></Expiration>",
        ),
    ];
    let put = put_lifecycle(&router, "aborting", &[], &configuration(&rules)).await;
    assert_eq!(
        put.status, 200,
        "the configuration the rows are judged against must be recorded, got {} with body {:?}",
        put.status,
        text(&put)
    );

    let rows = [
        AbortRow {
            case: "an upload under the aborting prefix",
            key: "staging/part-of-a-report",
            expected_rule_id: Some("abort-staging-3"),
            expected_days: 3,
        },
        AbortRow {
            case: "a deeper key under that same prefix",
            key: "staging/nested/deep/object",
            expected_rule_id: Some("abort-staging-3"),
            expected_days: 3,
        },
        AbortRow {
            case: "an upload under a prefix whose rule is switched off",
            key: "paused/object",
            expected_rule_id: None,
            expected_days: 0,
        },
        AbortRow {
            case: "an upload under a prefix whose rule only expires objects",
            key: "other/object",
            expected_rule_id: None,
            expected_days: 0,
        },
    ];

    for row in rows {
        let AbortRow {
            case,
            key,
            expected_rule_id,
            expected_days,
        } = row;
        let before = SystemTime::now();
        let initiated = initiate_upload(&router, "aborting", key).await;
        assert_eq!(
            initiated.status, 200,
            "case {case:?}: the upload must be initiated, got {} with body {:?}",
            initiated.status,
            text(&initiated)
        );
        let after = SystemTime::now();
        let upload_id = initiated
            .element("UploadId")
            .expect("an initiated upload reports its id");

        let listed = list_parts(&router, "aborting", key, &upload_id).await;
        assert_eq!(
            listed.status, 200,
            "case {case:?}: the upload must read back, got {} with body {:?}",
            listed.status,
            text(&listed)
        );

        for (reader, answer) in [
            ("CreateMultipartUpload", &initiated),
            ("ListParts", &listed),
        ] {
            match expected_rule_id {
                None => {
                    assert_eq!(
                        answer.header("x-amz-abort-rule-id"),
                        None,
                        "case {case:?}/{reader}: no rule in effect aborts this upload, so no \
                         rule id may be reported, got {:?}",
                        answer.header("x-amz-abort-rule-id")
                    );
                    assert_eq!(
                        answer.header("x-amz-abort-date"),
                        None,
                        "case {case:?}/{reader}: no rule in effect aborts this upload, so no \
                         abort date may be reported, got {:?}",
                        answer.header("x-amz-abort-date")
                    );
                }
                Some(rule_id) => {
                    assert_eq!(
                        answer.header("x-amz-abort-rule-id").as_deref(),
                        Some(rule_id),
                        "case {case:?}/{reader}: expected the aborting rule {rule_id:?}, got \
                         {:?}",
                        answer.header("x-amz-abort-rule-id")
                    );
                    let candidates = [
                        midnight_after_days(before, expected_days),
                        midnight_after_days(after, expected_days),
                    ];
                    let reported = answer.header("x-amz-abort-date");
                    assert!(
                        reported
                            .as_ref()
                            .is_some_and(|value| candidates.contains(value)),
                        "case {case:?}/{reader}: expected one of {candidates:?}, got {reported:?}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// How a refusal of a lifecycle configuration is CLASSIFIED, and the form a read reports a
// stored instant in.
//
// * A document whose SHAPE the published schema does not admit — an element the schema does
//   not declare where it arrived, or a container missing a child it requires — is refused
//   `MalformedXML` with that code's own fixed sentence, whatever the document says; a document
//   the schema admits whose one member's VALUE is outside what that member accepts is refused
//   `InvalidArgument` naming that argument. The schema is the
//   `BucketLifecycleConfiguration` closure
//   (`smithy_query(query="closure",operation="PutBucketLifecycleConfiguration")`), which is
//   where `LIFECYCLE_DOCUMENT_SCHEMA` and `LIFECYCLE_DOCUMENT_REQUIRED_ELEMENTS` come from and
//   what the last test here re-derives them against.
// * A transition's target is a `TransitionStorageClass`
//   (`smithy_query(query="shape",shape="TransitionStorageClass")`), so a value that IS one of
//   this service's storage classes (`smithy_query(query="shape",shape="StorageClass")`) but
//   not one of those six is refused for the VALUE, while a value that names no storage class
//   at all is text the published schema does not admit for that element.
// * An `InvalidArgument` about one day count names it: S3's envelope carries `<ArgumentName>`
//   and `<ArgumentValue>` beside the code and the message, the message stating the rule the
//   count broke and the pair saying which member carried which value.
// * `LifecycleExpiration$Date` and `Transition$Date` are timestamps a read reports inside the
//   document it answers, so each is rendered in the fixed-width form every instant of an S3
//   document carries.
// ---------------------------------------------------------------------------------------

/// The code S3 answers a document that did not validate against the published schema.
const MALFORMED_XML: &str = "MalformedXML";

/// The code S3 answers one member's value that is outside what that member accepts.
const INVALID_ARGUMENT: &str = "InvalidArgument";

/// A tag predicate the published schema DOES declare, as a filter's own `Tag`.
const TAG_PREDICATE: &str = "<Tag><Key>team</Key><Value>data</Value></Tag>";

/// The expiration action the document rows below carry, so what each row varies is the SHAPE
/// of the document around it rather than the action.
const ONE_EXPIRATION: &str = "<Expiration><Days>30</Days></Expiration>";

/// Assert `answer` is the 400 refusal carrying `code` and `message`.
fn assert_refused(case: &str, answer: &Answer, code: &str, message: &str) {
    assert_eq!(
        answer.status,
        400,
        "case {case:?}: expected HTTP 400, got {} with body {:?}",
        answer.status,
        text(answer)
    );
    assert_eq!(
        answer.error_code(),
        code,
        "case {case:?}: expected the code {code:?}, got {:?}",
        answer.error_code()
    );
    assert_eq!(
        answer.error_message(),
        message,
        "case {case:?}: expected the message {message:?}, got {:?}",
        answer.error_message()
    );
}

/// The documents the published schema does not admit, each as (the case, the whole document).
///
/// The rows differ along the KIND of shape fault — an element the schema declares nowhere, a
/// declared element in a position the schema does not declare it in, a container missing a
/// child it requires, and a root that is not the published container — and along WHERE in the
/// document it sits, the tag-filter containers included, since the rule is about the
/// document's shape rather than about one element.
fn malformed_documents() -> Vec<(&'static str, String)> {
    vec![
        (
            "a tag predicate inside a container the schema declares nowhere",
            configuration(&[rule(&format!(
                "<ID>wrapped-tags</ID><Status>Enabled</Status>\
                 <Filter><Tags>{TAG_PREDICATE}</Tags></Filter>{ONE_EXPIRATION}"
            ))]),
        ),
        (
            "a tag predicate where the schema declares no element of that name",
            configuration(&[rule(&format!(
                "<ID>tag-beside-the-filter</ID><Status>Enabled</Status>\
                 {TAG_PREDICATE}{ONE_EXPIRATION}"
            ))]),
        ),
        (
            "a tag predicate naming no Value, a child Tag requires",
            configuration(&[rule(&format!(
                "<ID>valueless-tag</ID><Status>Enabled</Status>\
                 <Filter><Tag><Key>team</Key></Tag></Filter>{ONE_EXPIRATION}"
            ))]),
        ),
        (
            "a tag predicate inside an And whose key element is misspelled",
            configuration(&[rule(&format!(
                "<ID>misspelled-key</ID><Status>Enabled</Status>\
                 <Filter><And><Prefix>logs/</Prefix>\
                 <Tag><Name>team</Name><Value>data</Value></Tag></And></Filter>\
                 {ONE_EXPIRATION}"
            ))]),
        ),
        (
            "an object-size bound nested inside the tag predicate",
            configuration(&[rule(&format!(
                "<ID>misnested-size</ID><Status>Enabled</Status>\
                 <Filter><Tag><Key>team</Key><Value>data</Value>\
                 <ObjectSizeGreaterThan>1</ObjectSizeGreaterThan></Tag></Filter>\
                 {ONE_EXPIRATION}"
            ))]),
        ),
        (
            "a rule naming no Status, the child a rule requires",
            configuration(&[rule(&format!(
                "<ID>statusless</ID><Filter>{TAG_PREDICATE}</Filter>{ONE_EXPIRATION}"
            ))]),
        ),
        (
            "an expiration whose day element is misspelled",
            configuration(&[rule(
                "<ID>misspelled-days</ID><Status>Enabled</Status>\
                 <Filter><Prefix>logs/</Prefix></Filter>\
                 <Expiration><Day>30</Day></Expiration>",
            )]),
        ),
        (
            "a rule element one level too deep",
            format!(
                "<LifecycleConfiguration xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
                 <Rules>{}</Rules></LifecycleConfiguration>",
                rule(&format!(
                    "<ID>nested-rule</ID><Status>Enabled</Status>\
                     <Filter>{TAG_PREDICATE}</Filter>{ONE_EXPIRATION}"
                ))
            ),
        ),
        (
            "a root element that is not the published container",
            format!(
                "<Lifecycle>{}</Lifecycle>",
                rule(&format!(
                    "<ID>wrong-root</ID><Status>Enabled</Status>\
                     <Filter>{TAG_PREDICATE}</Filter>{ONE_EXPIRATION}"
                ))
            ),
        ),
    ]
}

/// The documents the published schema DOES admit, each as (the case, the document, a fragment
/// a later read must report back).
///
/// The rows differ along the filter's form — the filter's own tag predicate, several
/// predicates inside an `And`, a filter left empty, and the legacy `Prefix` form — so what the
/// table above refuses is the document's shape rather than a tag predicate being there at all.
fn admitted_documents() -> Vec<(&'static str, String, &'static str)> {
    vec![
        (
            "a filter naming one tag predicate",
            configuration(&[rule(&format!(
                "<ID>one-tag</ID><Status>Enabled</Status>\
                 <Filter>{TAG_PREDICATE}</Filter>{ONE_EXPIRATION}"
            ))]),
            "<ID>one-tag</ID>",
        ),
        (
            "an And of a prefix and two tag predicates",
            configuration(&[rule(&format!(
                "<ID>and-tags</ID><Status>Enabled</Status><Filter><And>\
                 <Prefix>logs/</Prefix>{TAG_PREDICATE}\
                 <Tag><Key>tier</Key><Value>cold</Value></Tag></And></Filter>{ONE_EXPIRATION}"
            ))]),
            "<ID>and-tags</ID>",
        ),
        (
            "a filter left empty, which applies to every object",
            configuration(&[rule(&format!(
                "<ID>every-object</ID><Status>Enabled</Status><Filter></Filter>{ONE_EXPIRATION}"
            ))]),
            "<ID>every-object</ID>",
        ),
        (
            "the legacy Prefix form, with no filter at all",
            configuration(&[rule(&format!(
                "<ID>legacy-prefix</ID><Status>Enabled</Status>\
                 <Prefix>logs/</Prefix>{ONE_EXPIRATION}"
            ))]),
            "<ID>legacy-prefix</ID>",
        ),
    ]
}

#[tokio::test]
async fn a_lifecycle_document_the_published_schema_does_not_admit_is_refused_as_malformed() {
    let router = router();
    let name = "lifecycle-document-shape";
    bucket(&router, name).await;

    for (case, document) in malformed_documents() {
        let refused = put_lifecycle(&router, name, &[], &document).await;
        assert_refused(case, &refused, MALFORMED_XML, MALFORMED_XML_MESSAGE);

        // Nothing was stored, so the read answers the code the operation publishes for a
        // bucket carrying no configuration — and names the bucket it was addressed to.
        let read = get_lifecycle(&router, name).await;
        assert_eq!(
            read.status,
            404,
            "case {case:?}: a refused put must leave the bucket with no configuration, got \
             {} with body {:?}",
            read.status,
            text(&read)
        );
        assert_eq!(
            read.error_code(),
            NO_SUCH_LIFECYCLE_CONFIGURATION_CODE,
            "case {case:?}: expected the code {NO_SUCH_LIFECYCLE_CONFIGURATION_CODE:?}, got {:?}",
            read.error_code()
        );
        assert_eq!(
            read.error_message(),
            NO_SUCH_LIFECYCLE_CONFIGURATION_MESSAGE,
            "case {case:?}: expected the message \
             {NO_SUCH_LIFECYCLE_CONFIGURATION_MESSAGE:?}, got {:?}",
            read.error_message()
        );
        assert_eq!(
            read.element("BucketName").as_deref(),
            Some(name),
            "case {case:?}: the refusal must name the bucket addressed, got {:?} in {:?}",
            read.element("BucketName"),
            text(&read)
        );
    }
}

#[tokio::test]
async fn a_lifecycle_document_the_published_schema_admits_is_stored() {
    let router = router();
    let name = "lifecycle-document-admitted";
    bucket(&router, name).await;

    for (case, document, reads_back) in admitted_documents() {
        let stored = put_lifecycle(&router, name, &[], &document).await;
        assert_eq!(
            stored.status,
            200,
            "case {case:?}: expected the configuration to be stored, got {} with body {:?}",
            stored.status,
            text(&stored)
        );
        let read = get_lifecycle(&router, name).await;
        let reported = text(&read);
        assert!(
            reported.contains(reads_back),
            "case {case:?}: the stored configuration must report {reads_back:?}, got \
             {reported:?}"
        );
    }
}

/// One row of the tag-key table: the case and the rule body.
///
/// The rows differ along HOW the key misses the published bounds — the element spelled empty,
/// the element left out, a predicate that names neither half, and a key one character past the
/// published maximum — and along WHERE the predicate sits (the filter's own `Tag` and the one
/// inside its `And`), since the bounds belong to the tag rather than to its container.
fn tag_key_rows() -> Vec<(&'static str, String)> {
    let over_long = "k".repeat(MAX_TAG_KEY_CHARS + 1);
    vec![
        (
            "a tag predicate whose key element is spelled empty",
            format!(
                "<ID>empty-key</ID><Status>Enabled</Status>\
                 <Filter><Tag><Key></Key><Value>data</Value></Tag></Filter>{ONE_EXPIRATION}"
            ),
        ),
        (
            "a tag predicate naming no key element at all",
            format!(
                "<ID>keyless-tag</ID><Status>Enabled</Status>\
                 <Filter><Tag><Value>data</Value></Tag></Filter>{ONE_EXPIRATION}"
            ),
        ),
        (
            "a self-closing tag predicate, which names neither half of the pair",
            format!(
                "<ID>empty-tag</ID><Status>Enabled</Status>\
                 <Filter><Tag/></Filter>{ONE_EXPIRATION}"
            ),
        ),
        (
            "an empty key inside an And",
            format!(
                "<ID>empty-key-and</ID><Status>Enabled</Status><Filter><And>\
                 <Prefix>logs/</Prefix><Tag><Key></Key><Value>data</Value></Tag></And>\
                 </Filter>{ONE_EXPIRATION}"
            ),
        ),
        (
            "a key one character past the published maximum",
            format!(
                "<ID>over-long-key</ID><Status>Enabled</Status>\
                 <Filter><Tag><Key>{over_long}</Key><Value>data</Value></Tag></Filter>\
                 {ONE_EXPIRATION}"
            ),
        ),
    ]
}

#[tokio::test]
async fn a_tag_predicate_key_outside_the_published_bounds_states_those_bounds() {
    let router = router();
    let name = "lifecycle-tag-key-bounds";
    bucket(&router, name).await;

    for (case, body) in tag_key_rows() {
        let document = configuration(&[rule(&body)]);
        let refused = put_lifecycle(&router, name, &[], &document).await;
        assert_refused(
            case,
            &refused,
            "InvalidRequest",
            &tag_key_bounds_message(),
        );
    }

    // The shortest and the longest key the bounds admit are both stored, so the rows above
    // are refused for missing those bounds rather than for naming a tag at all.
    for (case, key) in [
        ("the shortest key the bounds admit", "k".to_string()),
        (
            "the longest key the bounds admit",
            "k".repeat(MAX_TAG_KEY_CHARS),
        ),
    ] {
        let document = configuration(&[rule(&format!(
            "<ID>bounded-key-{}</ID><Status>Enabled</Status>\
             <Filter><Tag><Key>{key}</Key><Value>data</Value></Tag></Filter>{ONE_EXPIRATION}",
            key.chars().count()
        ))]);
        let stored = put_lifecycle(&router, name, &[], &document).await;
        assert_eq!(
            stored.status,
            200,
            "case {case:?}: expected the configuration to be stored, got {} with body {:?}",
            stored.status,
            text(&stored)
        );
    }
}

/// The two transition actions, each as (the action's element, the element carrying its day
/// count, the day count a row sends) — the pair S3 names a bad target class by.
const TRANSITION_ACTIONS: [(&str, &str, &str); 2] = [
    ("Transition", "Days", "30"),
    ("NoncurrentVersionTransition", "NoncurrentDays", "30"),
];

/// A rule whose only action is `action`, moving the object to `storage_class`.
fn transition_rule(action: (&str, &str, &str), storage_class: &str) -> String {
    let (action, days_member, days) = action;
    rule(&format!(
        "<ID>target-{storage_class}</ID><Status>Enabled</Status>\
         <Filter><Prefix>tiered/</Prefix></Filter>\
         <{action}><{days_member}>{days}</{days_member}>\
         <StorageClass>{storage_class}</StorageClass></{action}>"
    ))
}

#[tokio::test]
async fn a_transition_target_outside_the_published_set_is_classified_by_what_it_names() {
    let router = router();
    let name = "lifecycle-transition-target";
    bucket(&router, name).await;

    // A value that IS one of this service's storage classes but not a transition target: the
    // VALUE is what the caller has to change, so the refusal names the action it belongs to.
    // Every such value of the model's own `StorageClass` set is a row, not just the one an
    // object sits in by default.
    let not_a_target = STORAGE_CLASS_VALUES
        .iter()
        .copied()
        .filter(|value| !TRANSITION_STORAGE_CLASS_VALUES.contains(value));
    for storage_class in not_a_target {
        for action in TRANSITION_ACTIONS {
            let case = format!("{storage_class} as a {} target", action.0);
            let document = configuration(&[transition_rule(action, storage_class)]);
            let refused = put_lifecycle(&router, name, &[], &document).await;
            assert_refused(
                &case,
                &refused,
                INVALID_ARGUMENT,
                &format!("Invalid target Storage class for {} action", action.0),
            );
        }
    }

    // A value that names NO storage class of this service: the element's text is outside the
    // vocabulary the published schema declares for it, so the DOCUMENT is what did not
    // validate. The rows differ along how the text misses the set — a class spelled in
    // another case, a plausible name the service has no such class for, a published target
    // with its words run together, and no text at all.
    for storage_class in ["standard_ia", "ARCHIVE", "GLACIERIR", ""] {
        for action in TRANSITION_ACTIONS {
            let case = format!("{storage_class:?} as a {} target", action.0);
            let document = configuration(&[transition_rule(action, storage_class)]);
            let refused = put_lifecycle(&router, name, &[], &document).await;
            assert_refused(&case, &refused, MALFORMED_XML, MALFORMED_XML_MESSAGE);
        }
    }

    // And every published target IS stored, so the two tables above refuse what the set
    // excludes rather than the element itself.
    for storage_class in TRANSITION_STORAGE_CLASS_VALUES.iter().copied() {
        for action in TRANSITION_ACTIONS {
            let case = format!("{storage_class} as a {} target", action.0);
            let document = configuration(&[transition_rule(action, storage_class)]);
            let stored = put_lifecycle(&router, name, &[], &document).await;
            assert_eq!(
                stored.status,
                200,
                "case {case:?}: a published target must be stored, got {} with body {:?}",
                stored.status,
                text(&stored)
            );
        }
    }
}

/// One row of the day-count table: the case, the rule body, the member the refusal names, the
/// value as sent, and the sentence the refusal carries.
struct DayCountRow {
    case: &'static str,
    body: String,
    argument: &'static str,
    value: &'static str,
    message: String,
}

/// The sentence S3 refuses a day count that has to be a non-zero positive integer with.
fn must_be_positive(member: &str, action: &str) -> String {
    format!("'{member}' for {action} action must be a positive integer")
}

/// Every day count a lifecycle rule carries, each sent a value its member does not accept.
///
/// The rows differ along WHICH member carries the count (the two expirations, the two
/// transitions, the abort action and the retained-version count), the FLOOR that member
/// publishes — a transition's day count "can be `0` or any positive integer"
/// (`Transition$Days`), while every other count must be "a non-zero positive integer" — and
/// the value sent (zero, minus one and counts further below the floor).
fn day_count_rows() -> Vec<DayCountRow> {
    vec![
        DayCountRow {
            case: "a transition one day before the object exists",
            body: "<ID>negative-transition</ID><Status>Enabled</Status>\
                   <Filter><Prefix>tiered/</Prefix></Filter>\
                   <Transition><Days>-1</Days><StorageClass>GLACIER</StorageClass></Transition>"
                .to_string(),
            argument: "Days",
            value: "-1",
            message: "'Days' in Transition action must be nonnegative".to_string(),
        },
        DayCountRow {
            case: "a transition a week before the object exists",
            body: "<ID>negative-transition-week</ID><Status>Enabled</Status>\
                   <Filter><Prefix>tiered/</Prefix></Filter>\
                   <Transition><Days>-7</Days><StorageClass>GLACIER</StorageClass></Transition>"
                .to_string(),
            argument: "Days",
            value: "-7",
            message: "'Days' in Transition action must be nonnegative".to_string(),
        },
        DayCountRow {
            case: "an expiration on the day the object is created",
            body: "<ID>zero-expiration</ID><Status>Enabled</Status>\
                   <Filter><Prefix>logs/</Prefix></Filter>\
                   <Expiration><Days>0</Days></Expiration>"
                .to_string(),
            argument: "Days",
            value: "0",
            message: must_be_positive("Days", "Expiration"),
        },
        DayCountRow {
            case: "an expiration before the object is created",
            body: "<ID>negative-expiration</ID><Status>Enabled</Status>\
                   <Filter><Prefix>logs/</Prefix></Filter>\
                   <Expiration><Days>-30</Days></Expiration>"
                .to_string(),
            argument: "Days",
            value: "-30",
            message: must_be_positive("Days", "Expiration"),
        },
        DayCountRow {
            case: "a noncurrent expiration on the day the version became noncurrent",
            body: "<ID>zero-noncurrent</ID><Status>Enabled</Status>\
                   <Filter><Prefix>logs/</Prefix></Filter>\
                   <NoncurrentVersionExpiration><NoncurrentDays>0</NoncurrentDays>\
                   </NoncurrentVersionExpiration>"
                .to_string(),
            argument: "NoncurrentDays",
            value: "0",
            message: must_be_positive("NoncurrentDays", "NoncurrentVersionExpiration"),
        },
        DayCountRow {
            case: "a noncurrent transition on the day the version became noncurrent",
            body: "<ID>zero-noncurrent-transition</ID><Status>Enabled</Status>\
                   <Filter><Prefix>tiered/</Prefix></Filter>\
                   <NoncurrentVersionTransition><NoncurrentDays>0</NoncurrentDays>\
                   <StorageClass>GLACIER</StorageClass></NoncurrentVersionTransition>"
                .to_string(),
            argument: "NoncurrentDays",
            value: "0",
            message: must_be_positive("NoncurrentDays", "NoncurrentVersionTransition"),
        },
        DayCountRow {
            case: "an abort action on the day the upload is initiated",
            body: "<ID>zero-abort</ID><Status>Enabled</Status>\
                   <Filter><Prefix>uploads/</Prefix></Filter>\
                   <AbortIncompleteMultipartUpload><DaysAfterInitiation>0\
                   </DaysAfterInitiation></AbortIncompleteMultipartUpload>"
                .to_string(),
            argument: "DaysAfterInitiation",
            value: "0",
            message: must_be_positive("DaysAfterInitiation", "AbortIncompleteMultipartUpload"),
        },
        DayCountRow {
            case: "a noncurrent expiration retaining no version at all",
            body: "<ID>zero-retained</ID><Status>Enabled</Status>\
                   <Filter><Prefix>logs/</Prefix></Filter>\
                   <NoncurrentVersionExpiration><NoncurrentDays>10</NoncurrentDays>\
                   <NewerNoncurrentVersions>0</NewerNoncurrentVersions>\
                   </NoncurrentVersionExpiration>"
                .to_string(),
            argument: "NewerNoncurrentVersions",
            value: "0",
            message: must_be_positive("NewerNoncurrentVersions", "NoncurrentVersionExpiration"),
        },
    ]
}

#[tokio::test]
async fn a_refused_day_count_is_named_with_the_value_the_request_sent() {
    let router = router();
    let name = "lifecycle-day-counts";
    bucket(&router, name).await;

    for row in day_count_rows() {
        let case = row.case;
        let document = configuration(&[rule(&row.body)]);
        let refused = put_lifecycle(&router, name, &[], &document).await;
        assert_refused(case, &refused, INVALID_ARGUMENT, &row.message);
        assert_eq!(
            refused.element("ArgumentName").as_deref(),
            Some(row.argument),
            "case {case:?}: the envelope must name the member it refused, got {:?} in {:?}",
            refused.element("ArgumentName"),
            text(&refused)
        );
        assert_eq!(
            refused.element("ArgumentValue").as_deref(),
            Some(row.value),
            "case {case:?}: the envelope must carry the value as sent, got {:?} in {:?}",
            refused.element("ArgumentValue"),
            text(&refused)
        );
    }
}

/// The rules that measure their actions from BOTH clocks, each as (the case, the rule body).
///
/// The rows differ along which pair of actions mixes the two scales, and on whether one single
/// action carries both: an object's expiration and its transitions are measured from the one
/// clock the rule is written against, so a document naming both is one the published schema
/// does not admit. The counts an action measures on a scale of ITS OWN — how long a version
/// has been noncurrent, how long an upload has been incomplete — are not in it.
fn mixed_clock_rules() -> Vec<(&'static str, String)> {
    vec![
        (
            "a dated transition beside a counted expiration",
            "<ID>date-transition-day-expiration</ID><Status>Enabled</Status>\
             <Filter><Prefix>tiered/</Prefix></Filter>\
             <Transition><Date>2030-01-01T00:00:00Z</Date>\
             <StorageClass>GLACIER</StorageClass></Transition>\
             <Expiration><Days>400</Days></Expiration>"
                .to_string(),
        ),
        (
            "a counted transition beside a dated expiration",
            "<ID>day-transition-date-expiration</ID><Status>Enabled</Status>\
             <Filter><Prefix>tiered/</Prefix></Filter>\
             <Transition><Days>30</Days><StorageClass>GLACIER</StorageClass></Transition>\
             <Expiration><Date>2030-01-01T00:00:00Z</Date></Expiration>"
                .to_string(),
        ),
        (
            "one transition naming both a date and a day count",
            "<ID>transition-both</ID><Status>Enabled</Status>\
             <Filter><Prefix>tiered/</Prefix></Filter>\
             <Transition><Days>30</Days><Date>2030-01-01T00:00:00Z</Date>\
             <StorageClass>GLACIER</StorageClass></Transition>"
                .to_string(),
        ),
        (
            "a dated transition beside a counted one",
            "<ID>date-and-day-transitions</ID><Status>Enabled</Status>\
             <Filter><Prefix>tiered/</Prefix></Filter>\
             <Transition><Date>2030-01-01T00:00:00Z</Date>\
             <StorageClass>GLACIER</StorageClass></Transition>\
             <Transition><Days>30</Days><StorageClass>STANDARD_IA</StorageClass></Transition>"
                .to_string(),
        ),
    ]
}

#[tokio::test]
async fn a_rule_measuring_its_actions_from_both_clocks_is_refused_as_malformed() {
    let router = router();
    let name = "lifecycle-mixed-clocks";
    bucket(&router, name).await;

    for (case, body) in mixed_clock_rules() {
        let document = configuration(&[rule(&body)]);
        let refused = put_lifecycle(&router, name, &[], &document).await;
        assert_refused(case, &refused, MALFORMED_XML, MALFORMED_XML_MESSAGE);
    }
}

/// One row of the date-rendering table: the case, the date as the request spells it, and the
/// date a read must report.
struct DateRow {
    case: &'static str,
    spelled: &'static str,
    reported: &'static str,
}

/// The spellings one midnight may arrive in, and the one form a read reports it in.
///
/// The rows differ along the FRACTION the request spells — none, three zero digits, and a
/// fraction of another width — and along the date itself, so what is pinned is the form the
/// document is written in rather than an echo of the request's text.
fn date_rows() -> Vec<DateRow> {
    vec![
        DateRow {
            case: "a midnight spelled with no fraction",
            spelled: "2030-01-01T00:00:00Z",
            reported: "2030-01-01T00:00:00.000Z",
        },
        DateRow {
            case: "the same midnight spelled with three zero digits",
            spelled: "2030-01-01T00:00:00.000Z",
            reported: "2030-01-01T00:00:00.000Z",
        },
        DateRow {
            case: "a midnight on a leap day",
            spelled: "2032-02-29T00:00:00Z",
            reported: "2032-02-29T00:00:00.000Z",
        },
        DateRow {
            case: "a midnight spelled with a one-digit fraction",
            spelled: "2041-12-31T00:00:00.0Z",
            reported: "2041-12-31T00:00:00.000Z",
        },
    ]
}

#[tokio::test]
async fn a_stored_date_reads_back_in_the_form_every_s3_document_carries() {
    let router = router();
    let name = "lifecycle-date-form";
    bucket(&router, name).await;

    for row in date_rows() {
        let case = row.case;
        // Both members the model reports a lifecycle instant in, in one configuration: the
        // expiration's own `Date` and a transition's, each measured from the same clock.
        let document = configuration(&[
            rule(&format!(
                "<ID>expires-on-{}</ID><Status>Enabled</Status>\
                 <Filter><Prefix>logs/</Prefix></Filter>\
                 <Expiration><Date>{}</Date></Expiration>",
                row.spelled, row.spelled
            )),
            rule(&format!(
                "<ID>transitions-on-{}</ID><Status>Enabled</Status>\
                 <Filter><Prefix>tiered/</Prefix></Filter>\
                 <Transition><Date>{}</Date><StorageClass>GLACIER</StorageClass></Transition>",
                row.spelled, row.spelled
            )),
        ]);
        let stored = put_lifecycle(&router, name, &[], &document).await;
        assert_eq!(
            stored.status,
            200,
            "case {case:?}: the configuration must be stored, got {} with body {:?}",
            stored.status,
            text(&stored)
        );

        let read = get_lifecycle(&router, name).await;
        assert_eq!(
            read.status,
            200,
            "case {case:?}: the stored configuration must read back, got {} with body {:?}",
            read.status,
            text(&read)
        );
        let dates = read.elements("Date");
        assert_eq!(
            dates,
            vec![row.reported.to_string(), row.reported.to_string()],
            "case {case:?}: both instants must be reported as {:?}, got {dates:?} in {:?}",
            row.reported,
            text(&read)
        );
    }
}

/// Both tables the lifecycle document schema is walked against, re-derived from the model
/// itself: every element one declares is a member of the `BucketLifecycleConfiguration`
/// closure under the name restXml spells it with, and every required child is a member that
/// closure marks `@required`.
///
/// Source: the model this crate ships, read here rather than restated — so a model update that
/// renames, adds or removes an element of the closure cannot leave the tables behind.
#[test]
fn the_lifecycle_document_schema_tables_match_the_model() {
    let model_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/smithy-model/com-amazonaws-s3-amazons3.json"
    );
    let model: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(model_path).expect("the service's Smithy model must be readable"),
    )
    .expect("the Smithy model must parse as JSON");
    let shapes = model["shapes"]
        .as_object()
        .expect("the model must carry a shapes object");

    /// The element a member is written under: its `@xmlName`, else its own name.
    fn element_of(name: &str, member: &serde_json::Value) -> String {
        member["traits"]["smithy.api#xmlName"]
            .as_str()
            .unwrap_or(name)
            .to_string()
    }

    /// The shape a member's element really carries: a LIST is flattened into the element its
    /// own member name gives it, so what arrives under that element is the list's member
    /// shape.
    fn carried_shape<'m>(
        shapes: &'m serde_json::Map<String, serde_json::Value>,
        target: &str,
    ) -> Option<&'m serde_json::Value> {
        let shape = shapes.get(target)?;
        match shape["type"].as_str() {
            Some("list") => shapes.get(shape["member"]["target"].as_str()?),
            _ => Some(shape),
        }
    }

    // The payload container, under the element name the operation's own member spells it with
    // (`PutBucketLifecycleConfigurationRequest$LifecycleConfiguration`, `@xmlName`).
    let payload = &shapes["com.amazonaws.s3#PutBucketLifecycleConfigurationRequest"]["members"]
        ["LifecycleConfiguration"];
    let root = element_of("LifecycleConfiguration", payload);
    let mut pending: Vec<(String, String)> = vec![(
        root.clone(),
        payload["target"]
            .as_str()
            .expect("the payload member names a shape")
            .to_string(),
    )];
    let mut derived_elements: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut derived_required: BTreeSet<(String, String)> = BTreeSet::new();
    while let Some((element, target)) = pending.pop() {
        if derived_elements.contains_key(&element) {
            continue;
        }
        let Some(members) = carried_shape(shapes, &target).and_then(|shape| {
            shape["members"]
                .as_object()
                .filter(|_| shape["type"] == "structure")
        }) else {
            continue;
        };
        let children = derived_elements.entry(element.clone()).or_default();
        for (name, member) in members {
            let child = element_of(name, member);
            children.insert(child.clone());
            if member["traits"].get("smithy.api#required").is_some() {
                derived_required.insert((element.clone(), child.clone()));
            }
            if let Some(target) = member["target"].as_str() {
                pending.push((child, target.to_string()));
            }
        }
    }
    // A LEAF carries text rather than elements, so the tables declare no row for it: the
    // derivation above adds a row only for a structure.
    derived_elements.retain(|_, children| !children.is_empty());

    assert_eq!(
        LIFECYCLE_DOCUMENT_SCHEMA[0].0, root,
        "the schema's first row is the payload container, got {:?}",
        LIFECYCLE_DOCUMENT_SCHEMA[0].0
    );
    let declared_elements: BTreeMap<String, BTreeSet<String>> = LIFECYCLE_DOCUMENT_SCHEMA
        .iter()
        .map(|(element, children)| {
            (
                (*element).to_string(),
                children.iter().map(|child| (*child).to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(
        declared_elements, derived_elements,
        "LIFECYCLE_DOCUMENT_SCHEMA must declare exactly the closure's own elements; declared \
         {declared_elements:?}, the model reports {derived_elements:?}"
    );

    let declared_required: BTreeSet<(String, String)> = LIFECYCLE_DOCUMENT_REQUIRED_ELEMENTS
        .iter()
        .map(|(container, child)| ((*container).to_string(), (*child).to_string()))
        .collect();
    assert_eq!(
        declared_required, derived_required,
        "LIFECYCLE_DOCUMENT_REQUIRED_ELEMENTS must name exactly the closure's required \
         members; declared {declared_required:?}, the model reports {derived_required:?}"
    );
}

#[tokio::test]
async fn current_expiration_leaves_one_marker_and_preserves_the_source_version() {
    let (router, engine) = super::bucket_lifecycle::router_with_engine();
    let name = "expiration-effect";
    service_com_amazonaws_s3_amazons3::state::service_state(&*engine).unwrap().lock()
        .set_experimental_current_expiration_bucket(Some(name.into()));
    bucket(&router, name).await;
    super::object_versioning::set_versioning(&router, name, "Enabled").await;
    let written = put_object(&router, name, "tmp/source", b"source bytes", &[]).await;
    let version = written.header("x-amz-version-id").unwrap();
    assert_eq!(put_object(&router, name, "keep", b"control", &[]).await.status, 200);
    let config = "<LifecycleConfiguration><Rule><ID>due</ID><Filter><Prefix>tmp/</Prefix></Filter><Status>Enabled</Status><Expiration><Date>2000-01-01T00:00:00Z</Date></Expiration></Rule></LifecycleConfiguration>";
    assert_eq!(put_lifecycle(&router, name, &[], config).await.status, 200);
    let scoped = send(&router, "GET", &format!("/{name}?versions&prefix=tmp/"), &[], "").await;
    assert_eq!(scoped.status, 200);
    assert!(!text(&scoped).contains("<DeleteMarker>"));
    let wrong_owner = send(&router, "GET", &format!("/{name}?versions"),
        &[("x-amz-expected-bucket-owner", "999999999999")], "").await;
    assert_eq!(wrong_owner.status, 403);
    assert_eq!(get_object(&router, name, "tmp/source", "", &[]).await.status, 200);
    for _ in 0..2 {
        let listed = send(&router, "GET", &format!("/{name}?versions"), &[], "").await;
        assert_eq!(listed.status, 200);
        assert_eq!(text(&listed).matches("<DeleteMarker>").count(), 1);
        assert_eq!(head_object(&router, name, "tmp/source", "").await.status, 404);
        let saved = get_object(&router, name, "tmp/source", &format!("?versionId={version}"), &[]).await;
        assert_eq!(saved.status, 200);
        assert_eq!(text(&saved), "source bytes");
        assert_eq!(text(&get_object(&router, name, "keep", "", &[]).await), "control");
    }
}
