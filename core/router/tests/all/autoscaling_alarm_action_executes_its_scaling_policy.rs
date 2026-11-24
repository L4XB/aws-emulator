// SPDX-License-Identifier: CC0-1.0

//! A CloudWatch alarm whose action names an Auto Scaling policy moves the group's capacity —
//! over the WHOLE mounted service set, the one `core/router/src/lib.rs` assembles for the
//! binary.
//!
//! The RULE under test: **an alarm transition executes every action configured for the state
//! it moved to; a scaling-policy action is executed by handing the transition to the scaling
//! service that owns the policy, and CloudWatch records the execution as an `Action` history
//! item naming that policy. Receiving the hand-over, Amazon EC2 Auto Scaling performs the
//! policy: the policy's `AdjustmentType` and `ScalingAdjustment` decide the `DesiredCapacity`
//! the group is moved to, held inside the group's own `MinSize`..`MaxSize` and leaving those
//! two alone, and the scaling activity it records names the alarm and the policy. It performs
//! none of it for a policy whose `Enabled` is false, nor for a group that has suspended
//! `AlarmNotification` — the process that receives those notifications — and it waits for the
//! cooldown the group's last change left behind.**
//!
//! Two services have to agree for that to hold: CloudWatch owns the alarm, its action list and
//! the history the execution is recorded in, and Auto Scaling owns the policy, the group and
//! the capacity. Amazon EC2 holds the launch template the group is built on. The adjustment
//! arithmetic itself is pinned over its own tables by
//! `services/service-com-amazonaws-autoscaling-autoscaling-2011-01-01/tests/all/execute_policy.rs`,
//! and the action dispatch by
//! `services/service-com-amazonaws-cloudwatch-graniteserviceversion20100801/tests/all/`; what
//! only this suite reaches is the hand-off between the two over the production service set.
//!
//! The rows differ along what the rule generalises over: the adjustment type, the sign and
//! magnitude of the adjustment, which of the three action lists carries the policy (and so
//! which state fires it), whether the result is held at a bound, whether the policy is
//! enabled, and whether the group receives alarm notifications at all.

use std::time::Duration;

use bytes::Bytes;
use serde_json::{json, Value};
use smithy_server_runtime::MultiServiceRouter;

use crate::cloudwatch_alarm_action_reaches_its_subscribed_queue::{authorization, cw, router};
use crate::query_wire::{element, elements, form_encode, opt_element};

/// The awsQuery API version an Auto Scaling client sends, and the ec2Query one Amazon EC2
/// expects.
const AUTOSCALING_VERSION: &str = "2011-01-01";
const EC2_VERSION: &str = "2016-11-15";

/// The Region the whole wiring of a row lives in: an alarm's actions are executed in the
/// Region the alarm lives in, so the alarm, the policy and the group share one.
const REGION: &str = "us-west-2";

/// How often a poll looks again, and how many times, before what it waits for is called
/// undelivered.
///
/// A BACKSTOP, not a schedule: every wait below polls for the CONDITION it needs — an action
/// CloudWatch has recorded, a capacity the group reports — so a loaded machine delays a row
/// instead of breaking it.
const LOOK_AGAIN: Duration = Duration::from_millis(25);
const LOOKS: usize = 400;

/// One form-wire call against the service whose signing name is `signing_name`, answered as
/// `(status, body)`.
async fn form_call(
    router: &MultiServiceRouter,
    signing_name: &str,
    version: &str,
    action: &str,
    params: &[(&str, &str)],
) -> (u16, String) {
    let mut body = format!("Action={action}&Version={version}");
    for (key, value) in params {
        body.push('&');
        body.push_str(&form_encode(key));
        body.push('=');
        body.push_str(&form_encode(value));
    }
    let request = http::Request::builder()
        .method("POST")
        .uri("/")
        .header("host", format!("{signing_name}.{REGION}.amazonaws.com"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", authorization(signing_name, REGION))
        .body(Bytes::from(body))
        .expect("the test request is well formed");
    let response = router.dispatch(request).await;
    let status = response.status().as_u16();
    let text = String::from_utf8_lossy(response.body()).to_string();
    (status, text)
}

/// One Auto Scaling call that must answer 200.
async fn asg(
    router: &MultiServiceRouter,
    case: &str,
    action: &str,
    params: &[(&str, &str)],
) -> String {
    let (status, body) =
        form_call(router, "autoscaling", AUTOSCALING_VERSION, action, params).await;
    assert_eq!(
        status, 200,
        "case {case:?}: expected HTTP 200 from autoscaling {action}, got {status} with {body}"
    );
    body
}

/// The owned parameter pairs of a request, as the borrowed pairs the wire helper takes.
fn borrowed(params: &[(String, String)]) -> Vec<(&str, &str)> {
    params
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

/// One parameter pair.
fn p(key: &str, value: impl Into<String>) -> (String, String) {
    (key.to_string(), value.into())
}

/// Create the launch template the group is built on — Amazon EC2's own resource — and the
/// group itself, with the processes `suspended` names suspended.
///
/// Most rows suspend `Launch`, so the capacity they move through is recorded without
/// provisioning anything; the group is created at the bottom of its range and brought to
/// `initial` after that suspension, and a `MinSize` the row asks for is raised onto the group
/// afterwards — which is why a group whose range does not start at zero is created at zero and
/// updated. `default_cooldown` is what the group's own changes leave behind.
async fn group_with(
    router: &MultiServiceRouter,
    case: &str,
    name: &str,
    bounds: (i32, i32),
    initial: i32,
    default_cooldown: i32,
    suspended: &[&str],
) {
    let template = format!("{name}-template");
    let (status, created) = form_call(
        router,
        "ec2",
        EC2_VERSION,
        "CreateLaunchTemplate",
        &[
            ("LaunchTemplateName", template.as_str()),
            ("LaunchTemplateData.ImageId", "ami-0123456789abcdef0"),
            ("LaunchTemplateData.InstanceType", "t3.micro"),
        ],
    )
    .await;
    assert_eq!(
        status, 200,
        "case {case:?}: Amazon EC2 must create the launch template, got {status} with {created}"
    );
    assert!(
        opt_element(&created, "launchTemplateId").is_some(),
        "case {case:?}: Amazon EC2 must report the template it created, got {created}"
    );

    let (min_size, max_size) = bounds;
    let create = vec![
        p("AutoScalingGroupName", name),
        p("LaunchTemplate.LaunchTemplateName", template.clone()),
        p("MinSize", "0"),
        p("MaxSize", max_size.to_string()),
        p("DesiredCapacity", "0"),
        p("DefaultCooldown", default_cooldown.to_string()),
        p("AvailabilityZones.member.1", format!("{REGION}a")),
    ];
    asg(router, case, "CreateAutoScalingGroup", &borrowed(&create)).await;

    // A request naming no process suspends EVERY process of the catalog, so a row that
    // suspends none makes no such call at all.
    if !suspended.is_empty() {
        let mut suspend = vec![p("AutoScalingGroupName", name)];
        for (index, process) in suspended.iter().enumerate() {
            suspend.push(p(
                &format!("ScalingProcesses.member.{}", index + 1),
                (*process).to_string(),
            ));
        }
        asg(router, case, "SuspendProcesses", &borrowed(&suspend)).await;
    }

    // The capacity the row starts from, recorded with the group's own capacity writer.
    if initial != 0 {
        let set = vec![
            p("AutoScalingGroupName", name),
            p("DesiredCapacity", initial.to_string()),
        ];
        asg(router, case, "SetDesiredCapacity", &borrowed(&set)).await;
    }
    // The floor the row asks for, raised onto the group now that it holds a capacity at or
    // above it.
    if min_size != 0 {
        let update = vec![
            p("AutoScalingGroupName", name),
            p("MinSize", min_size.to_string()),
        ];
        asg(
            router,
            case,
            "UpdateAutoScalingGroup",
            &borrowed(&update),
        )
        .await;
    }
    let reported = desired_capacity(router, case, name).await;
    assert_eq!(
        reported, initial,
        "case {case:?}: the group must start at {initial}, got {reported}"
    );
}

/// Put one simple scaling policy on `group` and answer the `PolicyARN` the service minted.
async fn put_policy(
    router: &MultiServiceRouter,
    case: &str,
    group: &str,
    policy: &str,
    adjustment_type: &str,
    scaling_adjustment: i32,
    members: &[(String, String)],
) -> String {
    let mut params = vec![
        p("AutoScalingGroupName", group),
        p("PolicyName", policy),
        p("PolicyType", "SimpleScaling"),
        p("AdjustmentType", adjustment_type),
        p("ScalingAdjustment", scaling_adjustment.to_string()),
    ];
    params.extend(members.iter().cloned());
    let body = asg(router, case, "PutScalingPolicy", &borrowed(&params)).await;
    let arn = element(&body, "PolicyARN");
    assert!(
        arn.contains(":scalingPolicy:"),
        "case {case:?}: PutScalingPolicy must report the policy's ARN, got {body}"
    );
    arn
}

/// The `DesiredCapacity` the group reports right now.
async fn desired_capacity(router: &MultiServiceRouter, case: &str, group: &str) -> i32 {
    let body = asg(
        router,
        case,
        "DescribeAutoScalingGroups",
        &[("AutoScalingGroupNames.member.1", group)],
    )
    .await;
    element(&body, "DesiredCapacity")
        .parse()
        .unwrap_or_else(|_| panic!("case {case:?}: DesiredCapacity must be a number, got {body}"))
}

/// The `MinSize` and `MaxSize` the group reports right now, which a policy never changes.
async fn bounds(router: &MultiServiceRouter, case: &str, group: &str) -> (String, String) {
    let body = asg(
        router,
        case,
        "DescribeAutoScalingGroups",
        &[("AutoScalingGroupNames.member.1", group)],
    )
    .await;
    (element(&body, "MinSize"), element(&body, "MaxSize"))
}

/// The causes of the scaling activities the group has recorded, newest first.
async fn activity_causes(router: &MultiServiceRouter, case: &str, group: &str) -> Vec<String> {
    let body = asg(
        router,
        case,
        "DescribeScalingActivities",
        &[("AutoScalingGroupName", group)],
    )
    .await;
    elements(&body, "Cause")
}

/// The instance ids the group reports as its members right now.
async fn instance_ids(router: &MultiServiceRouter, case: &str, group: &str) -> Vec<String> {
    let body = asg(
        router,
        case,
        "DescribeAutoScalingGroups",
        &[("AutoScalingGroupNames.member.1", group)],
    )
    .await;
    elements(&body, "InstanceId")
}

/// Create the alarm `name` with `member` (one of the three action lists) naming `action`.
async fn put_alarm(router: &MultiServiceRouter, case: &str, name: &str, member: &str, action: &str) {
    cw(
        router,
        case,
        REGION,
        "PutMetricAlarm",
        &json!({
            "AlarmName": name,
            "Namespace": "Custom/Scaling",
            "MetricName": "Backlog",
            "Statistic": "Average",
            "Period": 60,
            "EvaluationPeriods": 1,
            "Threshold": 10.0,
            "ComparisonOperator": "GreaterThanThreshold",
            "ActionsEnabled": true,
            member: [action],
        }),
    )
    .await;
}

/// Move the alarm `name` into `state`, which is the transition that executes the actions
/// configured for that state: "When the updated state differs from the previous value, the
/// action configured for the appropriate state is invoked" (`SetAlarmState`).
async fn force_state(router: &MultiServiceRouter, case: &str, name: &str, state: &str) {
    cw(
        router,
        case,
        REGION,
        "SetAlarmState",
        &json!({
            "AlarmName": name,
            "StateValue": state,
            "StateReason": "driven by this suite",
        }),
    )
    .await;
}

/// The `Action` history items CloudWatch has recorded for the alarm `name`, as
/// `(HistorySummary, HistoryData)` pairs.
async fn action_history(
    router: &MultiServiceRouter,
    case: &str,
    name: &str,
) -> Vec<(String, String)> {
    let answer = cw(
        router,
        case,
        REGION,
        "DescribeAlarmHistory",
        &json!({"AlarmName": name, "HistoryItemType": "Action"}),
    )
    .await;
    answer
        .get("AlarmHistoryItems")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("HistoryItemType").and_then(Value::as_str) == Some("Action")
                })
                .map(|item| {
                    (
                        item.get("HistorySummary")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        item.get("HistoryData")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Wait until CloudWatch has recorded at least `wanted` `Action` items naming `action` for the
/// alarm `name`, and answer them.
///
/// The hand-over to the scaling service is queued by the same pass that records the item, so
/// an item a reader can see means the transition has been handed over: every assertion about
/// what Auto Scaling then did is made after this.
async fn recorded_actions(
    router: &MultiServiceRouter,
    case: &str,
    alarm: &str,
    action: &str,
    wanted: usize,
) -> Vec<(String, String)> {
    for _ in 0..LOOKS {
        let items: Vec<(String, String)> = action_history(router, case, alarm)
            .await
            .into_iter()
            .filter(|(summary, data)| summary.contains(action) || data.contains(action))
            .collect();
        if items.len() >= wanted {
            return items;
        }
        tokio::time::sleep(LOOK_AGAIN).await;
    }
    panic!(
        "case {case:?}: CloudWatch recorded no {wanted} Action items naming {action}; it holds \
         {:?}",
        action_history(router, case, alarm).await
    )
}

/// Wait until the group reports `wanted` as its `DesiredCapacity`, and answer what it reports.
async fn settled_capacity(
    router: &MultiServiceRouter,
    case: &str,
    group: &str,
    wanted: i32,
) -> i32 {
    let mut reported = desired_capacity(router, case, group).await;
    for _ in 0..LOOKS {
        if reported == wanted {
            return reported;
        }
        tokio::time::sleep(LOOK_AGAIN).await;
        reported = desired_capacity(router, case, group).await;
    }
    reported
}

/// Require that each item of `items` records a SUCCESSFUL execution of `action` for `state`.
fn assert_action_recorded(case: &str, items: &[(String, String)], action: &str, state: &str) {
    assert!(
        !items.is_empty(),
        "case {case:?}: CloudWatch must record the action it executed"
    );
    for (summary, data) in items {
        assert!(
            summary.contains(action),
            "case {case:?}: the item's summary must name the policy it executed, got {summary:?}"
        );
        let document: Value = serde_json::from_str(data)
            .unwrap_or_else(|_| panic!("case {case:?}: HistoryData must be a document, got {data}"));
        assert_eq!(
            document.get("actionState").and_then(Value::as_str),
            Some("Succeeded"),
            "case {case:?}: the hand-over was accepted, so the item records it as succeeded, \
             got {data}"
        );
        assert_eq!(
            document.get("notificationResource").and_then(Value::as_str),
            Some(action),
            "case {case:?}: the item must name the action it executed, got {data}"
        );
        assert_eq!(
            document.get("stateValue").and_then(Value::as_str),
            Some(state),
            "case {case:?}: the item must name the state that fired the action, got {data}"
        );
    }
}

/// One row of the table: what the policy adjusts, which state fires it, and where the group
/// ends up.
struct Row<'a> {
    /// The case every assertion names.
    case: &'a str,
    /// `AdjustmentType` and `ScalingAdjustment` of the policy.
    adjustment_type: &'a str,
    scaling_adjustment: i32,
    /// The group's `MinSize` and `MaxSize`, which the policy may not change.
    bounds: (i32, i32),
    /// The `DesiredCapacity` the group starts at.
    initial: i32,
    /// The action list the policy sits in, and the state the alarm is moved to.
    member: &'a str,
    state: &'a str,
    /// The state the alarm is put through first, for a row whose target state is the one a
    /// fresh alarm already holds — an action is executed by a CHANGE of state.
    prior: Option<&'a str>,
    /// Whether the policy is `Enabled`.
    enabled: bool,
    /// Whether the group has suspended `AlarmNotification`.
    suspends_notifications: bool,
    /// The `DesiredCapacity` the group holds once the alarm has fired.
    expected: i32,
}

#[tokio::test]
async fn an_alarm_action_naming_a_scaling_policy_moves_the_groups_capacity() {
    let rows: &[Row<'_>] = &[
        Row {
            case: "an ExactCapacity policy sets the capacity it names",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 2,
            bounds: (0, 4),
            initial: 0,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 2,
        },
        Row {
            case: "a ChangeInCapacity policy adds its adjustment",
            adjustment_type: "ChangeInCapacity",
            scaling_adjustment: 2,
            bounds: (0, 4),
            initial: 1,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 3,
        },
        Row {
            case: "a PercentChangeInCapacity policy adds its percentage",
            adjustment_type: "PercentChangeInCapacity",
            scaling_adjustment: 100,
            bounds: (0, 4),
            initial: 2,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 4,
        },
        Row {
            case: "a policy in OKActions scales in when the alarm recovers",
            adjustment_type: "ChangeInCapacity",
            scaling_adjustment: -1,
            bounds: (0, 4),
            initial: 2,
            member: "OKActions",
            state: "OK",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 1,
        },
        Row {
            case: "a policy in InsufficientDataActions is fired by that state too",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 3,
            bounds: (0, 4),
            initial: 0,
            member: "InsufficientDataActions",
            state: "INSUFFICIENT_DATA",
            // A fresh alarm already reports INSUFFICIENT_DATA, so it is moved out of that
            // state first for the move back into it to be a transition at all.
            prior: Some("OK"),
            enabled: true,
            suspends_notifications: false,
            expected: 3,
        },
        Row {
            case: "a capacity above MaxSize is held at the bound",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 9,
            bounds: (0, 3),
            initial: 0,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 3,
        },
        Row {
            case: "a capacity below MinSize is held at the bound",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 0,
            bounds: (2, 4),
            initial: 3,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: false,
            expected: 2,
        },
        Row {
            case: "a group that has suspended AlarmNotification declines the hand-over",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 2,
            bounds: (0, 4),
            initial: 0,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: true,
            suspends_notifications: true,
            expected: 0,
        },
        Row {
            case: "a disabled policy is not performed",
            adjustment_type: "ExactCapacity",
            scaling_adjustment: 2,
            bounds: (0, 4),
            initial: 0,
            member: "AlarmActions",
            state: "ALARM",
            prior: None,
            enabled: false,
            suspends_notifications: false,
            expected: 0,
        },
    ];

    for (index, row) in rows.iter().enumerate() {
        let router = router();
        let case = row.case;
        let group = format!("alarm-driven-group-{index}");
        let policy_name = format!("alarm-driven-policy-{index}");
        let alarm = format!("alarm-driven-alarm-{index}");
        let suspended: &[&str] = if row.suspends_notifications {
            &["Launch", "AlarmNotification"]
        } else {
            &["Launch"]
        };

        group_with(
            &router,
            case,
            &group,
            row.bounds,
            row.initial,
            0,
            suspended,
        )
        .await;
        let mut members = vec![p("Cooldown", "0")];
        if !row.enabled {
            members.push(p("Enabled", "false"));
        }
        let policy = put_policy(
            &router,
            case,
            &group,
            &policy_name,
            row.adjustment_type,
            row.scaling_adjustment,
            &members,
        )
        .await;
        put_alarm(&router, case, &alarm, row.member, &policy).await;

        if let Some(prior) = row.prior {
            force_state(&router, case, &alarm, prior).await;
        }
        force_state(&router, case, &alarm, row.state).await;

        // Cause 2 of the report: CloudWatch records the action it executed, whatever Auto
        // Scaling then decides — so this holds for the declined rows as well.
        let items = recorded_actions(&router, case, &alarm, &policy, 1).await;
        assert_action_recorded(case, &items, &policy, row.state);

        // Cause 1: the policy the action named is performed, and the capacity it asks for is
        // what the group reports.
        let reported = settled_capacity(&router, case, &group, row.expected).await;
        assert_eq!(
            reported, row.expected,
            "case {case:?}: expected DesiredCapacity {}, got {reported}",
            row.expected
        );

        // The bounds are the group's own: a policy moves the capacity between them and never
        // moves them.
        let (min_size, max_size) = bounds(&router, case, &group).await;
        assert_eq!(
            (min_size.as_str(), max_size.as_str()),
            (
                row.bounds.0.to_string().as_str(),
                row.bounds.1.to_string().as_str()
            ),
            "case {case:?}: MinSize/MaxSize must be left alone, got {min_size}/{max_size}"
        );

        // A row the receiving service DECLINES performed nothing at all: beside the capacity
        // it left alone, the group records no scaling activity naming the policy — which a
        // group whose capacity moved and was put back would.
        if row.expected == row.initial {
            let causes = activity_causes(&router, case, &group).await;
            assert!(
                !causes.iter().any(|cause| cause.contains(&policy_name)),
                "case {case:?}: a declined hand-over performs nothing, so no activity names \
                 the policy, got {causes:?}"
            );
        }

        // No row of this table provisions an instance (`Launch` is suspended for every one),
        // and a scaling activity is what a launch or a termination records: the activity an
        // alarm-driven change records is pinned by
        // `an_alarm_driven_policy_launches_the_capacity_it_asks_for` below, over a group that
        // does provision.
        let instances = instance_ids(&router, case, &group).await;
        assert!(
            instances.is_empty(),
            "case {case:?}: the group has Launch suspended, so it holds no instance, got \
             {instances:?}"
        );
    }
}

#[tokio::test]
async fn an_alarm_driven_policy_launches_the_capacity_it_asks_for() {
    let case = "an alarm-driven scale-out of a group that provisions";
    let router = router();
    let group = "launching-group";
    let policy_name = "launching-policy";
    let alarm = "launching-alarm";

    // Nothing is suspended here, so the capacity the policy asks for is converged on: the
    // group launches the account's own Amazon EC2 instance for it and records the activity.
    group_with(&router, case, group, (0, 2), 0, 0, &[]).await;
    let policy = put_policy(
        &router,
        case,
        group,
        policy_name,
        "ChangeInCapacity",
        1,
        &[p("Cooldown", "0")],
    )
    .await;
    put_alarm(&router, case, alarm, "AlarmActions", &policy).await;
    force_state(&router, case, alarm, "ALARM").await;

    let items = recorded_actions(&router, case, alarm, &policy, 1).await;
    assert_action_recorded(case, &items, &policy, "ALARM");
    let reported = settled_capacity(&router, case, group, 1).await;
    assert_eq!(
        reported, 1,
        "case {case:?}: the alarm-driven change moves the capacity to 1, got {reported}"
    );

    let instances = instance_ids(&router, case, group).await;
    assert_eq!(
        instances.len(),
        1,
        "case {case:?}: the group launches the capacity it was moved to, got {instances:?}"
    );
    let causes = activity_causes(&router, case, group).await;
    assert!(
        causes.iter().any(|cause| {
            cause.contains(policy_name) && cause.contains(alarm) && cause.contains("ALARM")
        }),
        "case {case:?}: an activity must name the alarm, its state and the policy, got \
         {causes:?}"
    );
}

#[tokio::test]
async fn one_alarm_drives_every_group_whose_policy_it_names_except_a_suspended_one() {
    let case = "two groups, one alarm, AlarmNotification suspended on one of them";
    let router = router();
    let alarm = "shared-alarm";

    // Two groups configured identically, differing only in whether they receive alarm
    // notifications, each with its own policy named by the SAME alarm.
    let mut policies = Vec::new();
    for (index, suspended) in [vec!["Launch"], vec!["Launch", "AlarmNotification"]]
        .iter()
        .enumerate()
    {
        let group = format!("shared-alarm-group-{index}");
        group_with(&router, case, &group, (0, 4), 0, 0, suspended).await;
        let policy = put_policy(
            &router,
            case,
            &group,
            &format!("shared-alarm-policy-{index}"),
            "ExactCapacity",
            2,
            &[p("Cooldown", "0")],
        )
        .await;
        policies.push((group, policy));
    }

    cw(
        &router,
        case,
        REGION,
        "PutMetricAlarm",
        &json!({
            "AlarmName": alarm,
            "Namespace": "Custom/Scaling",
            "MetricName": "Backlog",
            "Statistic": "Average",
            "Period": 60,
            "EvaluationPeriods": 1,
            "Threshold": 10.0,
            "ComparisonOperator": "GreaterThanThreshold",
            "AlarmActions": [policies[0].1.clone(), policies[1].1.clone()],
        }),
    )
    .await;
    force_state(&router, case, alarm, "ALARM").await;

    // Both actions are executed and both are recorded: the decision to decline is the
    // receiving service's, not CloudWatch's.
    for (group, policy) in &policies {
        let items = recorded_actions(&router, case, alarm, policy, 1).await;
        assert_action_recorded(
            &format!("{case}: the action naming the policy of {group}"),
            &items,
            policy,
            "ALARM",
        );
    }

    let unsuspended = settled_capacity(&router, case, &policies[0].0, 2).await;
    assert_eq!(
        unsuspended, 2,
        "case {case:?}: the group that receives alarm notifications scales to 2, got \
         {unsuspended}"
    );
    let suspended = desired_capacity(&router, case, &policies[1].0).await;
    assert_eq!(
        suspended, 0,
        "case {case:?}: the group that has suspended AlarmNotification stays at 0, got \
         {suspended}"
    );
}

#[tokio::test]
async fn an_alarm_driven_policy_waits_for_the_cooldown_its_last_change_left() {
    let case = "an alarm firing twice inside the cooldown of its first change";
    let router = router();
    let group = "cooldown-group";
    let policy_name = "cooldown-policy";
    let alarm = "cooldown-alarm";

    // The group leaves no cooldown of its own behind (`DefaultCooldown` 0), and the policy
    // carries one: "When a cooldown period is specified here, it overrides the default
    // cooldown period defined for the Auto Scaling group"
    // (`PutScalingPolicyType$Cooldown`), so what holds the second firing back is the
    // cooldown the policy's first change left.
    group_with(&router, case, group, (0, 4), 0, 0, &["Launch"]).await;
    let policy = put_policy(
        &router,
        case,
        group,
        policy_name,
        "ChangeInCapacity",
        1,
        &[p("Cooldown", "300")],
    )
    .await;
    put_alarm(&router, case, alarm, "AlarmActions", &policy).await;

    force_state(&router, case, alarm, "ALARM").await;
    recorded_actions(&router, case, alarm, &policy, 1).await;
    let after_first = settled_capacity(&router, case, group, 1).await;
    assert_eq!(
        after_first, 1,
        "case {case:?}: the first firing adds the adjustment, got {after_first}"
    );

    // The alarm recovers and breaches again: the second hand-over is executed and recorded,
    // and the group is still inside the cooldown the first change started, so its capacity
    // does not move again.
    force_state(&router, case, alarm, "OK").await;
    force_state(&router, case, alarm, "ALARM").await;
    recorded_actions(&router, case, alarm, &policy, 2).await;
    let after_second = desired_capacity(&router, case, group).await;
    assert_eq!(
        after_second, 1,
        "case {case:?}: the cooldown holds the second firing back, got {after_second}"
    );
}

#[tokio::test]
async fn two_alarms_route_to_their_own_policy_generation_and_group() {
    let router = router();
    let case = "independent simple and step alarms with the same policy name";
    let groups = ["simple-group", "step-group"];
    let alarms = ["simple-alarm", "step-alarm"];
    let policy_name = "scale-out";
    for group in groups {
        group_with(&router, case, group, (0, 3), 0, 0, &[]).await;
    }
    let simple = put_policy(
        &router,
        case,
        groups[0],
        policy_name,
        "ChangeInCapacity",
        1,
        &[p("Cooldown", "0")],
    )
    .await;
    let step = asg(
        &router,
        case,
        "PutScalingPolicy",
        &[
            ("AutoScalingGroupName", groups[1]),
            ("PolicyName", policy_name),
            ("PolicyType", "StepScaling"),
            ("AdjustmentType", "ChangeInCapacity"),
            ("EstimatedInstanceWarmup", "0"),
            ("StepAdjustments.member.1.MetricIntervalLowerBound", "0"),
            ("StepAdjustments.member.1.ScalingAdjustment", "2"),
        ],
    )
    .await;
    let policy_arns = [simple, element(&step, "PolicyARN")];
    assert_ne!(policy_arns[0], policy_arns[1]);
    for (alarm, policy) in alarms.iter().zip(&policy_arns) {
        put_alarm(&router, case, alarm, "AlarmActions", policy).await;
    }
    for i in 0..2 {
        let other = 1 - i;
        let before = instance_ids(&router, case, groups[other]).await;
        let before_causes = activity_causes(&router, case, groups[other]).await;
        cw(&router, case, REGION, "SetAlarmState", &json!({
            "AlarmName":alarms[i], "StateValue":"ALARM", "StateReason":format!("controlled-{i}"),
            "StateReasonData":r#"{"version":"1.0","recentDatapoints":[12.0],"threshold":10.0}"#
        })).await;
        let items = recorded_actions(&router, case, alarms[i], &policy_arns[i], 1).await;
        assert_action_recorded(case, &items, &policy_arns[i], "ALARM");
        assert_eq!(action_history(&router, case, alarms[i]).await.len(), 1);
        let desired = (i + 1) as i32;
        assert_eq!(
            settled_capacity(&router, case, groups[i], desired).await,
            desired
        );
        assert_eq!(
            instance_ids(&router, case, groups[i]).await.len(),
            desired as usize
        );
        assert!(activity_causes(&router, case, groups[i])
            .await
            .iter()
            .any(|cause| cause.contains(alarms[i])
                && cause.contains(policy_name)
                && cause.contains("ALARM")));
        assert_eq!(instance_ids(&router, case, groups[other]).await, before);
        assert_eq!(
            activity_causes(&router, case, groups[other]).await,
            before_causes
        );
        assert_eq!(
            desired_capacity(&router, case, groups[other]).await,
            i as i32
        );
    }
}
