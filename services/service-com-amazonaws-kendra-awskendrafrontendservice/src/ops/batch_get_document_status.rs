// SPDX-License-Identifier: CC0-1.0

//! @op BatchGetDocumentStatus
//! See <https://docs.aws.amazon.com/kendra/latest/APIReference/API_BatchGetDocumentStatus.html>.
//!
//! # Modeled contract for `BatchGetDocumentStatus`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-kendra-awskendrafrontendservice/smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json")
//! ```
//!
//! Returns the indexing status for one or more documents submitted with the
//! BatchPutDocument API. When you use the BatchPutDocument API, documents are indexed
//! asynchronously. You can use the BatchGetDocumentStatus API to get the current status of
//! a list of documents so that you can determine if they have been successfully indexed.
//! You can also use the BatchGetDocumentStatus API to check the status of the
//! BatchDeleteDocument API. When a document is deleted from the index, Amazon Kendra
//! returns NOT_FOUND as the status.
//!
//! ## Input
//!
//! * `DocumentInfoList` (DocumentInfoList, len 1..10) **required** — A list of DocumentInfo
//!   objects that identify the documents for which to get the status. You identify the
//!   documents by their document ID and optional attributes.
//! * `IndexId` (IndexId, len 36..36, pattern ^[a-zA-Z0-9][a-zA-Z0-9-]*$) **required** — The
//!   identifier of the index to add documents to. The index ID is returned by the CreateIndex
//!   API.
//!
//! ## Output
//!
//! * `DocumentStatusList` (DocumentStatusList) — The status of documents. The status
//!   indicates if the document is waiting to be indexed, is in the process of indexing, has
//!   completed indexing, or failed indexing. If a document failed indexing, the status
//!   provides the reason why.
//! * `Errors` (BatchGetDocumentStatusResponseErrors) — A list of documents that Amazon Kendra
//!   couldn't get the status for. The list includes the ID of the document and the reason
//!   that the status couldn't be found.
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
// `BatchGetDocumentStatusError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-kendra-awskendrafrontendservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `BatchGetDocumentStatusError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.kendra#BatchGetDocumentStatus")]
#[derive(Debug)]
pub enum BatchGetDocumentStatusErrorModeled {
    AccessDeniedException(AccessDeniedException),
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    ResourceNotFoundException(ResourceNotFoundException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::batch_get_document_status::BatchGetDocumentStatusError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type BatchGetDocumentStatusError =
    crate::scaffolded::Scaffolded<BatchGetDocumentStatusErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<BatchGetDocumentStatusErrorModeled> for BatchGetDocumentStatusError {
    fn from(modeled: BatchGetDocumentStatusErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `BatchGetDocumentStatus` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct BatchGetDocumentStatus<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> BatchGetDocumentStatus<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for BatchGetDocumentStatus<Eng>
{
    type Input = BatchGetDocumentStatusInput;
    type Output = BatchGetDocumentStatusOutput;
    type Error = BatchGetDocumentStatusError;

    const NAME: &'static str = "BatchGetDocumentStatus";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::BATCH_GET_DOCUMENT_STATUS_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::BATCH_GET_DOCUMENT_STATUS_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let state = crate::state::service_state(&*self.engine).ok_or_else(|| {
            BatchGetDocumentStatusError::Modeled(
                BatchGetDocumentStatusErrorModeled::InternalServerException(
                    InternalServerException {
                        message: Some(
                            "the kendra emulator could not reach its state store".to_string(),
                        ),
                    },
                ),
            )
        })?;
        let mut state = state.lock();
        let now = ::std::time::SystemTime::now();
        state.materialize_due_transitions(now);

        let Some(index) = state.index(&input.index_id) else {
            return Err(
                BatchGetDocumentStatusErrorModeled::ResourceNotFoundException(
                    ResourceNotFoundException {
                        message: Some(format!("no index with id {}", input.index_id)),
                    },
                )
                .into(),
            );
        };

        if index.status != crate::state::STATUS_ACTIVE {
            return Err(
                BatchGetDocumentStatusErrorModeled::ConflictException(ConflictException {
                    message: Some(format!(
                    "index {} is in status {}; it must be {} before document status can be read",
                    index.id,
                    index.status,
                    crate::state::STATUS_ACTIVE
                )),
                })
                .into(),
            );
        }

        // A version without a connector job is ignored by the DocumentInfo contract.
        // The emulator currently stores only direct ingestion, without connector job history.
        for info in &input.document_info_list {
            let mut source = false;
            let mut job = false;
            for attribute in info.attributes.iter().flatten() {
                match attribute.key.as_str() {
                    "version" => (),
                    "dataSourceId" | "datasourceId" => source = true,
                    "jobExecutionId" => job = true,
                    _ => {
                        return Err(crate::scaffolded::Scaffolded::validation(
                            "unsupported DocumentInfo attribute",
                        ))
                    }
                }
            }
            if source != job {
                return Err(crate::scaffolded::Scaffolded::validation(
                    "dataSourceId and jobExecutionId must be used together",
                ));
            }
            if source {
                return Err(crate::scaffolded::Scaffolded::not_implemented(
                    "connector job document version status",
                ));
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut statuses = Vec::new();
        for info in input.document_info_list {
            if seen.insert(info.document_id.clone()) {
                let status = match index.documents.get(&info.document_id) {
                    None => "NOT_FOUND",
                    Some(doc) if doc.replaced_existing => "UPDATED",
                    Some(_) => "INDEXED",
                };
                statuses.push(Status {
                    document_id: Some(info.document_id),
                    document_status: Some(status.into()),
                    ..Default::default()
                });
            }
        }
        Ok(BatchGetDocumentStatusOutput {
            document_status_list: Some(statuses),
            errors: Some(Vec::new()),
        })
    }
}
