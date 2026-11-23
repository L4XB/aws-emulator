// SPDX-License-Identifier: CC0-1.0

//! @op PutVoiceConnectorLoggingConfiguration
//! # Modeled contract for `PutVoiceConnectorLoggingConfiguration`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-chimesdkvoice-chimesdktelephonyservice/smithy-model/com-amazonaws-chimesdkvoice-chimesdktelephonyservice.json")
//! ```
//!
//! Updates a Voice Connector's logging configuration.
//!
//! ## Input
//!
//! * `LoggingConfiguration` (LoggingConfiguration) **required** — The logging configuration
//!   being updated.
//! * `VoiceConnectorId` (VoiceConnectorId, pattern
//!   ^([a-z0-9]{21,22}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$)
//!   **required** — The Voice Connector ID.
//!
//! ## Output
//!
//! * `LoggingConfiguration` (LoggingConfiguration) — The updated logging configuration.
//!
//! ## Errors
//!
//! * `BadRequestException` (HTTP 400) — The input parameters don't match the service's
//!   restrictions.
//! * `ForbiddenException` (HTTP 403) — The client is permanently forbidden from making the
//!   request.
//! * `NotFoundException` (HTTP 404) — The requested resource couldn't be found.
//! * `ServiceFailureException` (HTTP 500) — The service encountered an unexpected error.
//! * `ServiceUnavailableException` (HTTP 503) — The service is currently unavailable.
//! * `ThrottledClientException` (HTTP 429) — The number of customer requests exceeds the
//!   request rate limit.
//! * `UnauthorizedClientException` (HTTP 401) — The client isn't authorized to request a
//!   resource.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `PutVoiceConnectorLoggingConfigurationError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-chimesdkvoice-chimesdktelephonyservice.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `PutVoiceConnectorLoggingConfigurationError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.chimesdkvoice#PutVoiceConnectorLoggingConfiguration")]
#[derive(Debug)]
pub enum PutVoiceConnectorLoggingConfigurationErrorModeled {
    BadRequestException(BadRequestException),
    ForbiddenException(ForbiddenException),
    NotFoundException(NotFoundException),
    ServiceFailureException(ServiceFailureException),
    ServiceUnavailableException(ServiceUnavailableException),
    ThrottledClientException(ThrottledClientException),
    UnauthorizedClientException(UnauthorizedClientException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::put_voice_connector_logging_configuration::PutVoiceConnectorLoggingConfigurationError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type PutVoiceConnectorLoggingConfigurationError = crate::scaffolded::Scaffolded<PutVoiceConnectorLoggingConfigurationErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<PutVoiceConnectorLoggingConfigurationErrorModeled> for PutVoiceConnectorLoggingConfigurationError {
    fn from(modeled: PutVoiceConnectorLoggingConfigurationErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `PutVoiceConnectorLoggingConfiguration` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct PutVoiceConnectorLoggingConfiguration<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> PutVoiceConnectorLoggingConfiguration<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for PutVoiceConnectorLoggingConfiguration<Eng>
{
    type Input = PutVoiceConnectorLoggingConfigurationInput;
    type Output = PutVoiceConnectorLoggingConfigurationOutput;
    type Error = PutVoiceConnectorLoggingConfigurationError;

    const NAME: &'static str = "PutVoiceConnectorLoggingConfiguration";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::PUT_VOICE_CONNECTOR_LOGGING_CONFIGURATION_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::PUT_VOICE_CONNECTOR_LOGGING_CONFIGURATION_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let state = crate::state::service_state(&*self.engine).ok_or_else(|| {
            PutVoiceConnectorLoggingConfigurationErrorModeled::ServiceFailureException(crate::state::store_unavailable())
        })?;
        let mut store = state.lock();
        let record = store.voice_connectors.get_mut(&input.voice_connector_id).ok_or_else(|| {
            PutVoiceConnectorLoggingConfigurationErrorModeled::NotFoundException(
                crate::state::voice_connector_not_found(&input.voice_connector_id),
            )
        })?;
        record.logging_configuration = Some(input.logging_configuration);
        Ok(PutVoiceConnectorLoggingConfigurationOutput {
            logging_configuration: record.logging_configuration.clone(),
        })
    }
}
