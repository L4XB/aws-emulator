// SPDX-License-Identifier: CC0-1.0

//! @op CreateDataSource
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_CreateDataSource.html>.
//!
//! # Modeled contract for `CreateDataSource`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Creates a data source connector that you want to use with an Amazon Kendra index. You
//! specify a name, data source connector type and description for your data source. You
//! also specify configuration information for the data source connector. CreateDataSource
//! is a synchronous operation. The operation returns 200 if the data source was
//! successfully created. Otherwise, an exception is raised. For an example of creating an
//! index and data source using the Python SDK, see Getting started with Python SDK. For an
//! example of creating an index and data source using the Java SDK, see Getting started
//! with Java SDK.
//!
//! ## Input
//!
//! * `ClientToken` (ClientTokenName, len 1..100, @idempotencyToken) — A token that you
//!   provide to identify the request to create a data source connector. Multiple calls to the
//!   CreateDataSource API with the same client token will create only one data source
//!   connector.
//! * `Configuration` (DataSourceConfiguration) — Configuration information to connect to your
//!   data source repository. You can't specify the Configuration parameter when the Type
//!   parameter is set to CUSTOM. If you do, you receive a ValidationException exception. The
//!   Configuration parameter is required for all other data sources.
//! * `CustomDocumentEnrichmentConfiguration` (CustomDocumentEnrichmentConfiguration) —
//!   Configuration information for altering document metadata and content during the document
//!   ingestion process. For more information on how to create, modify and delete document
//!   metadata, or make other content alterations when you ingest documents into Amazon
//!   Kendra, see Customizing document metadata during the ingestion process.
//! * `Description` (Description, len 0..1000, pattern ^\P{C}*$) — A description for the data
//!   source connector.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index you want to use with the data source connector.
//! * `LanguageCode` (LanguageCode, len 2..10, pattern ^[a-zA-Z-]*$) — The code for a
//!   language. This allows you to support a language for all documents when creating the data
//!   source connector. English is supported by default. For more information on supported
//!   languages, including their codes, see Adding documents in languages other than English.
//! * `Name` (DataSourceName, len 1..1000, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** —
//!   A name for the data source connector.
//! * `RoleArn` (RoleArn, len 0..1284, pattern
//!   ^arn:[a-z0-9-\.]{1,63}:[a-z0-9-\.]{0,63}:[a-z0-9-\.]{0,63}:[a-z0-9-\.]{0,63}:[^/].{0,1023}$)
//!   — The Amazon Resource Name (ARN) of an IAM role with permission to access the data
//!   source and required resources. For more information, see IAM access roles for Amazon
//!   Kendra.. You can't specify the RoleArn parameter when the Type parameter is set to
//!   CUSTOM. If you do, you receive a ValidationException exception. The RoleArn parameter is
//!   required for all other data sources.
//! * `Schedule` (ScanSchedule) — Sets the frequency for Amazon Kendra to check the documents
//!   in your data source repository and update the index. If you don't set a schedule Amazon
//!   Kendra will not periodically update the index. You can call the StartDataSourceSyncJob
//!   API to update the index. Specify a cron- format schedule string or an empty string to
//!   indicate that the index is updated on demand. You can't specify the Schedule parameter
//!   when the Type parameter is set to CUSTOM. If you do, you receive a ValidationException
//!   exception.
//! * `Tags` (TagList, len 0..200) — A list of key-value pairs that identify or categorize the
//!   data source connector. You can also use tags to help control access to the data source
//!   connector. Tag keys and values can consist of Unicode letters, digits, white space, and
//!   any of the following symbols: _ . : / = + - @.
//! * `Type` (DataSourceType, one of: ALFRESCO | BOX | CONFLUENCE | CUSTOM | DATABASE | FSX |
//!   GITHUB | GOOGLEDRIVE | JIRA | ONEDRIVE | QUIP | S3 | SALESFORCE | SERVICENOW |
//!   SHAREPOINT | SLACK | TEMPLATE | WEBCRAWLER | WORKDOCS) **required** — The type of data
//!   source repository. For example, SHAREPOINT.
//! * `VpcConfiguration` (DataSourceVpcConfiguration) — Configuration information for an
//!   Amazon Virtual Private Cloud to connect to your data source. For more information, see
//!   Configuring a VPC.
//!
//! ## Output
//!
//! * `Id` (DataSourceId, len 1..100, pattern ^[a-zA-Z0-9][a-zA-Z0-9_-]*$) **required** — The
//!   identifier of the data source connector.
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
//! * `ResourceAlreadyExistException` (HTTP 400) — The resource you want to use already
//!   exists. Please check you have provided the correct resource and try again.
//! * `ResourceNotFoundException` (HTTP 404) — The resource you want to use doesn’t exist.
//!   Please check you have provided the correct resource and try again.
//! * `ServiceQuotaExceededException` (HTTP 402) — You have exceeded the set limits for your
//!   Amazon Kendra service. Please see Quotas for more information, or contact Support to
//!   inquire about an increase of limits.
//! * `ThrottlingException` (HTTP 429) — The request was denied due to request throttling.
//!   Please reduce the number of requests and try again.
//! * `ValidationException` (HTTP 400) — The input fails to satisfy the constraints set by the
//!   Amazon Kendra service. Please provide the correct input and try again.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `CreateDataSourceError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `CreateDataSourceError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#CreateDataSource")]
#[derive(Debug)]
pub enum CreateDataSourceErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceAlreadyExistException(ResourceAlreadyExistException),
    ResourceNotFoundException(ResourceNotFoundException),
    ServiceQuotaExceededException(ServiceQuotaExceededException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::create_data_source::CreateDataSourceError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type CreateDataSourceError = crate::scaffolded::Scaffolded<CreateDataSourceErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<CreateDataSourceErrorModeled> for CreateDataSourceError {
    fn from(modeled: CreateDataSourceErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `CreateDataSource` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct CreateDataSource<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> CreateDataSource<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for CreateDataSource<Eng>
{
    type Input = CreateDataSourceInput;
    type Output = CreateDataSourceOutput;
    type Error = CreateDataSourceError;

    const NAME: &'static str = "CreateDataSource";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::CREATE_DATA_SOURCE_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::CREATE_DATA_SOURCE_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let store = crate::state::service_state(&*self.engine).ok_or_else(|| {
            CreateDataSourceErrorModeled::InternalServerException(InternalServerException {
                message: Some("missing state store".into()),
            })
        })?;
        let mut state = store.lock();
        let now = std::time::SystemTime::now();
        state.materialize_due_transitions(now);
        let index = state.index_mut(&input.index_id).ok_or_else(|| {
            CreateDataSourceErrorModeled::ResourceNotFoundException(ResourceNotFoundException {
                message: Some("index does not exist".into()),
            })
        })?;
        if index.status != "ACTIVE" {
            return Err(
                CreateDataSourceErrorModeled::ConflictException(ConflictException {
                    message: Some("parent index is not active".into()),
                })
                .into(),
            );
        }
        if !matches!(input.r#type.as_str(), "WEBCRAWLER" | "S3") || input.tags.as_ref().is_some_and(|t| !t.is_empty()) {
            return Err(crate::scaffolded::Scaffolded::not_implemented(
                "other connector types or data source tags",
            ));
        }
        let configuration = input.configuration.as_ref().ok_or_else(|| {
            crate::scaffolded::Scaffolded::validation("configuration is required")
        })?;
        crate::state::validate_connector(&input.r#type, configuration)
            .map_err(crate::scaffolded::Scaffolded::validation)?;
        if input.role_arn.is_none() {
            return Err(crate::scaffolded::Scaffolded::validation(
                "RoleArn is required for this connector",
            ));
        }
        if let Some(token) = &input.client_token {
            if let Some((id, _)) = index
                .data_sources
                .iter()
                .find(|(_, data)| data.client_token.as_ref() == Some(token))
            {
                return Ok(CreateDataSourceOutput { id: id.clone() });
            }
        }
        if index
            .data_sources
            .values()
            .any(|source| source.row.name.as_ref() == Some(&input.name))
        {
            return Err(CreateDataSourceErrorModeled::ResourceAlreadyExistException(
                ResourceAlreadyExistException {
                    message: Some("data source name already exists".into()),
                },
            )
            .into());
        }
        let id = loop {
            let id = crate::state::mint_uuid();
            if !index.data_sources.contains_key(&id) {
                break id;
            }
        };
        let row = DescribeDataSourceOutput {
            id: Some(id.clone()),
            index_id: Some(input.index_id),
            name: Some(input.name),
            r#type: Some(input.r#type),
            configuration: input.configuration,
            vpc_configuration: input.vpc_configuration,
            created_at: Some(now.into()),
            updated_at: Some(now.into()),
            description: input.description,
            status: Some("CREATING".into()),
            schedule: input.schedule,
            role_arn: input.role_arn,
            error_message: None,
            language_code: input.language_code,
            custom_document_enrichment_configuration: input
                .custom_document_enrichment_configuration,
        };
        index.data_sources.insert(
            id.clone(),
            crate::state::StoredDataSource {
                row,
                client_token: input.client_token,
                transition_at: Some(now + crate::state::INDEX_ACTIVATION_DELAY),
            },
        );
        Ok(CreateDataSourceOutput { id })
    }
}
