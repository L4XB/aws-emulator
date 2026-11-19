// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P1): ANONYMOUS reads on a router that ENFORCES signatures, which is how the
//! emulator runs by default (`--auth on`).
//!
//! Upstream's router refused every request that presented no credential before any S3
//! handler ran, so a public object and a public static website answered 403 although the
//! bucket grants read access to everyone. AWS serves them: "If you grant READ access to the
//! anonymous user, the GetObject operation returns the object without using an authorization
//! header" (`GetObject`), and "If you want your website to be public, you must make all your
//! content publicly readable for your customers to be able to access it at the website
//! endpoint" (Amazon S3 User Guide, *Website endpoints*).
//!
//! The rule under test: with signatures enforced, a request presenting NO credential reaches
//! `GetObject`, `HeadObject` and the website endpoint, which judge it against what the bucket
//! grants to everyone (bucket policy, ACL, Block Public Access) exactly as they do on a
//! router that does not enforce signatures; every other anonymous request (a write, a delete,
//! a listing, a request to another service) is still refused by the router, and a request
//! carrying a credential that does not verify is still refused.
//!
//! The rows differ along the bucket's grant (public policy, nothing), the surface (REST
//! path-style, REST virtual-hosted-style, website hyphen and dot forms), the method (GET,
//! HEAD, PUT, DELETE) and the operation family (object read, object write, listing).

use std::sync::Arc;

use service_com_amazonaws_s3_amazons3::EmulatedService;
use smithy_server_runtime::MultiServiceRouter;

use super::bucket_lifecycle::{send_bytes, Answer};
use super::object_and_tagging_roundtrip::{bucket, put_object, text};

/// The bucket whose policy grants `s3:GetObject` on every object to every principal.
const PUBLIC: &str = "fork-anon-public";
/// The bucket that grants nothing to anyone (Block Public Access left at its default).
const PRIVATE: &str = "fork-anon-private";
/// The object both buckets hold.
const KEY: &str = "data.csv";
/// Its bytes.
const BODY: &[u8] = b"id,value\n1,public\n";
/// The index document both buckets hold, and what the website root serves.
const INDEX_BODY: &str = "<html>fork index</html>";

/// One request presenting NO credential: no `Authorization` header and no presigned query,
/// which is what an unsigned SDK client, `curl` or a browser sends.
async fn anonymous(router: &MultiServiceRouter, method: &str, host: &str, uri: &str) -> Answer {
    let request = http::Request::builder()
        .method(method)
        .uri(format!("https://{host}{uri}"))
        .header("host", host)
        .body(bytes::Bytes::new())
        .expect("the test request must build");
    let response = router.dispatch(request).await;
    let (parts, body) = response.into_parts();
    Answer {
        status: parts.status.as_u16(),
        headers: parts.headers,
        body,
    }
}

/// A configuration request on the router that does not enforce signatures, required to
/// answer `expected`.
async fn configure(
    router: &MultiServiceRouter,
    uri: &str,
    content_type: &str,
    body: &str,
    expected: u16,
) {
    let answered = send_bytes(router, "PUT", uri, &[("content-type", content_type)], body.as_bytes())
        .await;
    assert_eq!(
        answered.status, expected,
        "setup {uri:?} must answer {expected}, got {} with {:?}",
        answered.status,
        text(&answered)
    );
}

/// Both buckets, their objects and their website configuration; only [`PUBLIC`] gets the
/// public policy (after its Block Public Access settings are lifted).
async fn stand_up(router: &MultiServiceRouter) {
    for name in [PUBLIC, PRIVATE] {
        bucket(router, name).await;
        for (key, body, media_type) in [
            (KEY, BODY, "text/csv"),
            ("index.html", INDEX_BODY.as_bytes(), "text/html"),
        ] {
            let written = put_object(router, name, key, body, &[("content-type", media_type)]).await;
            assert_eq!(
                written.status, 200,
                "setup: {name}/{key} must be written, got {} with {:?}",
                written.status,
                text(&written)
            );
        }
        configure(
            router,
            &format!("/{name}?website"),
            "application/xml",
            "<WebsiteConfiguration><IndexDocument><Suffix>index.html</Suffix></IndexDocument>\
             </WebsiteConfiguration>",
            200,
        )
        .await;
    }
    configure(
        router,
        &format!("/{PUBLIC}?publicAccessBlock"),
        "application/xml",
        "<PublicAccessBlockConfiguration><BlockPublicAcls>false</BlockPublicAcls>\
         <IgnorePublicAcls>false</IgnorePublicAcls><BlockPublicPolicy>false</BlockPublicPolicy>\
         <RestrictPublicBuckets>false</RestrictPublicBuckets></PublicAccessBlockConfiguration>",
        200,
    )
    .await;
    let policy = format!(
        "{{\"Version\":\"2012-10-17\",\"Statement\":[{{\"Sid\":\"PublicRead\",\
         \"Effect\":\"Allow\",\"Principal\":\"*\",\"Action\":\"s3:GetObject\",\
         \"Resource\":\"arn:aws:s3:::{PUBLIC}/*\"}}]}}"
    );
    configure(router, &format!("/{PUBLIC}?policy"), "application/json", &policy, 204).await;
}

#[tokio::test]
async fn anonymous_reads_are_judged_by_the_bucket_with_signatures_enforced() {
    let engine: Arc<engine::InMemoryEngine> = Arc::new(engine::InMemoryEngine::new());
    let mut router = MultiServiceRouter::new();
    // The fixture is written with enforcement off; the requests under test run with it on.
    router.set_request_authentication(false);
    router.register_service(EmulatedService::new(Arc::clone(&engine)));
    stand_up(&router).await;
    router.set_request_authentication(true);

    // (case, method, host, uri, expected status, expected body for a served GET)
    let website_hyphen = format!("{PUBLIC}.s3-website-us-east-1.amazonaws.com");
    let website_dot = format!("{PUBLIC}.s3-website.us-east-1.amazonaws.com");
    let private_website = format!("{PRIVATE}.s3-website-us-east-1.amazonaws.com");
    let virtual_host = format!("{PUBLIC}.s3.us-east-1.amazonaws.com");
    let rows: Vec<(&str, &str, &str, String, u16, Option<&[u8]>)> = vec![
        ("path-style GET of a public object", "GET", "localhost", format!("/{PUBLIC}/{KEY}"), 200, Some(BODY)),
        ("path-style GET via the loopback address", "GET", "127.0.0.1", format!("/{PUBLIC}/{KEY}"), 200, Some(BODY)),
        ("HEAD of a public object", "HEAD", "localhost", format!("/{PUBLIC}/{KEY}"), 200, None),
        ("virtual-hosted GET of a public object", "GET", virtual_host.as_str(), format!("/{KEY}"), 200, Some(BODY)),
        ("website root, hyphen form", "GET", website_hyphen.as_str(), "/".to_string(), 200, Some(INDEX_BODY.as_bytes())),
        ("website root, dot form", "GET", website_dot.as_str(), "/".to_string(), 200, Some(INDEX_BODY.as_bytes())),
        ("website key", "GET", website_hyphen.as_str(), format!("/{KEY}"), 200, Some(BODY)),
        ("path-style GET of a private object", "GET", "localhost", format!("/{PRIVATE}/{KEY}"), 403, None),
        ("HEAD of a private object", "HEAD", "localhost", format!("/{PRIVATE}/{KEY}"), 403, None),
        ("website root of a private bucket", "GET", private_website.as_str(), "/".to_string(), 403, None),
        // Still refused by the router: these handlers would read a request with no
        // credential as the account's own.
        ("anonymous PUT into a public bucket", "PUT", "localhost", format!("/{PUBLIC}/new.csv"), 403, None),
        ("anonymous DELETE in a public bucket", "DELETE", "localhost", format!("/{PUBLIC}/{KEY}"), 403, None),
        ("anonymous listing of a public bucket", "GET", "localhost", format!("/{PUBLIC}"), 403, None),
    ];
    for (case, method, host, uri, expected, body) in &rows {
        let answered = anonymous(&router, method, host, uri).await;
        assert_eq!(
            answered.status,
            *expected,
            "case {case:?}: {method} {host}{uri} expected {expected}, got {} with {:?}",
            answered.status,
            text(&answered)
        );
        if let Some(body) = body {
            let served: &[u8] = answered.body.as_ref();
            assert_eq!(
                served,
                *body,
                "case {case:?}: expected the stored bytes, got {:?}",
                text(&answered)
            );
        }
        if *expected == 403 && *method != "HEAD" && !host.contains("s3-website") {
            assert_eq!(
                answered.error_code(),
                "AccessDenied",
                "case {case:?}: expected AccessDenied, got {:?}",
                text(&answered)
            );
        }
    }

    // The refused DELETE changed nothing: the object is still served.
    let after = anonymous(&router, "GET", "localhost", &format!("/{PUBLIC}/{KEY}")).await;
    let after_body: &[u8] = after.body.as_ref();
    assert_eq!(
        (after.status, after_body),
        (200, BODY),
        "case {:?}: the object must survive the refused anonymous DELETE, got {:?}",
        "after delete",
        text(&after)
    );

    // A request that carries a credential is NOT anonymous: one that does not verify is
    // refused even where the bucket is public.
    let forged = http::Request::builder()
        .method("GET")
        .uri(format!("https://localhost/{PUBLIC}/{KEY}"))
        .header("host", "localhost")
        .header(
            "authorization",
            "AWS4-HMAC-SHA256 Credential=AKIAFORGED0000000000/20261119/us-east-1/s3/aws4_request, \
             SignedHeaders=host;x-amz-date, Signature=0000000000000000000000000000000000000000000000000000000000000000",
        )
        .header("x-amz-date", "20261119T120000Z")
        .header("x-amz-content-sha256", "UNSIGNED-PAYLOAD")
        .body(bytes::Bytes::new())
        .expect("the test request must build");
    let refused = router.dispatch(forged).await;
    assert_eq!(
        refused.status().as_u16(),
        403,
        "case {:?}: a credential that does not verify must still be refused, got {}",
        "forged signature",
        refused.status()
    );
}

#[tokio::test]
async fn an_anonymous_request_to_another_service_is_still_refused() {
    let engine: Arc<engine::InMemoryEngine> = Arc::new(engine::InMemoryEngine::new());
    let mut router = MultiServiceRouter::new();
    router.set_request_authentication(true);
    router.register_service(EmulatedService::new(Arc::clone(&engine)));
    router.register_service(service_com_amazonaws_sqs_amazonsqs::EmulatedService::new(
        Arc::clone(&engine),
    ));
    // An unsigned SQS call: the opt-in seam is S3's alone, so the router still refuses it.
    let request = http::Request::builder()
        .method("POST")
        .uri("https://sqs.us-east-1.amazonaws.com/")
        .header("host", "sqs.us-east-1.amazonaws.com")
        .header("x-amz-target", "AmazonSQS.ListQueues")
        .header("content-type", "application/x-amz-json-1.0")
        .body(bytes::Bytes::from_static(b"{}"))
        .expect("the test request must build");
    let response = router.dispatch(request).await;
    let status = response.status().as_u16();
    let body = String::from_utf8_lossy(response.body()).to_string();
    assert!(
        status != 200 && body.contains("AccessDenied"),
        "case {:?}: expected the router's AccessDenied refusal, got {status} with {body:?}",
        "unsigned SQS ListQueues"
    );
}
