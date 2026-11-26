// SPDX-License-Identifier: CC0-1.0

//! A `forward` to an `instance` or an `ip` target group, end to end over the wire: what a
//! client that sends traffic to the name a load balancer publishes gets back from the
//! workload the REGISTERED TARGET runs.
//!
//! The rule under test: **a `forward` whose chosen target names a resource of this account is
//! PROXIED to the server that resource runs on the port the registration receives traffic on,
//! and the client is answered with that server's own status, media type and bytes — however
//! the registration names the target (an instance id, or an address an interface of that
//! instance holds). A registration with no server behind it on that port is a target the load
//! balancer cannot connect to, which it answers `502` for.**
//!
//! Everything is driven through the two services' own APIs: EC2 is mounted beside this one on
//! the suite's own engine, the network, the security group and the instances are the records
//! EC2's own readers answer with, the instance's web server is the one its own user data
//! installed and started (`service-com-amazonaws-ec2-amazonec2`'s
//! `tests/all/instance_guest_web_server.rs` pins what that program leaves listening), and the
//! load balancer, its listener, its rule and the registrations are written through this
//! service's own operations. The request itself carries no AWS credential, exactly as a client
//! of the published `DNSName` sends it — and the last case is a REAL connection to that
//! published address, so the socket the name is served on is in the path too.
//!
//! The rows differ along what the rule generalises over: how the registration names the
//! target (its instance id, its private address), which document the request's path names
//! (the root document, another document of the same root, one the root holds nothing at), and
//! the registrations nothing stands behind (an instance running no server, and a port its
//! server does not listen on).

use std::time::{Duration, Instant};

use base64::Engine as _;
use smithy_server_runtime::MultiServiceRouter;

use crate::alb_data_plane::send;
use crate::alb_lambda_target_published_address::exchange;
use crate::provisioning_loop::{action, call, first_text, members, p};
use crate::target_security_group_health_check::{ec2_ok, emulator_with_ec2, reported};

/// How long a step waits for a clock-owed move — the instance's to `running`, the load
/// balancer's to `active`, a registration's to `healthy` — and how often it looks. Generous on
/// purpose: the deadline is only here so a failure is a failure rather than a hang.
const DEADLINE: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(25);

/// The health-check cadence every target group below is created with: the shortest
/// `HealthCheckIntervalSeconds` admits ("the range is 5-300"), so a verdict arrives as soon as
/// the schedule can reach one.
const INTERVAL_SECONDS: &str = "5";

/// An AMI id the launches name; nothing here asserts anything about the image.
const AMI: &str = "ami-0123456789abcdef0";

/// The network every row runs in: one VPC and two subnets of it in different zones, which is
/// what an Application Load Balancer is placed across.
const VPC_CIDR: &str = "10.80.0.0/16";
const SUBNETS: [(&str, &str); 2] = [("10.80.1.0/24", "us-west-2a"), ("10.80.2.0/24", "us-west-2b")];

/// The address the serving instance is launched at, which is also how the `ip` target group
/// registers it.
const SERVING_ADDRESS: &str = "10.80.1.10";

/// The port the target groups receive traffic on, which is the port the instance's own server
/// listens on.
const TRAFFIC_PORT: i32 = 80;

/// A port no server of the serving instance listens on.
const UNSERVED_PORT: i32 = 8081;

/// The marker the instance's own user-data program writes into the document it serves.
const MARKER: &str = "from-the-instance-4c1d";

/// The user data of the serving instance: it installs the Apache package of the Amazon Linux
/// family, starts its unit and writes the two documents the rows ask for into the directory
/// that package serves.
fn serving_user_data() -> String {
    format!(
        "#!/bin/bash\n\
         yum update -y\n\
         yum install -y httpd\n\
         systemctl enable httpd\n\
         systemctl start httpd\n\
         echo \"<h1>{MARKER}</h1>\" > /var/www/html/index.html\n\
         echo ok > /var/www/html/health.txt\n"
    )
}

/// The account's network, as the two services read it.
struct Network {
    vpc: String,
    subnets: Vec<String>,
    /// The group the load balancers are associated with, which is the source a target's own
    /// inbound rule names.
    load_balancer_group: String,
    /// The group governing the instances, whose inbound rule admits the load balancer on the
    /// traffic port.
    target_group_security_group: String,
}

/// Create that network through EC2's own API: the VPC with the internet gateway an
/// internet-facing load balancer needs, the two subnets, and the two security groups with the
/// one inbound rule that admits the load balancer to the targets on the traffic port.
async fn seed_network(router: &MultiServiceRouter) -> Network {
    let created = ec2_ok(router, "setup", "CreateVpc", &[p("CidrBlock", VPC_CIDR)]).await;
    let vpc = reported("setup", &created, "vpcId");
    let gateway = ec2_ok(router, "setup", "CreateInternetGateway", &[]).await;
    let gateway = reported("setup", &gateway, "internetGatewayId");
    ec2_ok(
        router,
        "setup",
        "AttachInternetGateway",
        &[p("InternetGatewayId", &gateway), p("VpcId", &vpc)],
    )
    .await;
    let mut subnets: Vec<String> = Vec::new();
    for (cidr, zone) in SUBNETS {
        let created = ec2_ok(
            router,
            "setup",
            "CreateSubnet",
            &[
                p("VpcId", &vpc),
                p("CidrBlock", cidr),
                p("AvailabilityZone", zone),
            ],
        )
        .await;
        subnets.push(reported("setup", &created, "subnetId"));
    }
    let load_balancer_group = security_group(router, &vpc, "itarget-alb").await;
    // The recorded shape: ONE security group, which the load balancer is associated with and
    // which governs the instances, admitting the traffic port from an address RANGE rather
    // than from the load balancer's own group — so a health check of a target is judged
    // against the addresses the load balancer's own interfaces carry.
    let target_group_security_group = load_balancer_group.clone();
    ec2_ok(
        router,
        "setup",
        "AuthorizeSecurityGroupIngress",
        &[
            p("GroupId", &target_group_security_group),
            p("IpPermissions.1.IpProtocol", "tcp"),
            p("IpPermissions.1.FromPort", &TRAFFIC_PORT.to_string()),
            p("IpPermissions.1.ToPort", &TRAFFIC_PORT.to_string()),
            p("IpPermissions.1.IpRanges.1.CidrIp", "0.0.0.0/0"),
        ],
    )
    .await;
    // And on the port no server of the instance listens on, so a row about THAT answers for
    // the server rather than for the firewall: a health check of it arrives and finds the
    // instance running, and the forward is the only thing left to decide the answer.
    ec2_ok(
        router,
        "setup",
        "AuthorizeSecurityGroupIngress",
        &[
            p("GroupId", &target_group_security_group),
            p("IpPermissions.1.IpProtocol", "tcp"),
            p("IpPermissions.1.FromPort", &UNSERVED_PORT.to_string()),
            p("IpPermissions.1.ToPort", &UNSERVED_PORT.to_string()),
            p("IpPermissions.1.IpRanges.1.CidrIp", "0.0.0.0/0"),
        ],
    )
    .await;
    Network {
        vpc,
        subnets,
        load_balancer_group,
        target_group_security_group,
    }
}

/// Create one security group of `vpc` and answer its `GroupId`.
async fn security_group(router: &MultiServiceRouter, vpc: &str, name: &str) -> String {
    let created = ec2_ok(
        router,
        "setup",
        "CreateSecurityGroup",
        &[
            p("VpcId", vpc),
            p("GroupName", name),
            p("GroupDescription", "a group of this suite"),
        ],
    )
    .await;
    reported("setup", &created, "groupId")
}

/// Launch one instance of `network`, with `user_data` as its user data and `address` as its
/// private address, answering its id.
async fn run_instance(
    router: &MultiServiceRouter,
    case: &str,
    network: &Network,
    address: &str,
    user_data: Option<String>,
) -> String {
    let mut params = vec![
        p("ImageId", AMI),
        p("MinCount", "1"),
        p("MaxCount", "1"),
        p("SubnetId", &network.subnets[0]),
        p("SecurityGroupId.1", &network.target_group_security_group),
        p("PrivateIpAddress", address),
    ];
    if let Some(user_data) = user_data {
        params.push(p(
            "UserData",
            &base64::engine::general_purpose::STANDARD.encode(user_data),
        ));
    }
    let launched = ec2_ok(router, case, "RunInstances", &params).await;
    reported(case, &launched, "instanceId")
}

/// Look at the instance until EC2 reports it `running` — the move the launch owes, which is
/// also when its guest comes up and runs its user data.
async fn running(router: &MultiServiceRouter, case: &str, instance: &str) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let described = ec2_ok(
            router,
            case,
            "DescribeInstances",
            &[p("InstanceId.1", instance)],
        )
        .await;
        let is_running = described.contains("<name>running</name>");
        if is_running || Instant::now() >= deadline {
            assert!(
                is_running,
                "case {case:?}: expected EC2 to report {instance} running, got {described}"
            );
            return;
        }
        ::tokio::time::sleep(POLL).await;
    }
}

/// Create a target group of `target_type` in the network's VPC and answer its ARN.
async fn target_group(
    router: &MultiServiceRouter,
    case: &str,
    name: &str,
    target_type: &str,
    network: &Network,
) -> String {
    let mut form = action("CreateTargetGroup");
    form.push(p("Name", name));
    form.push(p("Protocol", "HTTP"));
    form.push(p("Port", &TRAFFIC_PORT.to_string()));
    form.push(p("TargetType", target_type));
    form.push(p("VpcId", &network.vpc));
    form.push(p("HealthCheckProtocol", "HTTP"));
    form.push(p("HealthCheckPath", "/"));
    form.push(p("HealthCheckIntervalSeconds", INTERVAL_SECONDS));
    form.push(p("HealthyThresholdCount", "2"));
    let reply = call(router, &form).await;
    assert_eq!(
        reply.status, 200,
        "case {case:?}: expected 200 from CreateTargetGroup, got {} body {}",
        reply.status, reply.body
    );
    reply
        .text("TargetGroupArn")
        .unwrap_or_else(|| panic!("case {case:?}: no TargetGroupArn in {}", reply.body))
}

/// Register `id` on `port` with the target group `arn` names.
async fn register(router: &MultiServiceRouter, case: &str, arn: &str, id: &str, port: i32) {
    let mut form = action("RegisterTargets");
    form.push(p("TargetGroupArn", arn));
    form.push(p("Targets.member.1.Id", id));
    form.push(p("Targets.member.1.Port", &port.to_string()));
    let reply = call(router, &form).await;
    assert_eq!(
        reply.status, 200,
        "case {case:?}: expected 200 from RegisterTargets, got {} body {}",
        reply.status, reply.body
    );
}

/// Create an Application Load Balancer across the network's subnets, in the network's own
/// security group, whose HTTP:80 listener forwards 100% of its weight to `tg_arn` — and, where
/// `with_rule` is set, a priority-10 rule whose `path-pattern` condition matches every path and
/// which forwards to the same group. Answers the ARN and the published name.
async fn load_balancer(
    router: &MultiServiceRouter,
    case: &str,
    name: &str,
    network: &Network,
    tg_arn: &str,
    with_rule: bool,
) -> (String, String) {
    let mut form = action("CreateLoadBalancer");
    form.push(p("Name", name));
    for (index, subnet) in network.subnets.iter().enumerate() {
        form.push(p(&format!("Subnets.member.{}", index + 1), subnet));
    }
    form.push(p("SecurityGroups.member.1", &network.load_balancer_group));
    let reply = call(router, &form).await;
    assert_eq!(
        reply.status, 200,
        "case {case:?}: expected 200 from CreateLoadBalancer, got {} body {}",
        reply.status, reply.body
    );
    let arn = reply
        .text("LoadBalancerArn")
        .unwrap_or_else(|| panic!("case {case:?}: no LoadBalancerArn in {}", reply.body));
    let dns_name = reply
        .text("DNSName")
        .unwrap_or_else(|| panic!("case {case:?}: no DNSName in {}", reply.body));
    let mut form = action("CreateListener");
    form.push(p("LoadBalancerArn", &arn));
    form.push(p("Protocol", "HTTP"));
    form.push(p("Port", &TRAFFIC_PORT.to_string()));
    form.push(p("DefaultActions.member.1.Type", "forward"));
    form.push(p(
        "DefaultActions.member.1.ForwardConfig.TargetGroups.member.1.TargetGroupArn",
        tg_arn,
    ));
    form.push(p(
        "DefaultActions.member.1.ForwardConfig.TargetGroups.member.1.Weight",
        "100",
    ));
    let reply = call(router, &form).await;
    assert_eq!(
        reply.status, 200,
        "case {case:?}: expected 200 from CreateListener, got {} body {}",
        reply.status, reply.body
    );
    let listener_arn = reply
        .text("ListenerArn")
        .unwrap_or_else(|| panic!("case {case:?}: no ListenerArn in {}", reply.body));
    if with_rule {
        let mut form = action("CreateRule");
        form.push(p("ListenerArn", &listener_arn));
        form.push(p("Priority", "10"));
        form.push(p("Conditions.member.1.Field", "path-pattern"));
        form.push(p("Conditions.member.1.Values.member.1", "/*"));
        form.push(p("Actions.member.1.Type", "forward"));
        form.push(p(
            "Actions.member.1.ForwardConfig.TargetGroups.member.1.TargetGroupArn",
            tg_arn,
        ));
        form.push(p(
            "Actions.member.1.ForwardConfig.TargetGroups.member.1.Weight",
            "100",
        ));
        let reply = call(router, &form).await;
        assert_eq!(
            reply.status, 200,
            "case {case:?}: expected 200 from CreateRule, got {} body {}",
            reply.status, reply.body
        );
        // The rule is readable the way the client that wrote it reads it back.
        let mut form = action("DescribeRules");
        form.push(p("ListenerArn", &listener_arn));
        let reply = call(router, &form).await;
        let rules = members(&reply.body, "Rules");
        assert!(
            rules
                .iter()
                .any(|rule| first_text(rule, "Priority").as_deref() == Some("10")),
            "case {case:?}: expected DescribeRules to report the priority-10 rule, got {:?}",
            rules
        );
    }
    (arn, dns_name)
}

/// Look at the load balancer until it reports `State.Code` `active`.
async fn active(router: &MultiServiceRouter, case: &str, arn: &str) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let mut form = action("DescribeLoadBalancers");
        form.push(p("LoadBalancerArns.member.1", arn));
        let reply = call(router, &form).await;
        let code = first_text(&reply.body, "Code").unwrap_or_default();
        if code == "active" || Instant::now() >= deadline {
            assert_eq!(
                code, "active",
                "case {case:?}: expected the load balancer to reach active, got {code:?} in {}",
                reply.body
            );
            return;
        }
        ::tokio::time::sleep(POLL).await;
    }
}

/// Look at the target group until every registration it holds reports `wanted`.
async fn health_reaching(router: &MultiServiceRouter, case: &str, arn: &str, wanted: &str) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let mut form = action("DescribeTargetHealth");
        form.push(p("TargetGroupArn", arn));
        let reply = call(router, &form).await;
        let states: Vec<String> = members(&reply.body, "TargetHealthDescriptions")
            .iter()
            .map(|description| first_text(description, "State").unwrap_or_default())
            .collect();
        let settled = !states.is_empty() && states.iter().all(|state| state == wanted);
        if settled || Instant::now() >= deadline {
            assert!(
                settled,
                "case {case:?}: expected every registration to report {wanted}, got {states:?}"
            );
            return;
        }
        ::tokio::time::sleep(POLL).await;
    }
}

/// One deployment: the router, and the published name of each load balancer a row addresses.
struct Deployment {
    router: MultiServiceRouter,
    /// The load balancer forwarding to the `instance` target group of the serving instance.
    by_instance: String,
    /// The one forwarding to the `ip` target group that registers the same instance by its
    /// address.
    by_address: String,
    /// The one forwarding to an instance that runs no server.
    no_server: String,
    /// The one forwarding to the serving instance on a port it does not listen on.
    unserved_port: String,
}

/// Everything the rows are answered out of, written through the two services' own APIs.
async fn deployment() -> Deployment {
    let router = emulator_with_ec2();
    let network = seed_network(&router).await;
    let serving = run_instance(
        &router,
        "the serving instance",
        &network,
        SERVING_ADDRESS,
        Some(serving_user_data()),
    )
    .await;
    let bare = run_instance(&router, "an instance with no server", &network, "10.80.1.11", None).await;
    running(&router, "the serving instance", &serving).await;
    running(&router, "an instance with no server", &bare).await;

    let by_instance_tg = target_group(&router, "by instance id", "itarget-inst", "instance", &network).await;
    register(&router, "by instance id", &by_instance_tg, &serving, TRAFFIC_PORT).await;
    let by_address_tg = target_group(&router, "by address", "itarget-ip", "ip", &network).await;
    register(&router, "by address", &by_address_tg, SERVING_ADDRESS, TRAFFIC_PORT).await;
    let no_server_tg = target_group(&router, "no server", "itarget-bare", "instance", &network).await;
    register(&router, "no server", &no_server_tg, &bare, TRAFFIC_PORT).await;
    let unserved_tg = target_group(&router, "unserved port", "itarget-port", "instance", &network).await;
    register(&router, "unserved port", &unserved_tg, &serving, UNSERVED_PORT).await;

    // Every load balancer is created before anything waits, so the span each owes its move to
    // `active` runs at the same time.
    let (by_instance_arn, by_instance) = load_balancer(
        &router,
        "by instance id",
        "itarget-inst-lb",
        &network,
        &by_instance_tg,
        // The issue's own shape: a priority-10 path-pattern rule forwarding to the same group.
        true,
    )
    .await;
    let (by_address_arn, by_address) =
        load_balancer(&router, "by address", "itarget-ip-lb", &network, &by_address_tg, false).await;
    let (no_server_arn, no_server) = load_balancer(
        &router,
        "no server",
        "itarget-bare-lb",
        &network,
        &no_server_tg,
        false,
    )
    .await;
    let (unserved_arn, unserved_port) = load_balancer(
        &router,
        "unserved port",
        "itarget-port-lb",
        &network,
        &unserved_tg,
        false,
    )
    .await;
    for (case, arn) in [
        ("by instance id", &by_instance_arn),
        ("by address", &by_address_arn),
        ("no server", &no_server_arn),
        ("unserved port", &unserved_arn),
    ] {
        active(&router, case, arn).await;
    }
    // Every registration the rows read is healthy before a request is sent, which is the
    // signal a client waits for.
    for (case, arn) in [
        ("by instance id", &by_instance_tg),
        ("by address", &by_address_tg),
        ("no server", &no_server_tg),
        ("unserved port", &unserved_tg),
    ] {
        health_reaching(&router, case, arn, "healthy").await;
    }
    Deployment {
        router,
        by_instance,
        by_address,
        no_server,
        unserved_port,
    }
}

/// One request a row makes: which load balancer it addresses, the path it asks for, and what
/// the load balancer must answer — the status, the body (up to the line ending the guest's own
/// `echo` wrote) and the media type.
struct Asked<'a> {
    case: &'a str,
    dns_name: &'a str,
    path: &'a str,
    status: u16,
    body: &'a str,
    content_type: Option<&'a str>,
}

/// RULE: a `forward` whose chosen target names a resource of this account is proxied to the
/// server that resource runs on the registered port, and the client is answered with that
/// server's own status, media type and bytes; a registration with no server behind it on that
/// port is answered `502`.
#[tokio::test]
async fn a_forward_to_an_instance_target_is_answered_by_the_server_that_instance_runs() {
    let deployed = deployment().await;
    let root_document = format!("<h1>{MARKER}</h1>");
    let rows: Vec<Asked<'_>> = vec![
        Asked {
            case: "the instance target's root document",
            dns_name: deployed.by_instance.as_str(),
            path: "/",
            status: 200,
            body: root_document.as_str(),
            content_type: Some("text/html"),
        },
        Asked {
            case: "another document of the same root",
            dns_name: deployed.by_instance.as_str(),
            path: "/health.txt",
            status: 200,
            body: "ok",
            content_type: Some("text/plain"),
        },
        Asked {
            case: "a path the target's root holds nothing at",
            dns_name: deployed.by_instance.as_str(),
            path: "/not-deployed.html",
            status: 404,
            body: "",
            content_type: None,
        },
        Asked {
            case: "the same instance registered by its address",
            dns_name: deployed.by_address.as_str(),
            path: "/",
            status: 200,
            body: root_document.as_str(),
            content_type: Some("text/html"),
        },
        Asked {
            case: "a healthy instance target running no server",
            dns_name: deployed.no_server.as_str(),
            path: "/",
            status: 502,
            body: "",
            content_type: None,
        },
        Asked {
            case: "a port the target's own server does not listen on",
            dns_name: deployed.unserved_port.as_str(),
            path: "/",
            status: 502,
            body: "",
            content_type: None,
        },
    ];
    for Asked {
        case,
        dns_name,
        path,
        status,
        body,
        content_type,
    } in rows
    {
        let answered = send(&deployed.router, "GET", dns_name, path, &[]).await;
        assert_eq!(
            answered.status, status,
            "case {case:?}: expected status {status}, got {} body {:?}",
            answered.status, answered.body
        );
        assert_eq!(
            answered.body.trim_end(),
            body,
            "case {case:?}: expected the body {body:?}, got {:?}",
            answered.body
        );
        assert_eq!(
            answered.header("content-type"),
            content_type,
            "case {case:?}: expected the content type {content_type:?}, got {:?}",
            answered.header("content-type")
        );
    }

    // And over a REAL connection to the address the load balancer published, which is what a
    // client handed that name dials: the same answer, off the socket.
    let dialed = exchange(
        "the published address",
        &deployed.by_instance,
        "/health.txt",
    )
    .await;
    assert_eq!(
        dialed.status, 200,
        "case \"the published address\": expected the target's own 200, got {} in {:?}",
        dialed.status, dialed.raw
    );
    assert!(
        dialed.raw.contains("ok"),
        "case \"the published address\": expected the target's own document, got {:?}",
        dialed.raw
    );
}

/// A socket client receives the two independently provisioned guest documents;
/// removing the second registration leaves only the first document reachable.
#[tokio::test]
async fn two_registered_guest_markers() {
    let router = emulator_with_ec2();
    let network = seed_network(&router).await;
    let group = target_group(&router, "paired", "paired", "instance", &network).await;
    let mut instances = Vec::new();
    for (address, marker) in [("10.80.1.10", "guest-alpha"), ("10.80.1.11", "guest-beta")] {
        let id = run_instance(
            &router,
            marker,
            &network,
            address,
            Some(serving_user_data().replace(MARKER, marker)),
        )
        .await;
        running(&router, marker, &id).await;
        register(&router, marker, &group, &id, TRAFFIC_PORT).await;
        instances.push(id);
    }
    assert_ne!(instances[0], instances[1]);
    let (arn, dns) = load_balancer(&router, "paired", "paired-alb", &network, &group, false).await;
    active(&router, "paired", &arn).await;
    health_reaching(&router, "paired", &group, "healthy").await;
    let mut observed = Vec::new();
    for _ in 0..20 {
        let response = exchange("paired", &dns, "/").await;
        assert_eq!(response.status, 200);
        let body = response.raw.split_once("\r\n\r\n").unwrap().1.trim();
        assert!(matches!(
            body,
            "<h1>guest-alpha</h1>" | "<h1>guest-beta</h1>"
        ));
        observed.push(body.to_owned());
    }
    println!("ALB paired group {group}, instances {instances:?}, authority {dns}, returned bodies {observed:?}");
    assert!(
        observed.windows(2).all(|pair| pair[0] != pair[1]),
        "{observed:?}"
    );
    let mut form = action("DeregisterTargets");
    form.push(p("TargetGroupArn", &group));
    form.push(p("Targets.member.1.Id", &instances[1]));
    form.push(p("Targets.member.1.Port", "80"));
    assert_eq!(call(&router, &form).await.status, 200);
    for _ in 0..4 {
        let response = exchange("single remaining", &dns, "/").await;
        assert_eq!(response.status, 200);
        assert_eq!(
            response.raw.split_once("\r\n\r\n").unwrap().1.trim(),
            "<h1>guest-alpha</h1>"
        );
    }
}
