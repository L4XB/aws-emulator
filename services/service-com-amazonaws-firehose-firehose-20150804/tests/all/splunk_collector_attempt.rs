// SPDX-License-Identifier: CC0-1.0

//! The attempt a Splunk destination's delivery makes against its HTTP Event Collector.
//!
//! The rule under test: a stream whose destination is a Splunk HTTP Event Collector has its
//! buffered records ATTEMPTED against that collector once the buffer is due — "the HTTP Event
//! Collector (HEC) endpoint to which Firehose sends your data" (`HECEndpoint`) — as one POST
//! of the records' own bytes, carrying `HECToken` ("a GUID that you obtain from your Splunk
//! cluster when you create a new HEC endpoint") as the collector's authorization and shaped by
//! `HECEndpointType` ("this type can be either \"Raw\" or \"Event.\""). The buffer is handed
//! over only when the collector ANSWERS that it took the events; any other answer leaves the
//! records buffered for as long as `RetryOptions$DurationInSeconds` — "the total amount of
//! time that Firehose spends on retries. This duration starts after the initial attempt to
//! send data to Splunk fails" — allows, and once that window is spent they are parked in the
//! Amazon S3 location `S3Configuration` names under an `errorCode` naming the failure class
//! THIS attempt observed.
//!
//! The rows span what the rule generalises over: the endpoint type, whether a token is
//! configured, the records the buffer holds, the status the collector answers with, the form of
//! its answer, and — for the parked events — which failure class the answer puts the delivery
//! in.
//!
//! The stream is created and the records are put over the real `awsJson1_1` wire and the parked
//! objects are read back over Amazon S3's own, through the Firehose-and-S3 harness of
//! [`crate::destination_delivery`]; the collector is an in-process transport that records what
//! it was sent, so the request this emulator renders is the request the collector receives,
//! with nothing between them.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use aws_smithy_runtime_api::client::http::{
    HttpConnector, HttpConnectorFuture, SharedHttpConnector,
};
use aws_smithy_runtime_api::client::orchestrator::{HttpRequest, HttpResponse};
use aws_smithy_runtime_api::http::StatusCode;
use aws_smithy_types::body::SdkBody;
use serde_json::{json, Value};
use service_com_amazonaws_firehose_firehose_20150804::state::delivery::{
    deliver_due, DeliveryHop, Extractions,
};

use crate::delivery_stream_lifecycle::{PAST_THE_TRANSITION, ROLE_ARN};
use crate::destination_delivery::Emulator;

/// How long an outcome is waited for before a case fails: far longer than the emulator's
/// delivery span, so a slow machine does not fail a case the clock would have served.
const DELIVERY_DEADLINE: Duration = Duration::from_secs(30);

/// How often a delivery is driven, and its outcome read, while waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// The destination member a Splunk stream is created with.
const SPLUNK_MEMBER: &str = "SplunkDestinationConfiguration";

/// Where the backup of every row below delivers the records it COPIES, which under the
/// failed-events-only backup mode is nothing: a key under it is a delivered copy and never a
/// parked event.
const DELIVERED_PREFIX: &str = "delivered/";

/// The prefix a Splunk destination's failed events are written under when its backup configures
/// no `ErrorOutputPrefix`: the `firehose:error-output-type` of that destination family.
const FAILED_OUTPUT_PREFIX: &str = "splunk-failed/";

/// The token every row that configures one carries — a GUID, which is what `HECToken` is.
const HEC_TOKEN: &str = "00000000-0000-4000-8000-00000000000b";

/// The authority of the collector every row delivers to, which the request names as its `Host`.
const COLLECTOR_HOST: &str = "collector.splunk.test";

/// The retry window a row whose events must NOT be parked by the emulator's own delivery clock
/// configures: the longest `SplunkRetryOptions$DurationInSeconds` admits (`max: 7200`).
///
/// The emulator's own clock attempts the same buffer over the transport of the request that
/// started it, and nothing this emulator serves answers at a row's collector URL — so a window
/// this wide is one that clock never reaches the end of, which leaves the attempt that decides
/// a row's outcome to this suite's own collector.
const WIDE_RETRY_SECONDS: i64 = 7_200;

/// How far past the end of [`WIDE_RETRY_SECONDS`] the parking table's attempts are made.
///
/// A delivery is a function of the instant it is made at, so an attempt made at an instant that
/// window has already closed at parks the events the collector refused rather than owing them
/// again. Nothing waits for it: this is the `now` the delivery is asked about, not a sleep.
const PAST_THE_RETRY_WINDOW: Duration = Duration::from_secs(3 * 3_600);

/// The bucket one case's destination backs its failed events up to: one per case, so what a case
/// reads back is its own delivery and not a sibling's.
fn backup_bucket(case: &str) -> String {
    format!("firehose-splunk-hec-{case}")
}

/// That bucket as the `BucketARN` a destination names it by, in the form that member's pattern
/// requires.
fn backup_bucket_arn(case: &str) -> String {
    format!("arn:aws:s3:::{}", backup_bucket(case))
}

/// The URL of the collector one case delivers to. Nothing this emulator serves answers at it, so
/// the only transport that can answer an attempt against it is the case's own [`Collector`].
fn collector_url(case: &str) -> String {
    format!("https://{COLLECTOR_HOST}/{case}/services/collector/event")
}

/// The destination configuration of a Splunk collector, with the backup every row shares.
///
/// The buffering hints ask for the shortest wait their members admit
/// (`SplunkBufferingHints$IntervalInSeconds` 0..60, `SizeInMBs` 1..5), so a case does not wait
/// out a buffering interval it is not about, and the backup mode is the member's documented
/// default ("the default value is `FailedEventsOnly`" — `S3BackupMode`), under which the only
/// object a row can produce is a parked event.
fn splunk_configuration(case: &str, endpoint_type: &str, token: Option<&str>) -> Value {
    let mut config = json!({
        "HECEndpoint": collector_url(case),
        "HECEndpointType": endpoint_type,
        "HECAcknowledgmentTimeoutInSeconds": 180,
        "RetryOptions": { "DurationInSeconds": WIDE_RETRY_SECONDS },
        "BufferingHints": { "SizeInMBs": 1, "IntervalInSeconds": 0 },
        "S3BackupMode": "FailedEventsOnly",
        "S3Configuration": {
            "RoleARN": ROLE_ARN,
            "BucketARN": backup_bucket_arn(case),
            "Prefix": DELIVERED_PREFIX,
        },
    });
    if let Some(token) = token {
        config["HECToken"] = json!(token);
    }
    config
}

/// Create the bucket a case backs up to and the stream it puts into, without waiting for the
/// stream to become `ACTIVE` — [`past_the_transition`] waits that out once for a whole table.
async fn create_case(emulator: &Emulator, case: &str, config: Value) {
    emulator.create_bucket(&backup_bucket(case)).await;
    emulator
        .firehose_ok(
            "CreateDeliveryStream",
            json!({ "DeliveryStreamName": case, SPLUNK_MEMBER: config }),
        )
        .await;
}

/// Wait out the emulator's chosen `CREATING` -> `ACTIVE` span once, so every stream a table
/// created is accepting data.
async fn past_the_transition() {
    tokio::time::sleep(PAST_THE_TRANSITION).await;
}

/// Drive every delivery this store owes, as of `at`, over `transport` until `ready` holds — or
/// fail naming `case` and what did not happen.
async fn deliver_until(
    emulator: &Emulator,
    case: &str,
    stream: &str,
    transport: &SharedHttpConnector,
    at: impl Fn() -> SystemTime,
    expected: &str,
    mut ready: impl FnMut() -> bool,
) {
    let deadline = Instant::now() + DELIVERY_DEADLINE;
    loop {
        emulator.clear_delivery_backoff(stream);
        deliver_due(
            &**emulator.engine(),
            at(),
            Extractions::Run,
            // The case's own transport, and no emulator to put a delivery's authorization
            // question to: this suite drives the collector it serves itself.
            &DeliveryHop::over_transport(transport.clone()),
        )
        .await;
        if ready() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "case {case:?}: {expected} within {DELIVERY_DEADLINE:?}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Wait for `ready` to hold without driving a delivery, so what makes it hold is the emulator's
/// own delivery clock.
async fn wait_until(case: &str, expected: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + DELIVERY_DEADLINE;
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "case {case:?}: {expected} within {DELIVERY_DEADLINE:?}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// The instant this suite asks a delivery about when the attempt must be the one that decides
/// the outcome of a spent retry window — see [`PAST_THE_RETRY_WINDOW`].
fn past_the_retry_window() -> SystemTime {
    SystemTime::now() + PAST_THE_RETRY_WINDOW
}

// ---------------------------------------------------------------------------
// The collector
// ---------------------------------------------------------------------------

/// One attempt a collector was sent.
#[derive(Debug, Clone)]
struct Captured {
    method: String,
    uri: String,
    /// Header names lowercased, which is how HTTP compares them.
    headers: std::collections::BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// What the body of the collector's answer says about the events it was sent.
///
/// The HTTP Event Collector answers one JSON document stating what it did with them, whose
/// `code` is zero for the events it took. These are the forms an answer can take around that:
/// the acknowledgment, and each way an answer can fail to be one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// The acknowledgment: the success code.
    Acknowledges,
    DisabledAck,
    MissingId,
    PendingOnce,
    ForeignId,
    /// A code of the collector's own, which is what it answers for events it did not take.
    StatesAnotherCode,
    /// A JSON object stating no code at all.
    StatesNoCode,
    /// A body that is not JSON.
    NotJson,
    /// No body at all.
    Nothing,
}

/// A Splunk HTTP Event Collector that records every attempt it is sent and answers each with one
/// status and one form of answer — the in-process transport an attempt is made over.
#[derive(Debug, Clone)]
struct Collector {
    status: u16,
    answer: Answer,
    captured: Arc<Mutex<Vec<Captured>>>,
}

impl Collector {
    /// A collector answering `status` with the acknowledgment.
    fn answering(status: u16) -> Self {
        Self::answering_with(status, Answer::Acknowledges)
    }

    /// A collector answering `status` with `answer`.
    fn answering_with(status: u16, answer: Answer) -> Self {
        Self {
            status,
            answer,
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn transport(&self) -> SharedHttpConnector {
        SharedHttpConnector::new(self.clone())
    }

    fn captured(&self) -> Vec<Captured> {
        self.captured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl HttpConnector for Collector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let captured = {
            let mut requests = self.captured.lock().unwrap_or_else(PoisonError::into_inner);
            requests.push(Captured {
                method: request.method().to_string(),
                uri: request.uri().to_string(),
                headers: request
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.to_ascii_lowercase(), value.to_string()))
                    .collect(),
                body: request.body().bytes().unwrap_or_default().to_vec(),
            });
            requests.clone()
        };
        let body = match self.answer {
            Answer::Acknowledges | Answer::PendingOnce
                if request
                    .uri()
                    .to_string()
                    .ends_with("/services/collector/ack") =>
            {
                let poll = captured.last().expect("captured ACK query");
                let channel = poll.header("x-splunk-request-channel");
                let intake = captured
                    .iter()
                    .find(|req| {
                        !req.uri.ends_with("/services/collector/ack")
                            && req.header("x-splunk-request-channel") == channel
                    })
                    .expect("intake on poll channel");
                assert!(intake.header("x-splunk-request-channel").is_some());
                assert_eq!(
                    intake.header("x-splunk-request-channel"),
                    poll.header("x-splunk-request-channel")
                );
                assert_eq!(intake.header("authorization"), poll.header("authorization"));
                assert_eq!(
                    serde_json::from_slice::<Value>(&poll.body).unwrap(),
                    json!({"acks":[73]})
                );
                assert_eq!(
                    poll.uri,
                    intake
                        .uri
                        .replace("/services/collector/event", "/services/collector/ack")
                );
                let indexed = self.answer != Answer::PendingOnce
                    || captured
                        .iter()
                        .filter(|req| {
                            req.uri == poll.uri && req.header("x-splunk-request-channel") == channel
                        })
                        .count()
                        > 1;
                json!({"acks":{"73":indexed}}).to_string()
            }
            Answer::DisabledAck => json!({"code":14,"text":"ACK is disabled"}).to_string(),
            Answer::MissingId => json!({"code":0}).to_string(),
            Answer::ForeignId
                if request
                    .uri()
                    .to_string()
                    .ends_with("/services/collector/ack") =>
            {
                json!({"acks":{"74":true}}).to_string()
            }
            Answer::ForeignId | Answer::Acknowledges | Answer::PendingOnce => {
                json!({ "text": "Success", "code": 0, "ackId": 73 }).to_string()
            }
            Answer::StatesAnotherCode => {
                json!({ "text": "Invalid data format", "code": 6 }).to_string()
            }
            Answer::StatesNoCode => json!({ "text": "Success" }).to_string(),
            Answer::NotJson => "Success".to_string(),
            Answer::Nothing => String::new(),
        };
        let status = StatusCode::try_from(self.status).expect("the collector's status is a status");
        let response = HttpResponse::new(status, SdkBody::from(body));
        HttpConnectorFuture::new(Box::pin(async move { Ok(response) }))
    }
}

// ---------------------------------------------------------------------------
// The attempt
// ---------------------------------------------------------------------------

/// One row of the attempt table: how the destination addresses its collector, and the records
/// the buffer holds.
struct AttemptRow {
    case: &'static str,
    /// `HECEndpointType`: "either \"Raw\" or \"Event.\""
    endpoint_type: &'static str,
    /// `HECToken`, or `None` to configure none — the member is not required.
    token: Option<&'static str>,
    payloads: &'static [&'static [u8]],
}

impl AttemptRow {
    /// The body the collector is sent: the buffered records' own bytes in arrival order, sent
    /// verbatim to a `Raw` endpoint and one event document per line to an `Event` one.
    fn expected_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        for payload in self.payloads {
            if self.endpoint_type != "Raw" && !body.is_empty() {
                body.push(b'\n');
            }
            body.extend_from_slice(payload);
        }
        body
    }

    /// The media type the request carries: the event documents of an `Event` endpoint are JSON,
    /// and a `Raw` endpoint takes the stream's bytes as they were put.
    fn expected_content_type(&self) -> &'static str {
        if self.endpoint_type == "Raw" {
            "text/plain"
        } else {
            "application/json"
        }
    }
}

#[tokio::test]
async fn a_splunk_destinations_buffer_is_posted_to_the_configured_hec_endpoint() {
    // Each row varies what the rule generalises over: which endpoint type the attempt is shaped
    // by, whether a token is configured at all, and how many records the buffer holds and what
    // they carry (text, JSON, arbitrary bytes, an empty blob — `Data`'s length is 0 to
    // 1,024,000). Every row's collector acknowledges, so what each case proves is that the
    // attempt was MADE, in the form the collector takes, and that an acknowledged buffer is
    // handed over.
    let rows = [
        AttemptRow {
            case: "event-endpoint",
            endpoint_type: "Event",
            token: Some(HEC_TOKEN),
            payloads: &[b"{\"event\":\"one\"}", b"{\"event\":\"two\"}"],
        },
        AttemptRow {
            case: "raw-endpoint",
            endpoint_type: "Raw",
            token: Some(HEC_TOKEN),
            payloads: &[b"first", b"second", b"third"],
        },
        AttemptRow {
            case: "no-token-configured",
            endpoint_type: "Event",
            token: None,
            payloads: &[b"{\"event\":\"tokenless\"}"],
        },
        AttemptRow {
            case: "binary-and-empty-records",
            endpoint_type: "Raw",
            token: Some(HEC_TOKEN),
            payloads: &[&[0x00, 0x9f, 0x92, 0x96, 0xff], b"", b"\n\t ascii \r\n"],
        },
    ];

    let emulator = Emulator::new();
    let mut cases = Vec::new();
    for row in rows {
        let config = splunk_configuration(row.case, row.endpoint_type, row.token);
        create_case(&emulator, row.case, config).await;
        cases.push((row, Collector::answering(200)));
    }
    past_the_transition().await;

    for (row, collector) in cases {
        let case = row.case;
        let transport = collector.transport();
        let records: Vec<Vec<u8>> = row.payloads.iter().map(|data| data.to_vec()).collect();
        let (status, answered) = emulator.put_batch(case, &records).await;
        assert_eq!(
            status, 200,
            "case {case:?}: the batch must be accepted, got {answered}"
        );
        deliver_until(
            &emulator,
            case,
            case,
            &transport,
            SystemTime::now,
            "the collector is sent the buffer",
            || !collector.captured().is_empty(),
        )
        .await;

        let captured = collector.captured();
        let attempt = captured
            .first()
            .unwrap_or_else(|| panic!("case {case:?}: an attempt was captured"));
        assert_eq!(
            attempt.method, "POST",
            "case {case:?}: an attempt against a collector is a POST, got {:?}",
            attempt.method
        );
        assert_eq!(
            attempt.uri,
            collector_url(case),
            "case {case:?}: the attempt goes to the configured HECEndpoint, got {:?}",
            attempt.uri
        );
        assert_eq!(
            attempt.header("host"),
            Some(COLLECTOR_HOST),
            "case {case:?}: the attempt names the collector's own authority, got {:?}",
            attempt.header("host")
        );
        assert_eq!(
            attempt.header("content-type"),
            Some(row.expected_content_type()),
            "case {case:?}: the body is the media type a {:?} endpoint takes, got {:?}",
            row.endpoint_type,
            attempt.header("content-type")
        );
        let expected_authorization = row.token.map(|token| format!("Splunk {token}"));
        assert_eq!(
            attempt.header("authorization").map(str::to_string),
            expected_authorization,
            "case {case:?}: the attempt carries the configured HECToken under the collector's \
             own scheme and nothing else, got {:?}",
            attempt.header("authorization")
        );
        assert_eq!(
            attempt.body,
            row.expected_body(),
            "case {case:?}: the attempt carries every buffered record in arrival order, got {:?}",
            String::from_utf8_lossy(&attempt.body)
        );

        // An acknowledged buffer is handed over: nothing of it is still waiting for the
        // collector, and the failed-events-only backup takes nothing.
        deliver_until(
            &emulator,
            case,
            case,
            &transport,
            SystemTime::now,
            "the buffer is handed over",
            || emulator.buffered(case).is_empty(),
        )
        .await;
        let written = emulator.keys_under(&backup_bucket(case), "").await;
        assert!(
            written.is_empty(),
            "case {case:?}: an indexed buffer writes nothing to the failed-events backup, got \
             {written:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The acknowledgment
// ---------------------------------------------------------------------------

/// One row of the acknowledgment table: how the collector answers an attempt, and whether that
/// answer hands the buffer over.
struct AnswerRow {
    case: &'static str,
    status: u16,
    answer: Answer,
    acknowledges: bool,
}

#[tokio::test]
async fn only_an_answer_that_states_the_collector_took_the_events_hands_a_buffer_over() {
    // Each row varies the answer around the one document the collector states what it did in:
    // the success code, a code of its own, no code, a body that is not JSON, no body — and the
    // status it is answered with, which sits on both sides of each boundary of the success
    // class. The retry window of every row is wide, so a buffer an answer did not hand over is
    // one the records are still waiting for rather than one that is parked.
    let rows = [
        AnswerRow {
            case: "pending-then-indexed",
            status: 200,
            answer: Answer::PendingOnce,
            acknowledges: true,
        },
        AnswerRow {
            case: "missing-ack-id",
            status: 200,
            answer: Answer::MissingId,
            acknowledges: false,
        },
        AnswerRow {
            case: "only-foreign-id-confirmed",
            status: 200,
            answer: Answer::ForeignId,
            acknowledges: false,
        },
        AnswerRow {
            case: "acknowledged",
            status: 200,
            answer: Answer::Acknowledges,
            acknowledges: true,
        },
        AnswerRow {
            case: "acknowledged-at-299",
            status: 299,
            answer: Answer::Acknowledges,
            acknowledges: true,
        },
        AnswerRow {
            case: "states-another-code",
            status: 200,
            answer: Answer::StatesAnotherCode,
            acknowledges: false,
        },
        AnswerRow {
            case: "states-no-code",
            status: 200,
            answer: Answer::StatesNoCode,
            acknowledges: false,
        },
        AnswerRow {
            case: "answers-what-is-not-json",
            status: 200,
            answer: Answer::NotJson,
            acknowledges: false,
        },
        AnswerRow {
            case: "answers-no-body",
            status: 200,
            answer: Answer::Nothing,
            acknowledges: false,
        },
        AnswerRow {
            case: "below-the-success-class",
            status: 199,
            answer: Answer::Acknowledges,
            acknowledges: false,
        },
        AnswerRow {
            case: "refuses-the-token",
            status: 401,
            answer: Answer::Acknowledges,
            acknowledges: false,
        },
        AnswerRow {
            case: "refuses-the-data",
            status: 400,
            answer: Answer::Acknowledges,
            acknowledges: false,
        },
        AnswerRow {
            case: "answers-a-server-error",
            status: 503,
            answer: Answer::Acknowledges,
            acknowledges: false,
        },
    ];
    let payloads: Vec<Vec<u8>> = vec![b"first".to_vec(), b"second".to_vec()];

    let emulator = Emulator::new();
    let mut cases = Vec::new();
    for row in rows {
        let config = splunk_configuration(row.case, "Event", Some(HEC_TOKEN));
        create_case(&emulator, row.case, config).await;
        let collector = Collector::answering_with(row.status, row.answer);
        cases.push((row, collector));
    }
    past_the_transition().await;

    for (row, collector) in cases {
        let case = row.case;
        let transport = collector.transport();
        let (status, answered) = emulator.put_batch(case, &payloads).await;
        assert_eq!(
            status, 200,
            "case {case:?}: the batch must be accepted, got {answered}"
        );
        deliver_until(
            &emulator,
            case,
            case,
            &transport,
            SystemTime::now,
            "the collector is sent the buffer",
            || !collector.captured().is_empty(),
        )
        .await;

        if row.acknowledges {
            deliver_until(
                &emulator,
                case,
                case,
                &transport,
                SystemTime::now,
                "the buffer is handed over",
                || emulator.buffered(case).is_empty(),
            )
            .await;
        } else {
            // The records are neither indexed nor lost: they stay buffered for the retry
            // window. The buffer is taken out of the store for the duration of an attempt, so
            // this waits for the restored buffer rather than reading once.
            wait_until(case, "the records are back in the buffer", || {
                emulator.buffered(case) == payloads
            })
            .await;
        }
        let written = emulator.keys_under(&backup_bucket(case), "").await;
        assert!(
            written.is_empty(),
            "case {case:?}: nothing is written to the backup while the retry window is open, got \
             {written:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The failure class
// ---------------------------------------------------------------------------

/// One row of the parking table: how the collector refused the attempt, and the failure class
/// the error output of the parked events names it by.
struct FailureRow {
    case: &'static str,
    status: u16,
    answer: Answer,
    /// The `errorCode` the parked documents carry. The classes Firehose publishes for a Splunk
    /// destination, each prefixed with that destination family: a status refusing the credential
    /// is the token's, one refusing the request's content is the data's, any other error status
    /// is the indexer's, and an answer that does not state the collector took the events is one
    /// no acknowledgment arrived for.
    error_code: &'static str,
}

/// The fixed prefix every one of those codes carries: the destination family.
const FAILED_ERROR_CODE_PREFIX: &str = "Splunk.";

#[tokio::test]
async fn events_a_splunk_collector_refused_are_parked_under_the_class_the_attempt_observed() {
    // Each row varies how the collector refused the attempt, which is what the failure class of
    // the parked events is read from. Each row's retry window is the widest the member admits
    // and the attempt that parks the events is made at an instant past the end of it, so what
    // classifies a row's events is its own collector's answer and nothing else.
    let rows = [
        FailureRow {
            case: "ack-disabled",
            status: 400,
            answer: Answer::DisabledAck,
            error_code: "Splunk.DisabledAck",
        },
        FailureRow {
            case: "token-refused",
            status: 401,
            answer: Answer::Acknowledges,
            error_code: "Splunk.InvalidToken",
        },
        FailureRow {
            case: "token-forbidden",
            status: 403,
            answer: Answer::Acknowledges,
            error_code: "Splunk.InvalidToken",
        },
        FailureRow {
            case: "data-refused",
            status: 400,
            answer: Answer::Acknowledges,
            error_code: "Splunk.InvalidDataFormat",
        },
        FailureRow {
            case: "indexer-unavailable",
            status: 503,
            answer: Answer::Acknowledges,
            error_code: "Splunk.ServerError",
        },
        FailureRow {
            case: "indexer-error",
            status: 500,
            answer: Answer::Acknowledges,
            error_code: "Splunk.ServerError",
        },
        FailureRow {
            case: "events-not-acknowledged",
            status: 200,
            answer: Answer::StatesAnotherCode,
            error_code: "Splunk.AckTimeout",
        },
    ];
    let payloads: Vec<Vec<u8>> = vec![b"the-first-event".to_vec(), b"the-second-event".to_vec()];

    let emulator = Emulator::new();
    let mut cases = Vec::new();
    for row in rows {
        let config = splunk_configuration(row.case, "Event", Some(HEC_TOKEN));
        create_case(&emulator, row.case, config).await;
        let collector = Collector::answering_with(row.status, row.answer);
        cases.push((row, collector));
    }
    past_the_transition().await;

    for (row, collector) in cases {
        let case = row.case;
        let bucket = backup_bucket(case);
        let transport = collector.transport();
        let (status, answered) = emulator.put_batch(case, &payloads).await;
        assert_eq!(
            status, 200,
            "case {case:?}: the batch must be accepted, got {answered}"
        );
        // Every attempt is made at an instant the retry window has already closed at, so the
        // first one this collector refuses parks the events.
        deliver_until(
            &emulator,
            case,
            case,
            &transport,
            past_the_retry_window,
            "the refused events are parked in the backup",
            || !collector.captured().is_empty() && emulator.buffered(case).is_empty(),
        )
        .await;

        let written = emulator.keys_under(&bucket, "").await;
        let parked = emulator.keys_under(&bucket, FAILED_OUTPUT_PREFIX).await;
        let key = parked.first().unwrap_or_else(|| {
            panic!(
                "case {case:?}: an object under {FAILED_OUTPUT_PREFIX:?} holds the refused \
                 events, got {written:?}"
            )
        });
        let body = emulator.object(&bucket, key).await;
        // One error-output document per record, each naming how the attempt failed and carrying
        // the record's own bytes, so nothing of a parked event is lost.
        let documents: Vec<Value> = String::from_utf8_lossy(&body)
            .lines()
            .map(|line| {
                serde_json::from_str(line).unwrap_or_else(|error| {
                    panic!("case {case:?}: every parked line is a JSON document, got {error}")
                })
            })
            .collect();
        assert_eq!(
            documents.len(),
            payloads.len(),
            "case {case:?}: one document per refused event, got {documents:?}"
        );
        for (document, payload) in documents.iter().zip(&payloads) {
            let code = document["errorCode"].as_str().unwrap_or_default();
            assert!(
                code.starts_with(FAILED_ERROR_CODE_PREFIX),
                "case {case:?}: the errorCode names the destination family \
                 {FAILED_ERROR_CODE_PREFIX:?}, got {code:?}"
            );
            assert_eq!(
                code, row.error_code,
                "case {case:?}: the errorCode names the class this attempt observed, got {code:?}"
            );
            assert!(
                document["errorMessage"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty()),
                "case {case:?}: the document states what the code means, got {document:?}"
            );
            let raw = document["rawData"].as_str().unwrap_or_default();
            let decoded = aws_smithy_types::base64::decode(raw).unwrap_or_else(|error| {
                panic!("case {case:?}: rawData is base64, got {error} for {raw:?}")
            });
            assert_eq!(
                &decoded, payload,
                "case {case:?}: the document carries the event's own bytes, got {decoded:?}"
            );
        }
        let delivered = emulator.keys_under(&bucket, DELIVERED_PREFIX).await;
        assert!(
            delivered.is_empty(),
            "case {case:?}: the failed-events-only backup copies no incoming record, got \
             {delivered:?}"
        );
    }
}
