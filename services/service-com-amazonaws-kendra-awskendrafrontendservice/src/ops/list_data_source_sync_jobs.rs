// SPDX-License-Identifier: CC0-1.0

//! @op ListDataSourceSyncJobs
//! Implements the explicitly mounted single-page fixture only.
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_ListDataSourceSyncJobs.html>.
//!
//! # Modeled contract for `ListDataSourceSyncJobs`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Gets statistics about synchronizing a data source connector.
//!
//! ## Input
//!
//! * `Id` (DataSourceId, len 1..100, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** — The
//!   identifier of the data source connector.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index used with the data source connector.
//! * `MaxResults` (MaxResultsIntegerForListDataSourceSyncJobsRequest, range 1..10) — The
//!   maximum number of synchronization jobs to return in the response. If there are fewer
//!   results in the list, this response contains only the actual results.
//! * `NextToken` (NextToken, len 1..800) — If the previous response was incomplete (because
//!   there is more data to retrieve), Amazon Kendra returns a pagination token in the
//!   response. You can use this pagination token to retrieve the next set of jobs.
//! * `StartTimeFilter` (TimeRange) — When specified, the synchronization jobs returned in the
//!   list are limited to jobs between the specified dates.
//! * `StatusFilter` (DataSourceSyncJobStatus, one of: ABORTED | FAILED | INCOMPLETE |
//!   STOPPING | SUCCEEDED | SYNCING | SYNCING_INDEXING) — Only returns synchronization jobs
//!   with the Status field equal to the specified status.
//!
//! ## Output
//!
//! * `History` (DataSourceSyncJobHistoryList) — A history of synchronization jobs for the
//!   data source connector.
//! * `NextToken` (NextToken, len 1..800) — If the response is truncated, Amazon Kendra
//!   returns this token that you can use in the subsequent request to retrieve the next set
//!   of jobs.
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
// `ListDataSourceSyncJobsError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `ListDataSourceSyncJobsError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#ListDataSourceSyncJobs")]
#[derive(Debug)]
pub enum ListDataSourceSyncJobsErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::list_data_source_sync_jobs::ListDataSourceSyncJobsError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type ListDataSourceSyncJobsError =
    crate::scaffolded::Scaffolded<ListDataSourceSyncJobsErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<ListDataSourceSyncJobsErrorModeled> for ListDataSourceSyncJobsError {
    fn from(modeled: ListDataSourceSyncJobsErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `ListDataSourceSyncJobs` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct ListDataSourceSyncJobs<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> ListDataSourceSyncJobs<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for ListDataSourceSyncJobs<Eng>
{
    type Input = ListDataSourceSyncJobsInput;
    type Output = ListDataSourceSyncJobsOutput;
    type Error = ListDataSourceSyncJobsError;

    const NAME: &'static str = "ListDataSourceSyncJobs";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_DATA_SOURCE_SYNC_JOBS_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_DATA_SOURCE_SYNC_JOBS_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            ListDataSourceSyncJobsErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing Kendra state".into()),
            })
        })?;
        let mut state = store.lock();
        state.materialize_due_transitions(std::time::SystemTime::now());
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            ListDataSourceSyncJobsErrorModeled::ResourceNotFoundException(
                ResourceNotFoundException {
                    message: Some("index does not exist".into()),
                },
            )
        })?;
        let source = index.data_sources.get_mut(&input.id).ok_or_else(|| {
            ListDataSourceSyncJobsErrorModeled::ResourceNotFoundException(
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
        let _ = page;
        if input.next_token.is_some() {
            return Err(crate::scaffolded::Scaffolded::validation(
                "no additional single-page history",
            ));
        }
        let history = source
            .job
            .iter()
            .filter(|job| {
                input
                    .status_filter
                    .as_ref()
                    .is_none_or(|status| job.status.as_ref() == Some(status))
                    && input.start_time_filter.as_ref().is_none_or(|range| {
                        job.start_time.is_some_and(|when| {
                            range.start_time.is_none_or(|t| when >= t)
                                && range.end_time.is_none_or(|t| when <= t)
                        })
                    })
            })
            .cloned()
            .collect();
        Ok(ListDataSourceSyncJobsOutput {
            history: Some(history),
            next_token: None,
        })
    }
}
