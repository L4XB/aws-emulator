// SPDX-License-Identifier: CC0-1.0

//! @op SetVaultNotifications
//! # Modeled contract for `SetVaultNotifications`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-glacier-glacier/smithy-model/com-amazonaws-glacier-glacier.json")
//! ```
//!
//! This operation configures notifications that will be sent when specific events happen to
//! a vault. By default, you don't get any notifications. To configure vault notifications,
//! send a PUT request to the notification-configuration subresource of the vault. The
//! request should include a JSON document that provides an Amazon SNS topic and specific
//! events for which you want Amazon Glacier to send notifications to the topic. Amazon SNS
//! topics must grant permission to the vault to be allowed to publish notifications to the
//! topic. You can configure a vault to publish a notification for the following vault
//! events: ArchiveRetrievalCompleted This event occurs when a job that was initiated for an
//! archive retrieval is completed (InitiateJob). The status of the completed job can be
//! "Succeeded" or "Failed". The notification sent to the SNS topic is the same output as
//! returned from DescribeJob. InventoryRetrievalCompleted This event occurs when a job that
//! was initiated for an inventory retrieval is completed (InitiateJob). The status of the
//! completed job can be "Succeeded" or "Failed". The notification sent to the SNS topic is
//! the same output as returned from DescribeJob. An AWS account has full permission to
//! perform all operations (actions). However, AWS Identity and Access Management (IAM)
//! users don't have any permissions by default. You must grant them explicit permission to
//! perform specific actions. For more information, see Access Control Using AWS Identity
//! and Access Management (IAM). For conceptual information and underlying REST API, see
//! Configuring Vault Notifications in Amazon Glacier and Set Vault Notification
//! Configuration in the Amazon Glacier Developer Guide.
//!
//! ## Input
//!
//! * `accountId` (string) **required** — The AccountId value is the AWS account ID of the
//!   account that owns the vault. You can either specify an AWS account ID or optionally a
//!   single '-' (hyphen), in which case Amazon Glacier uses the AWS account ID associated
//!   with the credentials used to sign the request. If you use an account ID, do not include
//!   any hyphens ('-') in the ID.
//! * `vaultName` (string) **required** — The name of the vault.
//! * `vaultNotificationConfig` (VaultNotificationConfig) — Provides options for specifying
//!   notification configuration.
//!
//! ## Errors
//!
//! * `InvalidParameterValueException` (HTTP 400) — Returned if a parameter of the request is
//!   incorrectly specified.
//! * `MissingParameterValueException` (HTTP 400) — Returned if a required header or parameter
//!   is missing from the request.
//! * `NoLongerSupportedException` (HTTP 400) — Returned if the request was made by a customer
//!   with no Amazon Glacier storage. The request is denied as the API is no longer supported
//!   for new customers. Please use Amazon S3 Glacier storage classes instead.
//! * `ResourceNotFoundException` (HTTP 404) — Returned if the specified resource (such as a
//!   vault, upload ID, or job ID) doesn't exist.
//! * `ServiceUnavailableException` (HTTP 500) — Returned if the service cannot complete the
//!   request.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `SetVaultNotificationsError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-glacier-glacier.json");

#[derive(Debug, Clone, Default)]
pub struct SetVaultNotificationsOutput;

impl SetVaultNotificationsOutput {
    // `with_http` carries this operation's modelled response status. Remove it and
    // the status falls back to the protocol default, so a modelled 204 answers 200.
    pub const SCHEMA: ::aws_smithy_schema::Schema = ::aws_smithy_schema::Schema::new_struct(
        ::aws_smithy_schema::ShapeId::from_static("smithy.api#Unit", "smithy.api", "Unit"),
        ::aws_smithy_schema::ShapeType::Structure,
        &[],
    )
    .with_http(::aws_smithy_schema::traits::HttpTrait::new(
        "PUT",
        "/{accountId}/vaults/{vaultName}/notification-configuration",
        Some(204u16),
    ));
}

pub static SET_VAULT_NOTIFICATIONS_OUTPUT_SCHEMA: &::aws_smithy_schema::Schema = &SetVaultNotificationsOutput::SCHEMA;

impl ::aws_smithy_schema::serde::SerializableStruct for SetVaultNotificationsOutput {
    fn serialize_members(&self, _s: &mut dyn ::aws_smithy_schema::serde::ShapeSerializer) -> Result<(), ::aws_smithy_schema::serde::SerdeError> { Ok(()) }
}

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `SetVaultNotificationsError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.glacier#SetVaultNotifications")]
#[derive(Debug)]
pub enum SetVaultNotificationsErrorModeled {
    InvalidParameterValueException(InvalidParameterValueException),
    MissingParameterValueException(MissingParameterValueException),
    NoLongerSupportedException(NoLongerSupportedException),
    ResourceNotFoundException(ResourceNotFoundException),
    ServiceUnavailableException(ServiceUnavailableException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::set_vault_notifications::SetVaultNotificationsError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type SetVaultNotificationsError = crate::scaffolded::Scaffolded<SetVaultNotificationsErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<SetVaultNotificationsErrorModeled> for SetVaultNotificationsError {
    fn from(modeled: SetVaultNotificationsErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `SetVaultNotifications` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct SetVaultNotifications<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> SetVaultNotifications<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for SetVaultNotifications<Eng>
{
    type Input = SetVaultNotificationsInput;
    type Output = SetVaultNotificationsOutput;
    type Error = SetVaultNotificationsError;

    const NAME: &'static str = "SetVaultNotifications";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::SET_VAULT_NOTIFICATIONS_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = SET_VAULT_NOTIFICATIONS_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 204u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        use crate::state;
        let store = state::service_state(&*self.engine).ok_or_else(|| {
            SetVaultNotificationsErrorModeled::ServiceUnavailableException(state::service_unavailable(state::NO_STORE_MESSAGE))
        })?;
        let account = state::resolve_account(&input.account_id)
            .map_err(SetVaultNotificationsErrorModeled::InvalidParameterValueException)?;
        let region = state::request_region(ctx);
        let arn = state::vault_arn(&region, &input.vault_name);
        let missing = state::vault_not_found(&region, &input.vault_name);
        let mut store = store.lock();
        let vault = store.vaults.get_mut(&(account, input.vault_name))
            .filter(|vault| vault.vault_arn == arn)
            .ok_or(SetVaultNotificationsErrorModeled::ResourceNotFoundException(missing))?;
        let config = input.vault_notification_config.ok_or_else(|| {
            SetVaultNotificationsErrorModeled::InvalidParameterValueException(
                state::invalid_parameter_value("A notification configuration is required"),
            )
        })?;
        let valid_topic = config.sns_topic.as_deref().is_some_and(|arn| {
            let parts: Vec<_> = arn.split(':').collect();
            matches!(parts.as_slice(), ["arn", partition, "sns", region, account, topic]
                if !partition.is_empty() && !region.is_empty() && account.len() == 12
                    && account.bytes().all(|b| b.is_ascii_digit()) && !topic.is_empty())
        });
        let valid_events = config.events.as_ref().is_some_and(|events| {
            !events.is_empty() && events.iter().all(|event| matches!(event.as_str(),
                "ArchiveRetrievalCompleted" | "InventoryRetrievalCompleted"))
        });
        if !valid_topic || !valid_events {
            return Err(SetVaultNotificationsErrorModeled::InvalidParameterValueException(
                state::invalid_parameter_value("Provide an SNS topic ARN and at least one supported retrieval event"),
            ).into());
        }
        vault.notification_config = Some(config);
        Ok(SetVaultNotificationsOutput)
    }
}
