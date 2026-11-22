// SPDX-License-Identifier: CC0-1.0

//! Immediate, best-effort server access logs for successful GetObject and PutObject calls
//! on general-purpose buckets with a SimplePrefix and policy-authorized delivery.
//! A destination write is itself a normal in-process S3 operation, so destination policies,
//! including explicit denies and source conditions, apply. Delivery calls do not log themselves.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use aws_smithy_runtime_api::client::http::HttpConnector;
use aws_smithy_runtime_api::http::{Request, Response};
use aws_smithy_types::body::SdkBody;
use service_common::service_principal_calls::{ServicePrincipalCall, ServicePrincipalCalls};
use smithy_server_runtime::{BoxFuture, ErasedOperation, OperationContext, Service};

const PRINCIPAL: &str = "logging.s3.amazonaws.com";
const DELIVERY_LIMIT: Duration = Duration::from_secs(2);

pub(super) fn wrap<Eng: Send + Sync + 'static>(
    name: &str,
    inner: Box<dyn ErasedOperation>,
    engine: &Arc<Eng>,
) -> Box<dyn ErasedOperation> {
    let operation = match name {
        "GetObject" => "GET",
        "PutObject" => "PUT",
        _ => return inner,
    };
    Box::new(Logged {
        inner,
        operation,
        engine: Arc::clone(engine),
    })
}

struct Logged<Eng> {
    inner: Box<dyn ErasedOperation>,
    operation: &'static str,
    engine: Arc<Eng>,
}

impl<Eng: Send + Sync + 'static> ErasedOperation for Logged<Eng> {
    fn run<'r>(
        &'r self,
        svc: &'r dyn Service,
        req: &'r Request,
        ctx: &'r OperationContext<'r>,
    ) -> BoxFuture<'r, Response> {
        Box::pin(async move {
            let at = SystemTime::now();
            let response = self.inner.run(svc, req, ctx).await;
            if response.status().is_success() {
                let _ = tokio::time::timeout(DELIVERY_LIMIT, self.deliver(req, &response, ctx, at))
                    .await;
            }
            response
        })
    }

    fn auth_schemes(&self) -> Option<&'static [&'static str]> {
        self.inner.auth_schemes()
    }
}

impl<Eng: Send + Sync + 'static> Logged<Eng> {
    async fn deliver(
        &self,
        req: &Request,
        resp: &Response,
        ctx: &OperationContext<'_>,
        at: SystemTime,
    ) -> Option<()> {
        let engine = self.engine.as_ref();
        let path = super::request_metrics::path_style_path(engine, req);
        let bucket = super::addressed_bucket_name(&path)?;
        let object = super::wire::addressed_object(&path, super::wire::request_query(req.uri()))?;
        let (config, region) = {
            let state = crate::state::service_state(engine)?;
            let store = state.lock();
            let source = store.bucket(&bucket)?;
            let config = source.logging()?;
            let target = store.bucket(&config.target_bucket)?;
            if source.is_directory()
                || target.is_directory()
                || target.region != source.region
                || target.object_lock_enabled()
                || target.payer() == "Requester"
                || target.default_object_encryption().sse_algorithm.as_deref() != Some("AES256")
                || config
                    .target_grants
                    .as_ref()
                    .is_some_and(|grants| !grants.is_empty())
                || config
                    .target_object_key_format
                    .as_ref()
                    .is_some_and(|format| format.partitioned_prefix.is_some())
            {
                return None;
            }
            (config.clone(), source.region.clone())
        };
        if super::request_authorization::service_principal_call(engine, ctx)
            .is_some_and(|call| call.principal == PRINCIPAL)
        {
            return None;
        }
        let rid = resp.headers().get(super::wire::REQUEST_ID_HEADER)?;
        if rid.is_empty() || !rid.bytes().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        let when: chrono::DateTime<chrono::Utc> = at.into();
        let name = format!(
            "{}{}-{rid}",
            config.target_prefix,
            when.format("%Y-%m-%d-%H-%M-%S")
        );
        let key = encode_key(&object.key);
        // The dispatch API exposes the original URI but not its HTTP version.
        // Omit the query because it may carry signed credentials.
        let request_path = super::wire::request_path(req.uri());
        let status = resp.status().as_u16();
        let bytes = resp
            .body()
            .bytes()
            .map_or("-".to_owned(), |b| b.len().to_string());
        let body = format!(
            "{} {bucket} [{}] - - {rid} REST.{}.OBJECT {key} \"{} {request_path} -\" {status} - {bytes} - - - - - - {} - - - - - - -\n",
            crate::state::canonical_user_id(),
            when.format("%d/%b/%Y:%H:%M:%S +0000"),
            self.operation,
            self.operation,
            resp.headers().get("x-amz-id-2").unwrap_or("-")
        );
        let engine = (engine as &dyn std::any::Any).downcast_ref::<engine::InMemoryEngine>()?;
        let directory = engine.state::<ServicePrincipalCalls>();
        let credentials = ctx.credential_store();
        let minted = credentials.create(&emulator_server::aws_credentials::AWS_ACCESS_KEY);
        let guard = DeliveryIdentity {
            directory,
            credentials,
            key: minted.id.clone(),
        };
        let secret = minted.secret(emulator_server::aws_credentials::SECRET_ACCESS_KEY)?;
        let now = SystemTime::now();
        if !guard.directory.lock().remember(
            &guard.key,
            ServicePrincipalCall {
                principal: PRINCIPAL.to_owned(),
                source_arn: crate::state::bucket_arn(&bucket),
                source_account: engine::emulator_account_id().to_owned(),
                expires_at: now + DELIVERY_LIMIT + Duration::from_secs(30),
            },
            now,
        ) {
            return None;
        }
        // The host is used for S3 routing and signing only; the callback transport is in-process.
        let host = format!("s3.{region}.amazonaws.com");
        let uri = format!(
            "https://{host}/{}/{}",
            config.target_bucket,
            encode_key(&name)
        );
        let mut write = http::Request::put(uri)
            .header("host", &host)
            .header("if-none-match", "*")
            .body(body.into_bytes())
            .ok()?;
        service_common::in_process_sigv4::sign_in_process_request_as(
            &mut write, "s3", &region, &guard.key, secret,
        )
        .await
        .ok()?;
        let (parts, body) = write.into_parts();
        let request =
            Request::try_from(http::Request::from_parts(parts, SdkBody::from(body))).ok()?;
        let delivery = ctx.callback().http_connector().call(request).await.ok()?;
        delivery.status().is_success().then_some(())
    }
}

fn encode_key(key: &str) -> String {
    let mut encoded = String::new();
    for b in key.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            encoded.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(&mut encoded, "%{b:02X}");
        }
    }
    encoded
}

/// Withdraw both the temporary identity and its in-flight attribution on all exit paths.
struct DeliveryIdentity<'a> {
    directory: engine::ServiceState<ServicePrincipalCalls>,
    credentials: &'a emulator_server::CredentialStore,
    key: String,
}

impl Drop for DeliveryIdentity<'_> {
    fn drop(&mut self) {
        self.directory.lock().forget(&self.key);
        let _ = self.credentials.delete(&self.key);
    }
}
