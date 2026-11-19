// SPDX-License-Identifier: CC0-1.0

//! A rule that fires on its own SCHEDULE starts the Step Functions state machine its target
//! names, under the IAM role that target declares — the four services the question spans,
//! mounted on ONE engine exactly as `core/router/src/lib.rs` mounts them.
//!
//! RULE: "rules with `ScheduleExpressions` self-trigger based on the given schedule"
//! (`PutRule`) — such a rule is triggered by nothing a client publishes, and EventBridge
//! invokes each of its targets on that cadence ACTING AS the role the target declares,
//! handing a target that declares no `Input` the scheduled-event envelope EventBridge
//! publishes itself: `version`, an `id` that is fresh on every firing, `detail-type`
//! "Scheduled Event", `source` `aws.events`, the `account` and `region` the rule lives in,
//! the firing's own `time`, `resources` holding the rule's own ARN and nothing else, and an
//! empty `detail`. For a state-machine target that invocation is `states:StartExecution`, so
//! a firing is readable where a client reads it — in the executions Step Functions holds for
//! that machine, started with that document. A target whose ROLE cannot make the call starts
//! nothing, and the firing it could not be handed is written to that target's dead-letter
//! queue under EventBridge's own classification of which step failed.
//!
//! Four crates have to agree for that to hold (EventBridge fires the rule and makes the
//! delivery, IAM holds the role documents the delivery is judged against, Step Functions
//! serves `StartExecution` and runs the execution, Amazon SQS holds the dead letter), so the
//! suite lives here rather than in any of them.
//!
//! The rows differ along what the rule generalises over: the form the `ScheduleExpression`
//! takes — `rate` with a singular and with a plural unit, stated in minutes, in hours and in
//! days (IaC-Eval fork, P2: the `cron` rows are gone, a cron rule first fires at the first
//! instant it names). Every one of them has its FIRST firing fall due
//! the same span after the rule can reach a target, so what a row pins is that firing; which
//! INSTANTS each form then names is the reading pinned by the unit tests of the service's own
//! `state::schedule`, and the cadence between two firings, together with the input overrides
//! a target may declare, is pinned beside the service in its crate's
//! `tests/all/scheduled_rule_firing.rs`. What needs these four services together — the role a
//! firing is made as, and the execution it really starts — is what this suite holds.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use serde_json::{json, Value};
use smithy_server_runtime::MultiServiceRouter;

// The percent-encoder for an `application/x-www-form-urlencoded` component and the reader of
// one element of an awsQuery answer, which `all/cloudfront_key_value_store_handshake.rs` and
// `all/query_wire.rs` already own: `tests/all/main.rs` is one crate root, so IAM's wire is
// driven through those rather than through a second copy of each.
use crate::cloudfront_key_value_store_handshake::segment as form_encode;
use crate::query_wire::element;

/// The region every request below is signed for.
const REGION: &str = "us-west-2";

/// The principal EventBridge acts as, which a target's role has to trust and a dead-letter
/// queue's own policy has to admit the write from.
const EVENTS_PRINCIPAL: &str = "events.amazonaws.com";

/// The `source` EventBridge publishes a scheduled event under.
const SCHEDULED_SOURCE: &str = "aws.events";

/// The `detail-type` it publishes one under.
const SCHEDULED_DETAIL_TYPE: &str = "Scheduled Event";

/// The `version` every event envelope of this service carries.
const EVENT_VERSION: &str = "0";

/// The IAM action EventBridge performs to invoke a state-machine target: "for Step Functions
/// state machines […] EventBridge relies on IAM roles that you specify in the RoleARN
/// argument in PutTargets", and the call it makes as that role is `StartExecution`.
const START_EXECUTION_ACTION: &str = "states:StartExecution";

/// The condition EventBridge names on a dead letter for a delivery whose role it could not
/// assume at all, and the one for a role it could assume whose policies withhold the call.
/// Spelled here rather than imported from the service, so a wrong value in the service would
/// fail this suite.
const FAILED_TO_ASSUME_ROLE: &str = "FAILED_TO_ASSUME_ROLE";
const NO_PERMISSIONS: &str = "NO_PERMISSIONS";

/// The members AWS's scheduled-event document carries, which is what a target declaring no
/// input override is handed.
const ENVELOPE_MEMBERS: [&str; 9] = [
    "account",
    "detail",
    "detail-type",
    "id",
    "region",
    "resources",
    "source",
    "time",
    "version",
];

/// How long a case waits for the FIRST firing of a rule it has just targeted: generously
/// wider than the span this emulator holds a new schedule rule for, so a loaded host does
/// not decide the verdict.
const FIRING_PATIENCE: Duration = Duration::from_secs(60);

/// How long a case waits for an execution to leave `RUNNING`.
const TERMINAL_PATIENCE: Duration = Duration::from_secs(30);

/// How long one turn of a poll loop waits before looking again.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The four services this question spans, over ONE shared engine.
fn emulator() -> MultiServiceRouter {
    let engine: Arc<engine::InMemoryEngine> = Arc::new(engine::InMemoryEngine::new());
    let mut router = MultiServiceRouter::new();
    router.set_request_authentication(false);
    router.register_service(
        service_com_amazonaws_eventbridge_awsevents::EmulatedService::new(Arc::clone(&engine)),
    );
    router.register_service(
        service_com_amazonaws_iam_awsidentitymanagementv20100508::EmulatedService::new(Arc::clone(
            &engine,
        )),
    );
    router.register_service(service_com_amazonaws_sfn_awsstepfunctions::EmulatedService::new(
        Arc::clone(&engine),
    ));
    router.register_service(service_com_amazonaws_sqs_amazonsqs::EmulatedService::new(
        engine,
    ));
    router
}

/// The account id every ARN this emulator mints carries.
fn account() -> &'static str {
    engine::emulator_account_id()
}

/// A SigV4 `Authorization` header whose credential scope names `service`, which is what
/// routes the request. The signature is never checked: this router runs with request
/// authentication off.
fn authorization(service: &str) -> String {
    format!(
        "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20260101/{REGION}/{service}/aws4_request, \
         SignedHeaders=host, Signature=0000"
    )
}

/// Call one operation of an `awsJson` service, requiring a 200.
async fn rpc(
    router: &MultiServiceRouter,
    signing_name: &str,
    target_prefix: &str,
    json_version: &str,
    operation: &str,
    body: &Value,
) -> Value {
    let request = http::Request::builder()
        .method("POST")
        .uri("/")
        .header("host", format!("{signing_name}.{REGION}.amazonaws.com"))
        .header(
            "content-type",
            format!("application/x-amz-json-{json_version}"),
        )
        .header("x-amz-target", format!("{target_prefix}.{operation}"))
        .header("authorization", authorization(signing_name))
        .body(Bytes::from(body.to_string()))
        .expect("a well-formed request");
    let response = router.dispatch(request).await;
    let status = response.status().as_u16();
    let parsed: Value = if response.body().is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(response.body()).unwrap_or(Value::Null)
    };
    assert_eq!(
        status, 200,
        "case {operation}: expected 200, got {status} with {parsed:?}"
    );
    parsed
}

/// Call one EventBridge operation (`awsJson1_1`, `AWSEvents.<Operation>`).
async fn events(router: &MultiServiceRouter, operation: &str, body: &Value) -> Value {
    rpc(router, "events", "AWSEvents", "1.1", operation, body).await
}

/// Call one Step Functions operation (`awsJson1_0`, `AWSStepFunctions.<Operation>`).
async fn sfn(router: &MultiServiceRouter, operation: &str, body: &Value) -> Value {
    rpc(router, "states", "AWSStepFunctions", "1.0", operation, body).await
}

/// Call one Amazon SQS operation (`awsJson1_0`, `AmazonSQS.<Operation>`).
async fn sqs(router: &MultiServiceRouter, operation: &str, body: &Value) -> Value {
    rpc(router, "sqs", "AmazonSQS", "1.0", operation, body).await
}

/// One awsQuery call to IAM, requiring a 200, answered with the XML body.
async fn iam(router: &MultiServiceRouter, action: &str, params: &[(&str, String)]) -> String {
    let mut body = format!("Action={action}&Version=2010-05-08");
    for (name, value) in params {
        body.push('&');
        body.push_str(&form_encode(name));
        body.push('=');
        body.push_str(&form_encode(value));
    }
    let request = http::Request::builder()
        .method("POST")
        .uri("/")
        .header("host", "iam.amazonaws.com")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", authorization("iam"))
        .body(Bytes::from(body))
        .expect("a well-formed awsQuery request");
    let response = router.dispatch(request).await;
    let status = response.status().as_u16();
    let text = String::from_utf8_lossy(response.body()).to_string();
    assert_eq!(
        status, 200,
        "case {action}: expected 200 from IAM, got {status} with {text}"
    );
    text
}

/// The trust document of a role a service may act as: a role is assumable by the service
/// principal its own trust policy admits, and by no other.
fn trusted_by(service_principal: &str) -> Value {
    json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": { "Service": service_principal },
            "Action": "sts:AssumeRole",
        }],
    })
}

/// An IAM role called `name`, trusting `trust` and carrying `policy` inline; answers its ARN.
async fn role(router: &MultiServiceRouter, name: &str, trust: &Value, policy: &Value) -> String {
    let created = iam(
        router,
        "CreateRole",
        &[
            ("RoleName", name.to_string()),
            ("AssumeRolePolicyDocument", trust.to_string()),
        ],
    )
    .await;
    let arn = element(&created, "Arn");
    iam(
        router,
        "PutRolePolicy",
        &[
            ("RoleName", name.to_string()),
            ("PolicyName", "inline".to_string()),
            ("PolicyDocument", policy.to_string()),
        ],
    )
    .await;
    arn
}

/// A policy whose one statement allows `action` on `resource`.
fn allowing(action: &str, resource: &str) -> Value {
    json!({
        "Version": "2012-10-17",
        "Statement": [{ "Effect": "Allow", "Action": action, "Resource": resource }],
    })
}

/// The role Step Functions runs an execution as: the role a state machine is created with,
/// which that service is the one to act as.
async fn state_machine_role(router: &MultiServiceRouter, name: &str) -> String {
    role(
        router,
        name,
        &trusted_by("states.amazonaws.com"),
        &allowing("logs:PutLogEvents", "*"),
    )
    .await
}

/// A trivial STANDARD state machine — one `Pass` state, so an execution of it succeeds on
/// its own — created under `role_arn`; answers its ARN.
async fn state_machine(router: &MultiServiceRouter, name: &str, role_arn: &str) -> String {
    let definition = json!({
        "StartAt": "Pass",
        "States": { "Pass": { "Type": "Pass", "End": true } },
    });
    let created = sfn(
        router,
        "CreateStateMachine",
        &json!({
            "name": name,
            "definition": definition.to_string(),
            "roleArn": role_arn,
            "type": "STANDARD",
        }),
    )
    .await;
    created["stateMachineArn"]
        .as_str()
        .unwrap_or_else(|| panic!("CreateStateMachine answers a stateMachineArn, got {created}"))
        .to_string()
}

/// A dead-letter queue whose own policy admits the write EventBridge makes to it, so what
/// reaches it is decided by the target's role rather than by the queue's document ("for …
/// Amazon SQS queues, EventBridge relies on resource-based policies"). Answers its url and
/// the ARN a target's `DeadLetterConfig` names it by, read back from the queue.
async fn dead_letter_queue(router: &MultiServiceRouter, name: &str) -> (String, String) {
    let created = sqs(router, "CreateQueue", &json!({ "QueueName": name })).await;
    let url = created["QueueUrl"]
        .as_str()
        .unwrap_or_else(|| panic!("CreateQueue answers a QueueUrl, got {created}"))
        .to_string();
    let attributes = sqs(
        router,
        "GetQueueAttributes",
        &json!({ "QueueUrl": url, "AttributeNames": ["QueueArn"] }),
    )
    .await;
    let arn = attributes["Attributes"]["QueueArn"]
        .as_str()
        .unwrap_or_else(|| panic!("GetQueueAttributes answers the QueueArn, got {attributes}"))
        .to_string();
    let policy = json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": { "Service": EVENTS_PRINCIPAL },
            "Action": "sqs:SendMessage",
            "Resource": arn,
        }],
    });
    sqs(
        router,
        "SetQueueAttributes",
        &json!({ "QueueUrl": url, "Attributes": { "Policy": policy.to_string() } }),
    )
    .await;
    (url, arn)
}

/// The messages waiting on the queue at `url`, with their attributes.
async fn received(router: &MultiServiceRouter, url: &str) -> Vec<Value> {
    let answer = sqs(
        router,
        "ReceiveMessage",
        &json!({
            "QueueUrl": url,
            "MaxNumberOfMessages": 10,
            "MessageAttributeNames": ["All"],
        }),
    )
    .await;
    answer["Messages"].as_array().cloned().unwrap_or_default()
}

/// The one message the queue at `url` holds once it holds one, looking again until it does or
/// `FIRING_PATIENCE` has passed with it not doing so.
async fn dead_letter(router: &MultiServiceRouter, case: &str, why: &str, url: &str) -> Value {
    let began = Instant::now();
    loop {
        let waiting = received(router, url).await;
        if let Some(message) = waiting.into_iter().next() {
            return message;
        }
        assert!(
            began.elapsed() < FIRING_PATIENCE,
            "case {case:?}: expected the firing {why} could not be handed over to be dead-lettered"
        );
        ::tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Write a rule firing on `schedule` with `targets` as its targets, and answer the rule's own
/// ARN — which is what the envelope of every firing of it names as its one resource.
async fn schedule_rule(
    router: &MultiServiceRouter,
    case: &str,
    name: &str,
    schedule: &str,
    targets: Value,
) -> String {
    let written = events(
        router,
        "PutRule",
        &json!({ "Name": name, "ScheduleExpression": schedule, "State": "ENABLED" }),
    )
    .await;
    let rule_arn = written["RuleArn"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: PutRule answers a RuleArn, got {written}"))
        .to_string();
    let targeted = events(
        router,
        "PutTargets",
        &json!({ "Rule": name, "Targets": targets }),
    )
    .await;
    assert_eq!(
        targeted["FailedEntryCount"],
        json!(0),
        "case {case:?}: PutTargets reported a failed entry: {targeted}"
    );
    rule_arn
}

/// The executions the state machine at `arn` holds.
async fn executions(router: &MultiServiceRouter, arn: &str) -> Vec<Value> {
    let listed = sfn(router, "ListExecutions", &json!({ "stateMachineArn": arn })).await;
    listed["executions"].as_array().cloned().unwrap_or_default()
}

/// The executions of the machine at `arn` once it holds at least `wanted` of them, looking
/// again until it does or `FIRING_PATIENCE` has passed with it not doing so.
async fn executions_once_there_are(
    router: &MultiServiceRouter,
    case: &str,
    arn: &str,
    wanted: usize,
) -> Vec<Value> {
    let began = Instant::now();
    loop {
        let held = executions(router, arn).await;
        if held.len() >= wanted {
            return held;
        }
        assert!(
            began.elapsed() < FIRING_PATIENCE,
            "case {case:?}: expected {wanted} execution(s) of {arn} from the rule's own \
             schedule, got {held:?}"
        );
        ::tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// The execution at `execution_arn` once it has left `RUNNING`.
async fn once_terminal(router: &MultiServiceRouter, case: &str, execution_arn: &str) -> Value {
    let began = Instant::now();
    loop {
        let described = sfn(
            router,
            "DescribeExecution",
            &json!({ "executionArn": execution_arn }),
        )
        .await;
        if described["status"].as_str() != Some("RUNNING") {
            return described;
        }
        assert!(
            began.elapsed() < TERMINAL_PATIENCE,
            "case {case:?}: expected a terminal status, got {described}"
        );
        ::tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Check that `document` is EventBridge's own scheduled-event envelope for the rule at
/// `rule_arn`, and answer the `id` it carries.
fn scheduled_envelope_id(case: &str, document: &str, rule_arn: &str) -> String {
    let envelope: Value = serde_json::from_str(document).unwrap_or_else(|error| {
        panic!("case {case:?}: the handed-over document is not JSON ({error}): {document:?}")
    });
    let carried: BTreeSet<String> = envelope
        .as_object()
        .unwrap_or_else(|| panic!("case {case:?}: expected a JSON document, got {envelope}"))
        .keys()
        .cloned()
        .collect();
    let expected: BTreeSet<String> = ENVELOPE_MEMBERS.iter().map(|name| name.to_string()).collect();
    assert_eq!(
        carried, expected,
        "case {case:?}: expected exactly the scheduled event's own members, got {envelope}"
    );
    assert_eq!(
        envelope["version"].as_str(),
        Some(EVENT_VERSION),
        "case {case:?}: expected the envelope's version, got {envelope}"
    );
    assert_eq!(
        envelope["source"].as_str(),
        Some(SCHEDULED_SOURCE),
        "case {case:?}: expected EventBridge's own source, got {envelope}"
    );
    assert_eq!(
        envelope["detail-type"].as_str(),
        Some(SCHEDULED_DETAIL_TYPE),
        "case {case:?}: expected the scheduled detail-type, got {envelope}"
    );
    assert_eq!(
        envelope["detail"],
        json!({}),
        "case {case:?}: expected an empty detail, got {envelope}"
    );
    assert_eq!(
        envelope["region"].as_str(),
        Some(REGION),
        "case {case:?}: expected the rule's own region, got {envelope}"
    );
    assert_eq!(
        envelope["account"].as_str(),
        Some(account()),
        "case {case:?}: expected the account the rule belongs to, got {envelope}"
    );
    assert_eq!(
        envelope["resources"],
        json!([rule_arn]),
        "case {case:?}: expected the rule's own ARN as the one resource, got {envelope}"
    );
    let time = envelope["time"].as_str().unwrap_or_else(|| {
        panic!("case {case:?}: expected the firing's own instant, got {envelope}")
    });
    assert!(
        time.ends_with('Z') && time.contains('T'),
        "case {case:?}: expected a UTC timestamp, got {time:?} in {envelope}"
    );
    let id = envelope["id"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: expected an event id, got {envelope}"));
    assert!(
        !id.is_empty(),
        "case {case:?}: expected a non-empty event id, got {envelope}"
    );
    id.to_string()
}

/// One row: the form of the `ScheduleExpression`, and the names the row's own resources take.
struct Row {
    case: &'static str,
    slug: &'static str,
    schedule: &'static str,
}

/// Every `rate` form a `ScheduleExpression` states a cadence in. The first firing of each falls
/// due the same span after the rule can reach its target, so a row's cadence is not waited out.
const ROWS: &[Row] = &[
    Row {
        case: "a rate of one minute, the shortest a rate can state",
        slug: "rate-one-minute",
        schedule: "rate(1 minute)",
    },
    Row {
        case: "a rate of several minutes, stated in the plural unit",
        slug: "rate-five-minutes",
        schedule: "rate(5 minutes)",
    },
    Row {
        case: "a rate stated in hours",
        slug: "rate-one-hour",
        schedule: "rate(1 hour)",
    },
    Row {
        case: "a rate stated in days",
        slug: "rate-two-days",
        schedule: "rate(2 days)",
    },
    // IaC-Eval fork (P2): the two `cron` rows are gone. A cron rule's first firing is now the
    // first instant its expression names (up to an hour away for `cron(0 * * * ? *)`), not the
    // span after the rule can reach its target that this table waits for; that first firing is
    // pinned by `a_new_cron_rule_first_fires_at_its_first_named_instant` in the service's own
    // `state::schedule` unit tests.
];

/// What one row wired up, carried from the setup pass to the verification pass.
struct Wired {
    case: &'static str,
    rule_name: String,
    rule_arn: String,
    machine_arn: String,
    role_arn: String,
}

/// Every form of schedule fires, with nothing published, and what the firing starts is an
/// execution of the target state machine carrying EventBridge's own scheduled-event envelope.
///
/// The rows are wired up first and verified afterwards: each rule's first firing is owed from
/// the moment that rule can reach its target, so one wait covers the table rather than one
/// per row.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_scheduled_rule_starts_its_state_machine_with_the_scheduled_event() {
    let router = emulator();
    let machine_role = state_machine_role(&router, "sfn-scheduled-executions").await;

    let mut wired = Vec::with_capacity(ROWS.len());
    for row in ROWS {
        let machine_arn =
            state_machine(&router, &format!("scheduled-{}", row.slug), &machine_role).await;
        // The role EventBridge acts as to start that one machine's executions, which is the
        // grant the reference topology writes for a schedule rule's state-machine target.
        let role_arn = role(
            &router,
            &format!("events-start-{}", row.slug),
            &trusted_by(EVENTS_PRINCIPAL),
            &allowing(START_EXECUTION_ACTION, &machine_arn),
        )
        .await;
        let rule_name = format!("schedule-{}", row.slug);
        let rule_arn = schedule_rule(
            &router,
            row.case,
            &rule_name,
            row.schedule,
            json!([{ "Id": "to-state-machine", "Arn": machine_arn, "RoleArn": role_arn }]),
        )
        .await;
        wired.push(Wired {
            case: row.case,
            rule_name,
            rule_arn,
            machine_arn,
            role_arn,
        });
    }

    for row in &wired {
        let case = row.case;
        // The wiring a client reads back before it watches the target.
        let listed = events(
            &router,
            "ListTargetsByRule",
            &json!({ "Rule": row.rule_name }),
        )
        .await;
        let targets = listed["Targets"].as_array().cloned().unwrap_or_default();
        assert_eq!(
            targets.len(),
            1,
            "case {case:?}: expected the one target the rule was given, got {listed}"
        );
        assert_eq!(
            targets[0]["Arn"].as_str(),
            Some(row.machine_arn.as_str()),
            "case {case:?}: expected the target to name the state machine, got {listed}"
        );
        assert_eq!(
            targets[0]["RoleArn"].as_str(),
            Some(row.role_arn.as_str()),
            "case {case:?}: expected the target to name the role it is invoked as, got {listed}"
        );

        let started = executions_once_there_are(&router, case, &row.machine_arn, 1).await;
        let execution_arn = started[0]["executionArn"]
            .as_str()
            .unwrap_or_else(|| {
                panic!("case {case:?}: an execution carries its ARN, got {started:?}")
            })
            .to_string();
        let described = once_terminal(&router, case, &execution_arn).await;
        let input = described["input"].as_str().unwrap_or_else(|| {
            panic!(
                "case {case:?}: the execution reports the input it was started with, got \
                 {described}"
            )
        });
        scheduled_envelope_id(case, input, &row.rule_arn);
        // The execution really ran: a `Pass` state machine started by the firing reaches its
        // own terminal status, so the firing is backed by a run rather than by a record of one.
        assert_eq!(
            described["status"].as_str(),
            Some("SUCCEEDED"),
            "case {case:?}: expected the started execution to run to completion, got {described}"
        );
    }
}

/// One target of the refusal case: why its role cannot make the call, and what EventBridge's
/// own classification of that failure is.
struct Refused {
    /// The target's `Id`, which is what the dead letter names it by.
    id: &'static str,
    why: &'static str,
    /// The service principal the target's role trusts: EventBridge's own, or another
    /// service's — a role it cannot assume at all.
    trusts: &'static str,
    /// The action the role's inline policy allows.
    allows: &'static str,
    /// The `ERROR_CODE` the dead letter carries.
    code: &'static str,
}

/// A firing is made ACTING AS the target's role, so a target whose role cannot make the call
/// starts nothing — and the firing it could not be handed goes to that target's dead-letter
/// queue, naming the rule, the target and which step failed. Another target of the SAME
/// firing, whose role can make the call, is started and dead-letters nothing.
///
/// The two refused targets are the two steps the delivery takes: a role EventBridge may not
/// assume at all (its trust names another service), and one it may assume whose policies
/// withhold the call. Each refusal is read where the service RECORDS it — the dead letter —
/// rather than inferred from an execution's absence, so nothing here rests on the order the
/// targets of one firing are handed over in.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_firing_whose_role_cannot_start_the_execution_dead_letters_it_and_starts_nothing() {
    const REFUSED: &[Refused] = &[
        Refused {
            id: "untrusted",
            why: "under a role EventBridge may not assume",
            trusts: "ecs-tasks.amazonaws.com",
            allows: START_EXECUTION_ACTION,
            code: FAILED_TO_ASSUME_ROLE,
        },
        Refused {
            id: "ungranted",
            why: "under a role whose policies withhold the call",
            trusts: EVENTS_PRINCIPAL,
            // Nothing about starting an execution, which in IAM is an implicit deny.
            allows: "logs:PutLogEvents",
            code: NO_PERMISSIONS,
        },
    ];
    let case = "a schedule rule with a granted and two refused targets";
    let router = emulator();
    let machine_role = state_machine_role(&router, "sfn-refusals").await;

    // Every target of the one rule: each with a state machine of its own and a dead-letter
    // queue of its own, so what each handover came to is read on its own.
    let mut targets = Vec::new();
    let mut refused_targets = Vec::new();
    for refused in REFUSED {
        let machine_arn = state_machine(&router, &format!("{}-target", refused.id), &machine_role)
            .await;
        let role_arn = role(
            &router,
            &format!("events-{}", refused.id),
            &trusted_by(refused.trusts),
            &allowing(refused.allows, &machine_arn),
        )
        .await;
        let (queue_url, queue_arn) =
            dead_letter_queue(&router, &format!("{}-dead-letters", refused.id)).await;
        targets.push(json!({
            "Id": refused.id,
            "Arn": machine_arn,
            "RoleArn": role_arn,
            "DeadLetterConfig": { "Arn": queue_arn },
        }));
        refused_targets.push((refused, machine_arn, queue_url));
    }
    let granted_machine = state_machine(&router, "granted-target", &machine_role).await;
    let granted_role = role(
        &router,
        "events-granted",
        &trusted_by(EVENTS_PRINCIPAL),
        &allowing(START_EXECUTION_ACTION, &granted_machine),
    )
    .await;
    let (granted_queue_url, granted_queue_arn) =
        dead_letter_queue(&router, "granted-dead-letters").await;
    targets.push(json!({
        "Id": "granted",
        "Arn": granted_machine,
        "RoleArn": granted_role,
        "DeadLetterConfig": { "Arn": granted_queue_arn },
    }));

    let rule_arn = schedule_rule(
        &router,
        case,
        "refusals",
        "rate(1 minute)",
        Value::Array(targets),
    )
    .await;

    // The granted target: an execution of its own machine, started with the envelope.
    let started = executions_once_there_are(&router, case, &granted_machine, 1).await;
    let execution_arn = started[0]["executionArn"]
        .as_str()
        .unwrap_or_else(|| panic!("case {case:?}: an execution carries its ARN, got {started:?}"));
    let described = once_terminal(&router, case, execution_arn).await;
    let input = described["input"].as_str().unwrap_or_else(|| {
        panic!("case {case:?}: the execution reports its input, got {described}")
    });
    let started_with = scheduled_envelope_id(case, input, &rule_arn);

    // The refused targets: the firing each could not be handed, on its own queue.
    for (refused, machine_arn, queue_url) in &refused_targets {
        let why = refused.why;
        let message = dead_letter(&router, case, why, queue_url).await;
        let body = message["Body"]
            .as_str()
            .unwrap_or_else(|| panic!("case {case:?} {why}: expected a Body, got {message}"));
        // The dead letter carries what the target would have been handed: the same firing's
        // envelope, which the granted target was started with.
        let dead_lettered = scheduled_envelope_id(case, body, &rule_arn);
        assert_eq!(
            dead_lettered, started_with,
            "case {case:?} {why}: expected the dead letter to carry the firing the granted \
             target was started with, got {message}"
        );
        let attributes = &message["MessageAttributes"];
        assert_eq!(
            attributes["RULE_ARN"]["StringValue"],
            json!(rule_arn),
            "case {case:?} {why}: expected the dead letter to name the rule, got {message}"
        );
        assert_eq!(
            attributes["TARGET_ARN"]["StringValue"],
            json!(machine_arn),
            "case {case:?} {why}: expected the dead letter to name the target, got {message}"
        );
        assert_eq!(
            attributes["ERROR_CODE"]["StringValue"],
            json!(refused.code),
            "case {case:?} {why}: expected the step that failed to be {}, got {message}",
            refused.code
        );
        // And the call was NOT made: the machine the refused target names holds nothing.
        let held = executions(&router, machine_arn).await;
        assert!(
            held.is_empty(),
            "case {case:?}: expected no execution of {machine_arn} {why}, got {held:?}"
        );
    }

    // The granted target's own queue: a delivery that was made dead-letters nothing.
    let granted_dead_letters = received(&router, &granted_queue_url).await;
    assert!(
        granted_dead_letters.is_empty(),
        "case {case:?}: expected the granted target's delivery to dead-letter nothing, got \
         {granted_dead_letters:?}"
    );
}
