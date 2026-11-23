// SPDX-License-Identifier: CC0-1.0

//! @op StartDataSourceSyncJob
//! Implements the explicitly mounted single-page fixture only.
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_StartDataSourceSyncJob.html>.
//!
//! # Modeled contract for `StartDataSourceSyncJob`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Starts a synchronization job for a data source connector. If a synchronization job is
//! already in progress, Amazon Kendra returns a ResourceInUseException exception.
//! Re-syncing your data source with your index after modifying, adding, or deleting
//! documents from your data source respository could take up to an hour or more, depending
//! on the number of documents to sync.
//!
//! ## Input
//!
//! * `Id` (DataSourceId, len 1..100, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** — The
//!   identifier of the data source connector to synchronize.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index used with the data source connector.
//!
//! ## Output
//!
//! * `ExecutionId` (String, len 1..2048) — Identifies a particular synchronization job.
//!
//! ## Errors
//!
//! * `AccessDeniedException` (HTTP 403) — You don't have sufficient access to perform this
//!   action. Please ensure you have the required permission policies and user accounts and
//!   try again.
//! * `ConflictException` (HTTP 409) — A conflict occurred with the request. Please fix any
//!   inconsistences with your resources and try again.
//! * `InternalServerException` (HTTP 500) — An issue occurred with the internal server used
//!   for your Amazon Kendra service. Please wait a few minutes and try again, or contact
//!   Support for help.
//! * `ResourceInUseException` (HTTP 400) — The resource you want to use is currently in use.
//!   Please check you have provided the correct resource and try again.
//! * `ResourceNotFoundException` (HTTP 404) — The resource you want to use doesn’t exist.
//!   Please check you have provided the correct resource and try again.
//! * `ThrottlingException` (HTTP 429) — The request was denied due to request throttling.
//!   Please reduce the number of requests and try again.
//! * `ValidationException` (HTTP 400) — The input fails to satisfy the constraints set by the
//!   Amazon Kendra service. Please provide the correct input and try again.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `StartDataSourceSyncJobError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `StartDataSourceSyncJobError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#StartDataSourceSyncJob")]
#[derive(Debug)]
pub enum StartDataSourceSyncJobErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceInUseException(ResourceInUseException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::start_data_source_sync_job::StartDataSourceSyncJobError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type StartDataSourceSyncJobError =
    crate::scaffolded::Scaffolded<StartDataSourceSyncJobErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<StartDataSourceSyncJobErrorModeled> for StartDataSourceSyncJobError {
    fn from(modeled: StartDataSourceSyncJobErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `StartDataSourceSyncJob` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct StartDataSourceSyncJob<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> StartDataSourceSyncJob<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for StartDataSourceSyncJob<Eng>
{
    type Input = StartDataSourceSyncJobInput;
    type Output = StartDataSourceSyncJobOutput;
    type Error = StartDataSourceSyncJobError;

    const NAME: &'static str = "StartDataSourceSyncJob";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::START_DATA_SOURCE_SYNC_JOB_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::START_DATA_SOURCE_SYNC_JOB_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            StartDataSourceSyncJobErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing Kendra state".into()),
            })
        })?;
        let mut state = store.lock();
        state.materialize_due_transitions(std::time::SystemTime::now());
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            StartDataSourceSyncJobErrorModeled::ResourceNotFoundException(
                ResourceNotFoundException {
                    message: Some("index does not exist".into()),
                },
            )
        })?;
        if index.status != "ACTIVE" || index.data_sources.len() != 1 || !index.documents.is_empty()
        {
            return Err(
                StartDataSourceSyncJobErrorModeled::ConflictException(ConflictException {
                    message: Some("exclusive empty index required".into()),
                })
                .into(),
            );
        }
        let source = index.data_sources.get_mut(&input.id).ok_or_else(|| {
            StartDataSourceSyncJobErrorModeled::ResourceNotFoundException(
                ResourceNotFoundException {
                    message: Some("unknown source in index".into()),
                },
            )
        })?;
        let Some(page) = source.page.clone() else {
            return Err(crate::scaffolded::Scaffolded::not_implemented(
                "general connector synchronization",
            ));
        };
        if source.job.is_some() || !page.matches(source) {
            return Err(
                StartDataSourceSyncJobErrorModeled::ConflictException(ConflictException {
                    message: Some("source changed or socket already used".into()),
                })
                .into(),
            );
        }
        let socket = page.take().ok_or_else(|| {
            crate::scaffolded::Scaffolded::validation("single-page transport consumed")
        })?;
        let id = crate::state::mint_uuid();
        source.job = Some(crate::single_page::running(&id));
        drop(state);
        let job_id = id.clone();
        tokio::task::spawn_blocking(move || {
            let result = page.fetch(socket);
            if let Some(index) = store.lock().index_mut(&input.index_id) {
                crate::single_page::finish(index, &input.id, &job_id, &page, result);
            }
        });
        Ok(StartDataSourceSyncJobOutput {
            execution_id: Some(id),
        })
    }
}
