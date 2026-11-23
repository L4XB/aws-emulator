// SPDX-License-Identifier: CC0-1.0

//! @op ListDataSources
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_ListDataSources.html>.
//!
//! # Modeled contract for `ListDataSources`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Lists the data source connectors that you have created.
//!
//! ## Input
//!
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index used with one or more data source connectors.
//! * `MaxResults` (MaxResultsIntegerForListDataSourcesRequest, range 1..100) — The maximum
//!   number of data source connectors to return.
//! * `NextToken` (NextToken, len 1..800) — If the previous response was incomplete (because
//!   there is more data to retrieve), Amazon Kendra returns a pagination token in the
//!   response. You can use this pagination token to retrieve the next set of data source
//!   connectors.
//!
//! ## Output
//!
//! * `NextToken` (NextToken, len 1..800) — If the response is truncated, Amazon Kendra
//!   returns this token that you can use in the subsequent request to retrieve the next set
//!   of data source connectors.
//! * `SummaryItems` (DataSourceSummaryList) — An array of summary information for one or more
//!   data source connector.
//!
//! ## Errors
//!
//! * `AccessDeniedException` (HTTP 403) — You don't have sufficient access to perform this
//!   action. Please ensure you have the required permission policies and user accounts and
//!   try again.
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
// `ListDataSourcesError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `ListDataSourcesError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#ListDataSources")]
#[derive(Debug)]
pub enum ListDataSourcesErrorModeled {
    AccessDeniedException(AccessDeniedException),
    InternalServerException(InternalServerException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::list_data_sources::ListDataSourcesError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type ListDataSourcesError = crate::scaffolded::Scaffolded<ListDataSourcesErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<ListDataSourcesErrorModeled> for ListDataSourcesError {
    fn from(modeled: ListDataSourcesErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `ListDataSources` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct ListDataSources<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> ListDataSources<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for ListDataSources<Eng>
{
    type Input = ListDataSourcesInput;
    type Output = ListDataSourcesOutput;
    type Error = ListDataSourcesError;

    const NAME: &'static str = "ListDataSources";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_DATA_SOURCES_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_DATA_SOURCES_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            ListDataSourcesErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing state store".into()),
            })
        })?;
        let mut state = store.lock();
        let now = std::time::SystemTime::now();
        state.materialize_due_transitions(now);
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            ListDataSourcesErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("index does not exist".into()),
            })
        })?;
        let scope = format!("ListDataSources:{}", input.index_id);
        let start = match input.next_token {
            Some(token) => crate::state::decode_page_token(&scope, &token)
                .filter(|key| key.len() == 36)
                .ok_or_else(|| {
                    crate::scaffolded::Scaffolded::validation("invalid pagination token")
                })?,
            None => String::new(),
        };
        let mut entries = index.data_sources.range(start..);
        let summary_items = entries
            .by_ref()
            .take(input.max_results.unwrap_or(100) as usize)
            .map(|(_, source)| {
                let row = &source.row;
                DataSourceSummary {
                    name: row.name.clone(),
                    id: row.id.clone(),
                    r#type: row.r#type.clone(),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    status: row.status.clone(),
                    language_code: row.language_code.clone(),
                }
            })
            .collect();
        let next_token = entries
            .next()
            .map(|(id, _)| crate::state::encode_page_token(&scope, id));
        Ok(ListDataSourcesOutput {
            summary_items: Some(summary_items),
            next_token,
        })
    }
}
