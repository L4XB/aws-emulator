// SPDX-License-Identifier: CC0-1.0

//! The event an EBS volume's CREATE OUTCOME is published as, end to end over the wire.
//!
//! THE RULE: a volume create is not finished when the API answers it — the answer reports a
//! `creating` volume — so Amazon EBS publishes the OUTCOME of that create, once per volume
//! it was asked to create, onto the account's default EventBridge bus as `source` `aws.ec2`
//! / `detail-type` `EBS Volume Notification`, naming the volume in the envelope's
//! `resources` and carrying in the event `detail` the call it is the outcome of
//! (`createVolume`), the `result` that call reached, the `cause` of a result that failed and
//! the id of the call itself.
//! A volume that will come into being reports the state it reaches (`available`) with no
//! cause, and one that cannot be created at all reports `failed` with the reason the
//! service that refused it gave — so anything that reacts to a volume's fate learns of a
//! failure instead of waiting for a volume that will never appear.
//!
//! The rows span what the rule generalises over: the OUTCOME (a volume that is created, and
//! one that is not) and, for a failure, WHY (a key that is disabled, one scheduled for
//! deletion, one no key of the account carries) — the outcome following what the store
//! settled on rather than the shape of the request.
//!
//! Nothing here reads this crate's own state: AWS KMS, EventBridge and Amazon SQS are
//! mounted BESIDE Amazon EC2 on one engine, exactly as the assembled router mounts them,
//! every key is minted and moved through KMS's own API, and every event is read off the
//! queue a real rule on the real default bus delivered it to.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use service_com_amazonaws_ec2_amazonec2 as ec2;
use service_com_amazonaws_eventbridge_awsevents as eventbridge;
use service_com_amazonaws_kms_trentservice as kms;
use service_com_amazonaws_sqs_amazonsqs as sqs;
use smithy_server_runtime::MultiServiceRouter;

use super::ec2_query::{call, element};

/// The Region every call here is signed for.
const REGION: &str = "us-east-1";

/// The zone every volume here is created in.
const ZONE: &str = "us-east-1a";

/// The `source`, `detail-type` and `detail.event` the outcome of a volume create is
/// published under, spelled out here rather than imported, so a wrong constant in the
/// implementation cannot make a test pass.
const EC2_EVENT_SOURCE: &str = "aws.ec2";
const VOLUME_NOTIFICATION: &str = "EBS Volume Notification";
const CREATE_VOLUME_EVENT: &str = "createVolume";

/// The `result` each outcome carries.
const AVAILABLE: &str = "available";
const FAILED: &str = "failed";

/// How long a poll keeps asking for an event before giving up. Generous: the publication is
/// made once the outcome falls due, so this is many times the work a row does and still lets
/// a broken case fail rather than hang.
const DELIVERY_BUDGET: Duration = Duration::from_secs(60);

/// How long a poll waits between asks.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How many `ReceiveMessage` rounds one drain makes: a receive answers at most ten
/// messages, and this is well above the events the whole table publishes.
const MAX_RECEIVE_ROUNDS: usize = 8;

/// One request's form parameters, as the ec2Query harness takes them.
type Params = Vec<(String, String)>;

/// A `(key, value)` request parameter.
fn p(key: &str, value: impl Into<String>) -> (String, String) {
    (key.to_string(), value.into())
}

/// Amazon EC2, AWS KMS, EventBridge and Amazon SQS over ONE shared engine, exactly as the
/// assembled router mounts them: the key a volume names, the bus its outcome is published
/// onto and the queue a rule delivers that event to are each that service's own resource.
fn emulator() -> MultiServiceRouter {
    let engine: Arc<engine::InMemoryEngine> = Arc::new(engine::InMemoryEngine::new());
    let mut router = MultiServiceRouter::new();
    router.set_request_authentication(false);
    router.register_service(ec2::EmulatedService::new(Arc::clone(&engine)));
    router.register_service(kms::EmulatedService::new(Arc::clone(&engine)));
    router.register_service(eventbridge::EmulatedService::new(Arc::clone(&engine)));
    router.register_service(sqs::EmulatedService::new(engine));
    router
}

/// One `awsJson` RPC call against the service whose signing name is `signing_name`,
/// answering the status and the payload.
async fn json_call(
    router: &MultiServiceRouter,
    signing_name: &str,
    target_prefix: &str,
    json_version: &str,
    operation: &str,
    body: Value,
) -> (u16, Value) {
    let request = ::http::Request::builder()
        .method("POST")
        .uri(format!("https://{signing_name}.{REGION}.amazonaws.com/"))
        .header("host", format!("{signing_name}.{REGION}.amazonaws.com"))
        .header(
            "content-type",
            format!("application/x-amz-json-{json_version}"),
        )
        .header("x-amz-target", format!("{target_prefix}.{operation}"))
        .header(
            "authorization",
            format!(
                "AWS4-HMAC-SHA256 Credential=AKIDVOLUMEEVENTS/20260101/{REGION}/\
                 {signing_name}/aws4_request, SignedHeaders=host;x-amz-target, Signature=0"
            ),
        )
        .body(::bytes::Bytes::from(
            ::serde_json::to_vec(&body).expect("the request body serializes"),
        ))
        .expect("a well-formed JSON request");
    let response = router.dispatch(request).await;
    let status = response.status().as_u16();
    let payload: Value = if response.body().is_empty() {
        Value::Null
    } else {
        ::serde_json::from_slice(response.body()).unwrap_or(Value::Null)
    };
    (status, payload)
}

/// One AWS KMS call, which must be served.
async fn kms_ok(router: &MultiServiceRouter, case: &str, operation: &str, body: Value) -> Value {
    let (status, payload) =
        json_call(router, "kms", "TrentService", "1.1", operation, body).await;
    assert_eq!(
        status, 200,
        "case {case:?}: KMS {operation} must answer 200, got {status} with {payload}"
    );
    payload
}

/// One EventBridge call, which must be served.
async fn events_ok(router: &MultiServiceRouter, case: &str, operation: &str, body: Value) -> Value {
    let (status, payload) = json_call(router, "events", "AWSEvents", "1.1", operation, body).await;
    assert_eq!(
        status, 200,
        "case {case:?}: EventBridge {operation} must answer 200, got {status} with {payload}"
    );
    payload
}

/// One Amazon SQS call, which must be served.
async fn sqs_ok(router: &MultiServiceRouter, case: &str, operation: &str, body: Value) -> Value {
    let (status, payload) = json_call(router, "sqs", "AmazonSQS", "1.0", operation, body).await;
    assert_eq!(
        status, 200,
        "case {case:?}: Amazon SQS {operation} must answer 200, got {status} with {payload}"
    );
    payload
}

/// One customer managed key KMS minted, answering its key id and its ARN — the ARN being
/// how KMS names the key in what it says about it.
async fn key(router: &MultiServiceRouter, case: &str) -> (String, String) {
    let created = kms_ok(router, case, "CreateKey", json!({})).await;
    let metadata = &created["KeyMetadata"];
    let key_id = metadata["KeyId"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: CreateKey reports the key it minted, got {created}"))
        .to_string();
    let arn = metadata["Arn"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: CreateKey reports the key's ARN, got {created}"))
        .to_string();
    (key_id, arn)
}

/// A queue, created through Amazon SQS itself and carrying the grant a rule's target needs —
/// EventBridge makes the target's `sqs:SendMessage` as its own service principal and relies
/// on the queue's resource-based policy — answering its url and the ARN a rule names it by.
async fn queue(router: &MultiServiceRouter, case: &str, name: &str) -> (String, String) {
    let created = sqs_ok(router, case, "CreateQueue", json!({ "QueueName": name })).await;
    let url = created["QueueUrl"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: CreateQueue must answer a QueueUrl, got {created}"))
        .to_string();
    let attributes = sqs_ok(
        router,
        case,
        "GetQueueAttributes",
        json!({ "QueueUrl": &url, "AttributeNames": ["QueueArn"] }),
    )
    .await;
    let arn = attributes["Attributes"]["QueueArn"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("case {case:?}: GetQueueAttributes must answer a QueueArn, got {attributes}")
        })
        .to_string();
    let policy = json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": { "Service": "events.amazonaws.com" },
            "Action": "sqs:SendMessage",
            "Resource": &arn,
        }],
    })
    .to_string();
    sqs_ok(
        router,
        case,
        "SetQueueAttributes",
        json!({ "QueueUrl": &url, "Attributes": { "Policy": policy } }),
    )
    .await;
    (url, arn)
}

/// A default-bus rule matching the requested EC2 notification type, delivering to the queue
/// `queue_arn`, and the queue's url — the ONE wiring both tests below watch the events
/// through.
async fn watched_queue(router: &MultiServiceRouter, case: &str, name: &str, detail_type: &str) -> String {
    let (url, queue_arn) = queue(router, case, name).await;
    events_ok(
        router,
        case,
        "PutRule",
        json!({
            "Name": name,
            "State": "ENABLED",
            "EventPattern": json!({
                "source": [EC2_EVENT_SOURCE],
                "detail-type": [detail_type],
            })
            .to_string(),
        }),
    )
    .await;
    events_ok(
        router,
        case,
        "PutTargets",
        json!({ "Rule": name, "Targets": [{ "Id": "t1", "Arn": queue_arn }] }),
    )
    .await;
    url
}

/// Every event waiting on `url` right now, taken off the queue so a later drain reports only
/// what arrived after this one.
async fn drain(router: &MultiServiceRouter, case: &str, url: &str) -> Vec<Value> {
    let mut events = Vec::new();
    for _ in 0..MAX_RECEIVE_ROUNDS {
        let received = sqs_ok(
            router,
            case,
            "ReceiveMessage",
            json!({ "QueueUrl": url, "MaxNumberOfMessages": 10, "WaitTimeSeconds": 0 }),
        )
        .await;
        let messages = received["Messages"].as_array().cloned().unwrap_or_default();
        if messages.is_empty() {
            break;
        }
        for message in messages {
            if let Some(handle) = message["ReceiptHandle"].as_str() {
                sqs_ok(
                    router,
                    case,
                    "DeleteMessage",
                    json!({ "QueueUrl": url, "ReceiptHandle": handle }),
                )
                .await;
            }
            let body = message["Body"].as_str().unwrap_or_default();
            events.push(
                ::serde_json::from_str::<Value>(body)
                    .unwrap_or_else(|error| panic!("case {case:?}: a delivered event is JSON ({error}): {body}")),
            );
        }
    }
    events
}

/// The events delivered to `url` that name `volume_arn` in their `resources`, looked for
/// until the deadline passes rather than waited out, so the answer is the one the service
/// published however fast the host runs. Every other event drained on the way is answered
/// too, so nothing a row published is lost to another row's poll.
async fn delivered_for(
    router: &MultiServiceRouter,
    case: &str,
    url: &str,
    volume_arn: &str,
) -> (Vec<Value>, Vec<Value>) {
    let deadline = Instant::now() + DELIVERY_BUDGET;
    let mut mine: Vec<Value> = Vec::new();
    let mut others: Vec<Value> = Vec::new();
    loop {
        for event in drain(router, case, url).await {
            let names_volume = event["resources"]
                .as_array()
                .is_some_and(|resources| {
                    resources
                        .iter()
                        .any(|resource| resource.as_str() == Some(volume_arn))
                });
            if names_volume {
                mine.push(event);
            } else {
                others.push(event);
            }
        }
        if !mine.is_empty() {
            return (mine, others);
        }
        assert!(
            Instant::now() < deadline,
            "case {case:?}: an EBS volume notification for {volume_arn} must be delivered, \
             drained {others:?} instead"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// One volume created in [`ZONE`] with `extra` beside the members every row carries,
/// answering the volume id it was accepted under and the ARN the account knows it by.
async fn create_volume(router: &MultiServiceRouter, case: &str, extra: Params) -> (String, String) {
    let mut params = vec![
        p("AvailabilityZone", ZONE),
        p("VolumeType", "gp3"),
        p("Size", "4"),
    ];
    params.extend(extra);
    let (status, body) = call(router, REGION, "CreateVolume", &params).await;
    assert_eq!(
        status, 200,
        "case {case:?}: CreateVolume must answer 200, got {status} with {body}"
    );
    let volume_id = element(&body, "volumeId")
        .unwrap_or_else(|| panic!("case {case:?}: CreateVolume reports a volumeId, got {body}"));
    let volume_arn = element(&body, "volumeArn")
        .unwrap_or_else(|| panic!("case {case:?}: CreateVolume reports a volumeArn, got {body}"));
    (volume_id, volume_arn)
}

/// What a row requires of the event's `cause`.
#[derive(Debug)]
enum Cause {
    /// Exactly this sentence — a create that failed states WHY, in the words of the service
    /// that refused it, and one that did not fail carries the empty string.
    Exactly(String),
    /// Some sentence the refusing service stated, whatever its wording: the row is about a
    /// refusal this suite does not pin the text of.
    Stated,
}

/// One row of the outcome table.
struct Row {
    case: &'static str,
    /// The members this row adds to the create.
    extra: Params,
    /// The `result` the event must report.
    result: &'static str,
    /// The `cause` it must carry.
    cause: Cause,
}

/// THE RULE over the outcomes a volume create can reach.
#[tokio::test]
async fn every_volume_create_publishes_one_notification_of_its_outcome() {
    let router = emulator();
    let url = watched_queue(&router, "the volume notifications", "ebs-volume-events", VOLUME_NOTIFICATION).await;

    // Each key is minted by KMS itself and then moved into the state its row is about,
    // through KMS's own API.
    let (enabled, _) = key(&router, "an enabled key").await;
    let (disabled, disabled_arn) = key(&router, "a disabled key").await;
    kms_ok(
        &router,
        "a disabled key",
        "DisableKey",
        json!({ "KeyId": disabled.clone() }),
    )
    .await;
    let (pending_deletion, pending_deletion_arn) = key(&router, "a key pending deletion").await;
    kms_ok(
        &router,
        "a key pending deletion",
        "ScheduleKeyDeletion",
        json!({ "KeyId": pending_deletion.clone(), "PendingWindowInDays": 7 }),
    )
    .await;

    let rows = vec![
        Row {
            case: "a volume naming no customer managed key",
            extra: Vec::new(),
            result: AVAILABLE,
            cause: Cause::Exactly(String::new()),
        },
        Row {
            case: "an encrypted volume naming no key of its own",
            extra: vec![p("Encrypted", "true")],
            result: AVAILABLE,
            cause: Cause::Exactly(String::new()),
        },
        Row {
            case: "a volume under a key KMS will produce a data key under",
            extra: vec![p("Encrypted", "true"), p("KmsKeyId", enabled.clone())],
            result: AVAILABLE,
            cause: Cause::Exactly(String::new()),
        },
        Row {
            case: "a volume under a disabled key",
            extra: vec![p("Encrypted", "true"), p("KmsKeyId", disabled.clone())],
            result: FAILED,
            // The sentence KMS states about a key in that state, which is what names the
            // key and what about it refused the data key.
            cause: Cause::Exactly(format!("{disabled_arn} is disabled.")),
        },
        Row {
            case: "a volume under a key scheduled for deletion",
            extra: vec![
                p("Encrypted", "true"),
                p("KmsKeyId", pending_deletion.clone()),
            ],
            result: FAILED,
            cause: Cause::Exactly(format!("{pending_deletion_arn} is pending deletion.")),
        },
        Row {
            case: "a volume under a key identifier no key of the account carries",
            extra: vec![
                p("Encrypted", "true"),
                p("KmsKeyId", "11111111-2222-3333-4444-555555555555"),
            ],
            result: FAILED,
            cause: Cause::Stated,
        },
    ];

    // Every event delivered while another row was being waited for, so a row's own event is
    // never missed because an earlier poll drained it.
    let mut spare: Vec<Value> = Vec::new();
    // The id each row's own event carried: the ids tell two outcomes apart, so no two rows
    // may report the same one.
    let mut request_ids: Vec<String> = Vec::new();
    for row in &rows {
        let case = row.case;
        let (volume_id, volume_arn) = create_volume(&router, case, row.extra.clone()).await;
        let mut mine: Vec<Value> = spare
            .iter()
            .filter(|event| {
                event["resources"].as_array().is_some_and(|resources| {
                    resources
                        .iter()
                        .any(|resource| resource.as_str() == Some(volume_arn.as_str()))
                })
            })
            .cloned()
            .collect();
        if mine.is_empty() {
            let (delivered, others) = delivered_for(&router, case, &url, &volume_arn).await;
            mine = delivered;
            spare.extend(others);
        }

        assert_eq!(
            mine.len(),
            1,
            "case {case:?}: exactly one notification is published for {volume_id}, got {mine:?}"
        );
        let event = &mine[0];
        assert_eq!(
            event["source"].as_str(),
            Some(EC2_EVENT_SOURCE),
            "case {case:?}: expected the source {EC2_EVENT_SOURCE}, got {event}"
        );
        assert_eq!(
            event["detail-type"].as_str(),
            Some(VOLUME_NOTIFICATION),
            "case {case:?}: expected the detail-type {VOLUME_NOTIFICATION}, got {event}"
        );
        assert_eq!(
            event["resources"].as_array().map(Vec::len),
            Some(1),
            "case {case:?}: the event names the one volume it is about, got {event}"
        );
        let detail = &event["detail"];
        assert_eq!(
            detail["event"].as_str(),
            Some(CREATE_VOLUME_EVENT),
            "case {case:?}: expected the event {CREATE_VOLUME_EVENT}, got {event}"
        );
        assert_eq!(
            detail["result"].as_str(),
            Some(row.result),
            "case {case:?}: expected the result {:?}, got {event}",
            row.result
        );
        let cause = detail["cause"].as_str().unwrap_or_else(|| {
            panic!("case {case:?}: the detail carries a cause member, got {event}")
        });
        match &row.cause {
            Cause::Exactly(expected) => assert_eq!(
                cause, expected,
                "case {case:?}: expected the cause {expected:?}, got {cause:?}"
            ),
            Cause::Stated => assert!(
                !cause.is_empty(),
                "case {case:?}: a failed outcome states why, got {cause:?}"
            ),
        }

        // The event names the CALL it is the outcome of, in the form a request id takes —
        // asserted through the ONE reading of that form this tree has
        // (`service_common::query_error_envelope::is_request_id`) — and no two outcomes carry
        // the same id.
        let request_id = detail["request-id"].as_str().unwrap_or_else(|| {
            panic!("case {case:?}: the detail carries a request-id member, got {event}")
        });
        assert!(
            ::service_common::query_error_envelope::is_request_id(request_id),
            "case {case:?}: expected a request id of the 8-4-4-4-12 form, got {request_id:?}"
        );
        assert!(
            !request_ids.contains(&request_id.to_string()),
            "case {case:?}: {request_id:?} was already reported for another outcome, got \
             {request_ids:?}"
        );
        request_ids.push(request_id.to_string());
    }
}

/// THE RULE's other half: the event is the outcome of a CREATE, not of a call — so a
/// `ClientToken` replayed against the volume it already minted creates nothing and publishes
/// nothing a second time.
#[tokio::test]
async fn a_replayed_client_token_publishes_no_second_notification() {
    let case = "a replayed client token";
    let router = emulator();
    let url = watched_queue(&router, case, "ebs-volume-replays", VOLUME_NOTIFICATION).await;

    let token = vec![p("ClientToken", "one-and-the-same-create")];
    let (volume_id, volume_arn) = create_volume(&router, case, token.clone()).await;
    let (replayed_id, replayed_arn) = create_volume(&router, case, token).await;
    assert_eq!(
        replayed_id, volume_id,
        "case {case:?}: a replayed token names the volume it minted, got {replayed_id} \
         after {volume_id}"
    );
    assert_eq!(
        replayed_arn, volume_arn,
        "case {case:?}: that volume keeps its ARN, got {replayed_arn} after {volume_arn}"
    );

    let (mine, others) = delivered_for(&router, case, &url, &volume_arn).await;
    assert!(
        others.is_empty(),
        "case {case:?}: only this volume's own notification is published, got {others:?}"
    );
    assert_eq!(
        mine.len(),
        1,
        "case {case:?}: one volume was created, so one notification is published, got {mine:?}"
    );

    // Both calls have been answered, so anything the replay published is already on the
    // queue: a second drain reports it if there is one.
    let after = drain(&router, case, &url).await;
    assert!(
        after.is_empty(),
        "case {case:?}: the replay created nothing, so it published nothing, got {after:?}"
    );
}

/// A registration that minted no AMI must not publish an available state. One real
/// registration then witnesses that the same default-bus rule and queue are live.
#[tokio::test]
async fn only_a_successfully_registered_ami_publishes_its_available_state() {
    let case = "registered AMI";
    let router = emulator();
    let url = watched_queue(&router, case, "ami-registration", "EC2 AMI State Change").await;
    let (volume, _) = create_volume(&router, case, Vec::new()).await;
    let (status, body) = call(&router, REGION, "CreateSnapshot", &[p("VolumeId", volume)]).await;
    assert_eq!(status, 200, "{body}");
    let snapshot = element(&body, "snapshotId").expect("CreateSnapshot returned its ID");
    let mut params = vec![
        p("Name", "registered-image"),
        p("RootDeviceName", "/dev/sda1"),
        p("BlockDeviceMapping.1.DeviceName", "/dev/sda1"),
        p("BlockDeviceMapping.1.Ebs.SnapshotId", "snap-00000000000000000"),
    ];
    let (status, body) = call(&router, REGION, "RegisterImage", &params).await;
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("InvalidSnapshotID.NotFound"), "{body}");
    params[3] = p("BlockDeviceMapping.1.Ebs.SnapshotId", snapshot);
    let (status, body) = call(&router, REGION, "RegisterImage", &params).await;
    assert_eq!(status, 200, "{body}");
    let image = element(&body, "imageId").expect("RegisterImage returned its ID");
    let arn = format!("arn:aws:ec2:{REGION}::image/{image}");
    let (events, others) = delivered_for(&router, case, &url, &arn).await;
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(events.len(), 1, "{events:?}");
    let event = &events[0];
    assert_eq!(event["source"], "aws.ec2");
    assert_eq!(event["detail-type"], "EC2 AMI State Change");
    assert_eq!(event["account"], engine::emulator_account_id());
    assert_eq!(event["region"], REGION);
    assert_eq!(event["resources"], json!([arn]));
    assert_eq!(event["detail"]["ImageId"], image);
    assert_eq!(event["detail"]["State"], "available");
    assert_eq!(event["detail"]["ErrorMessage"], "");
    assert!(event["detail"]["RequestId"].as_str().is_some_and(|id| !id.is_empty()));
    // Give any stray best-effort publication its own independent delivery window.
    for _ in 0..40 {
        tokio::time::sleep(POLL_INTERVAL).await;
        assert!(drain(&router, case, &url).await.is_empty());
    }
}
