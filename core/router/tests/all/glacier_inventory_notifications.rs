// SPDX-License-Identifier: CC0-1.0

//! Drive Glacier, SNS, and SQS through their regular in-process service entry points.
//! Observe the messages accepted into the SNS subscription ledger independently of Glacier.

use bytes::Bytes;
use http::{Request, Response};
use serde_json::{json, Value};
use smithy_server_runtime::MultiServiceRouter;
use std::{sync::Arc, time::Duration};

const REGION: &str = "us-west-2";

async fn request(
    router: &MultiServiceRouter,
    service: &str,
    method: &str,
    path: &str,
    body: &str,
    target: Option<&str>,
) -> Response<Bytes> {
    let mut req = Request::builder().method(method).uri(path)
        .header("host", format!("{service}.{REGION}.amazonaws.com"))
        .header("authorization", format!("AWS4-HMAC-SHA256 Credential=FIXTURE/20261021/{REGION}/{service}/aws4_request, SignedHeaders=host, Signature=00"));
    if service == "sns" {
        req = req.header("content-type", "application/x-www-form-urlencoded");
    } else if let Some(name) = target {
        req = req
            .header("content-type", "application/x-amz-json-1.0")
            .header("x-amz-target", name);
    } else {
        req = req
            .header("content-type", "application/json")
            .header("x-amz-glacier-version", "2012-06-01");
    }
    let response = router
        .dispatch(req.body(Bytes::copy_from_slice(body.as_bytes())).unwrap())
        .await;
    assert!(
        response.status().is_success(),
        "{} {}",
        response.status(),
        String::from_utf8_lossy(response.body())
    );
    response
}

#[tokio::test]
async fn inventory_completion_publishes_only_for_the_selected_vault_and_event() {
    let engine = Arc::new(engine::InMemoryEngine::new());
    let mut router = MultiServiceRouter::new();
    router.set_request_authentication(false);
    router.register_service(service_com_amazonaws_glacier_glacier::EmulatedService::new(
        engine.clone(),
    ));
    router.register_service(
        service_com_amazonaws_sns_amazonsimplenotificationservice::EmulatedService::new(
            engine.clone(),
        ),
    );
    router.register_service(service_com_amazonaws_sqs_amazonsqs::EmulatedService::new(
        engine.clone(),
    ));
    let account = engine::emulator_account_id();
    let topic = format!("arn:aws:sns:{REGION}:{account}:inventory");
    let queue = format!("arn:aws:sqs:{REGION}:{account}:observer");
    request(
        &router,
        "sqs",
        "POST",
        "/",
        r#"{"QueueName":"observer"}"#,
        Some("AmazonSQS.CreateQueue"),
    )
    .await;
    request(
        &router,
        "sns",
        "POST",
        "/",
        "Action=CreateTopic&Version=2010-03-31&Name=inventory",
        None,
    )
    .await;
    request(
        &router,
        "sns",
        "POST",
        "/",
        &format!(
            "Action=Subscribe&Version=2010-03-31&TopicArn={topic}&Protocol=sqs&Endpoint={queue}"
        ),
        None,
    )
    .await;
    let mut jobs = Vec::new();
    for (vault, event) in [
        ("first", "InventoryRetrievalCompleted"),
        ("second", "ArchiveRetrievalCompleted"),
    ] {
        let path = format!("/-/vaults/{vault}");
        request(&router, "glacier", "PUT", &path, "", None).await;
        request(
            &router,
            "glacier",
            "PUT",
            &format!("{path}/notification-configuration"),
            &json!({"SNSTopic":topic,"Events":[event]}).to_string(),
            None,
        )
        .await;
        let job = request(
            &router,
            "glacier",
            "POST",
            &format!("{path}/jobs"),
            r#"{"Type":"inventory-retrieval"}"#,
            None,
        )
        .await;
        jobs.push(job.headers()["x-amz-job-id"].to_str().unwrap().to_owned());
    }
    let state =
        service_com_amazonaws_sns_amazonsimplenotificationservice::state::service_state(&*engine)
            .unwrap();
    // No DescribeJob or GetJobOutput call induces the publication.
    let poll = async {
        loop {
            let delivered = state
                .lock()
                .delivered
                .values()
                .flatten()
                .map(|m| m.message.to_string())
                .collect::<Vec<_>>();
            if !delivered.is_empty() {
                break delivered;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let messages = tokio::time::timeout(Duration::from_secs(8), poll)
        .await
        .unwrap();
    assert_eq!(messages.len(), 1);
    let payload: Value = serde_json::from_str(&messages[0]).unwrap();
    assert_eq!(payload["JobId"], jobs[0]);
    assert_eq!(payload["Action"], "InventoryRetrieval");
    assert_eq!(
        payload["VaultARN"],
        format!("arn:aws:glacier:{REGION}:{account}:vaults/first")
    );
    assert_eq!(payload["StatusCode"], "Succeeded");
    assert_eq!(payload["Completed"], true);
    assert_eq!(payload["SNSTopic"], topic);
    for _ in 0..2 {
        for (vault, job) in ["first", "second"].iter().zip(&jobs) {
            let result = request(
                &router,
                "glacier",
                "GET",
                &format!("/-/vaults/{vault}/jobs/{job}"),
                "",
                None,
            )
            .await;
            let status: Value = serde_json::from_slice(result.body()).unwrap();
            assert_eq!(status["Completed"], true);
        }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        state.lock().delivered.values().map(Vec::len).sum::<usize>(),
        1
    );
}
