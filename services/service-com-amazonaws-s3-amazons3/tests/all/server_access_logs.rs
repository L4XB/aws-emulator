// SPDX-License-Identifier: CC0-1.0

//! A bucket's own logging configuration and the destination service policy determine whether
//! a real object request leaves a log. Nothing in these tests inserts log objects directly.

use super::bucket_lifecycle::{put_bucket_policy, router, send};
use super::object_and_tagging_roundtrip::{bucket, put_object, text};
use smithy_server_runtime::MultiServiceRouter;

async fn enable(router: &MultiServiceRouter, source: &str, destination: &str) {
    let xml = format!(
        "<BucketLoggingStatus xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><LoggingEnabled><TargetBucket>{destination}</TargetBucket><TargetPrefix>records/</TargetPrefix></LoggingEnabled></BucketLoggingStatus>"
    );
    assert_eq!(
        send(router, "PUT", &format!("/{source}?logging"), &[], &xml)
            .await
            .status,
        200
    );
}

async fn logs(router: &MultiServiceRouter, bucket: &str) -> Vec<String> {
    let reply = send(
        router,
        "GET",
        &format!("/{bucket}?list-type=2&prefix=records%2F"),
        &[],
        "",
    )
    .await;
    assert_eq!(reply.status, 200);
    text(&reply)
        .split("<Key>")
        .skip(1)
        .map(|s| s.split("</Key>").next().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn server_access_logs_match_both_client_response_ids() {
    let r = router();
    let src = "source-access-records";
    let dst = "destination-access-records";
    bucket(&r, src).await;
    bucket(&r, dst).await;
    enable(&r, src, dst).await;
    // A configured destination with no grant receives nothing; the source still succeeds.
    assert_eq!(put_object(&r, src, "one", b"body", &[]).await.status, 200);
    assert!(logs(&r, dst).await.is_empty());
    let policy = serde_json::json!({"Version":"2012-10-17","Statement":[{
        "Effect":"Allow","Principal":{"Service":"logging.s3.amazonaws.com"},
        "Action":"s3:PutObject","Resource":format!("arn:aws:s3:::{dst}/records/*"),
        "Condition":{"ArnLike":{"aws:SourceArn":format!("arn:aws:s3:::{src}")},
                     "StringEquals":{"aws:SourceAccount":engine::emulator_account_id()}}
    }]});
    let mut unrelated = policy.clone();
    unrelated["Statement"][0]["Condition"]["ArnLike"]["aws:SourceArn"] =
        serde_json::json!("arn:aws:s3:::unrelated-source");
    assert_eq!(
        put_bucket_policy(&r, dst, &unrelated.to_string())
            .await
            .status,
        204
    );
    assert_eq!(
        put_object(&r, src, "no-source-match", b"body", &[])
            .await
            .status,
        200
    );
    assert!(logs(&r, dst).await.is_empty());
    let mut denied = policy.clone();
    let mut refusal = denied["Statement"][0].clone();
    refusal["Effect"] = serde_json::json!("Deny");
    denied["Statement"].as_array_mut().unwrap().push(refusal);
    assert_eq!(
        put_bucket_policy(&r, dst, &denied.to_string()).await.status,
        204
    );
    assert_eq!(
        put_object(&r, src, "explicit-deny", b"body", &[])
            .await
            .status,
        200
    );
    assert!(logs(&r, dst).await.is_empty());
    assert_eq!(
        put_bucket_policy(&r, dst, &policy.to_string()).await.status,
        204
    );
    let write = put_object(&r, src, "two", b"body", &[]).await;
    let read = send(&r, "GET", &format!("/{src}/two"), &[], "").await;
    assert_eq!((write.status, read.status), (200, 200));
    let keys = logs(&r, dst).await;
    assert_eq!(keys.len(), 2, "{keys:?}");
    let mut bodies = Vec::new();
    for key in keys {
        assert!(key.starts_with("records/"));
        bodies.push(text(
            &send(&r, "GET", &format!("/{dst}/{key}"), &[], "").await,
        ));
    }
    for (answer, method) in [(write, "PUT"), (read, "GET")] {
        let id = answer.headers["x-amz-request-id"].to_str().unwrap();
        assert_eq!(
            bodies
                .iter()
                .filter(|body| body.contains(&format!(" {id} REST.{method}.OBJECT two ")))
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn server_access_logs_self_target_does_not_record_its_own_delivery() {
    let r = router();
    let src = "self-target-records";
    bucket(&r, src).await;
    let policy = serde_json::json!({"Version":"2012-10-17","Statement":[{
        "Effect":"Allow","Principal":{"Service":"logging.s3.amazonaws.com"},
        "Action":"s3:PutObject","Resource":format!("arn:aws:s3:::{src}/records/*")
    }]});
    assert_eq!(
        put_bucket_policy(&r, src, &policy.to_string()).await.status,
        204
    );
    enable(&r, src, src).await;
    assert_eq!(
        put_object(&r, src, "normal", b"normal", &[]).await.status,
        200
    );
    assert_eq!(logs(&r, src).await.len(), 1);
}
