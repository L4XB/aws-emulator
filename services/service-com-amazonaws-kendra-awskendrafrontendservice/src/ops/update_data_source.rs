// SPDX-License-Identifier: CC0-1.0

//! @op UpdateDataSource
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_UpdateDataSource.html>.
//!
//! # Modeled contract for `UpdateDataSource`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Updates an Amazon Kendra data source connector.
//!
//! ## Input
//!
//! * `Configuration` (DataSourceConfiguration) — Configuration information you want to update
//!   for the data source connector.
//! * `CustomDocumentEnrichmentConfiguration` (CustomDocumentEnrichmentConfiguration) —
//!   Configuration information you want to update for altering document metadata and content
//!   during the document ingestion process. For more information on how to create, modify and
//!   delete document metadata, or make other content alterations when you ingest documents
//!   into Amazon Kendra, see Customizing document metadata during the ingestion process.
//! * `Description` (Description, len 0..1000, pattern ^\P{C}*$) — A new description for the
//!   data source connector.
//! * `Id` (DataSourceId, len 1..100, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** — The
//!   identifier of the data source connector you want to update.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index used with the data source connector.
//! * `LanguageCode` (LanguageCode, len 2..10, pattern ^[a-zA-Z-]*$) — The code for a language
//!   you want to update for the data source connector. This allows you to support a language
//!   for all documents when updating the data source. English is supported by default. For
//!   more information on supported languages, including their codes, see Adding documents in
//!   languages other than English.
//! * `Name` (DataSourceName, len 1..1000, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) — A new name
//!   for the data source connector.
//! * `RoleArn` (RoleArn, len 0..1284, pattern
//!   ^arn:[a-z0-9-\.]{1,63}:[a-z0-9-\.]{0,63}:[a-z0-9-\.]{0,63}:[a-z0-9-\.]{0,63}:[^/].{0,1023}$)
//!   — The Amazon Resource Name (ARN) of an IAM role with permission to access the data
//!   source and required resources. For more information, see IAM roles for Amazon Kendra.
//! * `Schedule` (ScanSchedule) — The sync schedule you want to update for the data source
//!   connector.
//! * `VpcConfiguration` (DataSourceVpcConfiguration) — Configuration information for an
//!   Amazon Virtual Private Cloud to connect to your data source. For more information, see
//!   Configuring a VPC.
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
// `UpdateDataSourceError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

#[derive(Debug, Clone, Default)]
pub struct UpdateDataSourceOutput;

impl UpdateDataSourceOutput {
    // `with_http` carries this operation's modelled response status. Remove it and
    // the status falls back to the protocol default, so a modelled 204 answers 200.
    pub const SCHEMA: ::aws_smithy_schema::Schema = ::aws_smithy_schema::Schema::new_struct(
        ::aws_smithy_schema::ShapeId::from_static("smithy.api#Unit", "smithy.api", "Unit"),
        ::aws_smithy_schema::ShapeType::Structure,
        &[],
    );
}

pub static UPDATE_DATA_SOURCE_OUTPUT_SCHEMA: &::aws_smithy_schema::Schema =
    &UpdateDataSourceOutput::SCHEMA;

impl ::aws_smithy_schema::serde::SerializableStruct for UpdateDataSourceOutput {
    fn serialize_members(
        &self,
        _s: &mut dyn ::aws_smithy_schema::serde::ShapeSerializer,
    ) -> Result<(), ::aws_smithy_schema::serde::SerdeError> {
        Ok(())
    }
}

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `UpdateDataSourceError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#UpdateDataSource")]
#[derive(Debug)]
pub enum UpdateDataSourceErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::update_data_source::UpdateDataSourceError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type UpdateDataSourceError = crate::scaffolded::Scaffolded<UpdateDataSourceErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<UpdateDataSourceErrorModeled> for UpdateDataSourceError {
    fn from(modeled: UpdateDataSourceErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `UpdateDataSource` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct UpdateDataSource<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> UpdateDataSource<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for UpdateDataSource<Eng>
{
    type Input = UpdateDataSourceInput;
    type Output = UpdateDataSourceOutput;
    type Error = UpdateDataSourceError;

    const NAME: &'static str = "UpdateDataSource";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::UPDATE_DATA_SOURCE_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = UPDATE_DATA_SOURCE_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            UpdateDataSourceErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing state store".into()),
            })
        })?;
        let mut state = store.lock();
        let now = std::time::SystemTime::now();
        state.materialize_due_transitions(now);
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            UpdateDataSourceErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("index does not exist".into()),
            })
        })?;
        if index.status != "ACTIVE" {
            return Err(
                UpdateDataSourceErrorModeled::ConflictException(ConflictException {
                    message: Some("parent index is not active".into()),
                })
                .into(),
            );
        }
        if let Some(cfg) = &input.configuration {
            crate::state::validate_web_source(cfg)
                .map_err(crate::scaffolded::Scaffolded::validation)?;
        }
        if input.name.as_ref().is_some_and(|name| {
            index
                .data_sources
                .iter()
                .any(|(id, source)| id != &input.id && source.row.name.as_ref() == Some(name))
        }) {
            return Err(crate::scaffolded::Scaffolded::validation(
                "duplicate data source name",
            ));
        }
        let source = index.data_sources.get_mut(&input.id).ok_or_else(|| {
            UpdateDataSourceErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("data source does not exist in index".into()),
            })
        })?;
        if source.row.status.as_deref() != Some("ACTIVE") {
            return Err(
                UpdateDataSourceErrorModeled::ConflictException(ConflictException {
                    message: Some("data source must be active".into()),
                })
                .into(),
            );
        }
        if input.name.is_some() {
            source.row.name = input.name;
        }
        if input.configuration.is_some() {
            source.row.configuration = input.configuration;
        }
        if input.vpc_configuration.is_some() {
            source.row.vpc_configuration = input.vpc_configuration;
        }
        if input.description.is_some() {
            source.row.description = input.description;
        }
        if input.schedule.is_some() {
            source.row.schedule = input.schedule;
        }
        if input.role_arn.is_some() {
            source.row.role_arn = input.role_arn;
        }
        if input.language_code.is_some() {
            source.row.language_code = input.language_code;
        }
        if input.custom_document_enrichment_configuration.is_some() {
            source.row.custom_document_enrichment_configuration =
                input.custom_document_enrichment_configuration;
        }
        source.row.status = Some("UPDATING".into());
        source.row.updated_at = Some(now.into());
        source.transition_at = Some(now + crate::state::INDEX_ACTIVATION_DELAY);
        Ok(UpdateDataSourceOutput)
    }
}
