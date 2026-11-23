// SPDX-License-Identifier: CC0-1.0

//! @op DeleteDataSource
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_DeleteDataSource.html>.
//!
//! # Modeled contract for `DeleteDataSource`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Deletes an Amazon Kendra data source connector. An exception is not thrown if the data
//! source is already being deleted. While the data source is being deleted, the Status
//! field returned by a call to the DescribeDataSource API is set to DELETING. For more
//! information, see Deleting Data Sources. Deleting an entire data source or re-syncing
//! your index after deleting specific documents from a data source could take up to an hour
//! or more, depending on the number of documents you want to delete.
//!
//! ## Input
//!
//! * `Id` (DataSourceId, len 1..100, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** — The
//!   identifier of the data source connector you want to delete.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index used with the data source connector.
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
// `DeleteDataSourceError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

#[derive(Debug, Clone, Default)]
pub struct DeleteDataSourceOutput;

impl DeleteDataSourceOutput {
    // `with_http` carries this operation's modelled response status. Remove it and
    // the status falls back to the protocol default, so a modelled 204 answers 200.
    pub const SCHEMA: ::aws_smithy_schema::Schema = ::aws_smithy_schema::Schema::new_struct(
        ::aws_smithy_schema::ShapeId::from_static("smithy.api#Unit", "smithy.api", "Unit"),
        ::aws_smithy_schema::ShapeType::Structure,
        &[],
    );
}

pub static DELETE_DATA_SOURCE_OUTPUT_SCHEMA: &::aws_smithy_schema::Schema =
    &DeleteDataSourceOutput::SCHEMA;

impl ::aws_smithy_schema::serde::SerializableStruct for DeleteDataSourceOutput {
    fn serialize_members(
        &self,
        _s: &mut dyn ::aws_smithy_schema::serde::ShapeSerializer,
    ) -> Result<(), ::aws_smithy_schema::serde::SerdeError> {
        Ok(())
    }
}

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `DeleteDataSourceError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#DeleteDataSource")]
#[derive(Debug)]
pub enum DeleteDataSourceErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::delete_data_source::DeleteDataSourceError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type DeleteDataSourceError = crate::scaffolded::Scaffolded<DeleteDataSourceErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<DeleteDataSourceErrorModeled> for DeleteDataSourceError {
    fn from(modeled: DeleteDataSourceErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `DeleteDataSource` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct DeleteDataSource<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> DeleteDataSource<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for DeleteDataSource<Eng>
{
    type Input = DeleteDataSourceInput;
    type Output = DeleteDataSourceOutput;
    type Error = DeleteDataSourceError;

    const NAME: &'static str = "DeleteDataSource";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::DELETE_DATA_SOURCE_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = DELETE_DATA_SOURCE_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            DeleteDataSourceErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing state store".into()),
            })
        })?;
        let mut state = store.lock();
        let now = std::time::SystemTime::now();
        state.materialize_due_transitions(now);
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            DeleteDataSourceErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("index does not exist".into()),
            })
        })?;
        if index.status != "ACTIVE" {
            return Err(
                DeleteDataSourceErrorModeled::ConflictException(ConflictException {
                    message: Some("parent index is not active".into()),
                })
                .into(),
            );
        }
        let source = index.data_sources.get_mut(&input.id).ok_or_else(|| {
            DeleteDataSourceErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("data source does not exist in index".into()),
            })
        })?;
        if source.row.status.as_deref() != Some("ACTIVE") {
            return Err(
                DeleteDataSourceErrorModeled::ConflictException(ConflictException {
                    message: Some("data source must be active".into()),
                })
                .into(),
            );
        }
        source.row.status = Some("DELETING".into());
        source.row.updated_at = Some(now.into());
        source.transition_at = Some(now + crate::state::INDEX_ACTIVATION_DELAY);
        Ok(DeleteDataSourceOutput)
    }
}
