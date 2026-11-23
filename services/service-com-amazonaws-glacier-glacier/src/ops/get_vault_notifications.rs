// SPDX-License-Identifier: CC0-1.0

//! @op GetVaultNotifications
//! # Modeled contract for `GetVaultNotifications`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-glacier-glacier/smithy-model/com-amazonaws-glacier-glacier.json")
//! ```
//!
//! This operation retrieves the notification-configuration subresource of the specified
//! vault. For information about setting a notification configuration on a vault, see
//! SetVaultNotifications. If a notification configuration for a vault is not set, the
//! operation returns a 404 Not Found error. For more information about vault notifications,
//! see Configuring Vault Notifications in Amazon Glacier. An AWS account has full
//! permission to perform all operations (actions). However, AWS Identity and Access
//! Management (IAM) users don't have any permissions by default. You must grant them
//! explicit permission to perform specific actions. For more information, see Access
//! Control Using AWS Identity and Access Management (IAM). For conceptual information and
//! underlying REST API, see Configuring Vault Notifications in Amazon Glacier and Get Vault
//! Notification Configuration in the Amazon Glacier Developer Guide.
//!
//! ## Input
//!
//! * `accountId` (string) **required** — The AccountId value is the AWS account ID of the
//!   account that owns the vault. You can either specify an AWS account ID or optionally a
//!   single '-' (hyphen), in which case Amazon Glacier uses the AWS account ID associated
//!   with the credentials used to sign the request. If you use an account ID, do not include
//!   any hyphens ('-') in the ID.
//! * `vaultName` (string) **required** — The name of the vault.
//!
//! ## Output
//!
//! * `vaultNotificationConfig` (VaultNotificationConfig) — Returns the notification
//!   configuration set on the vault.
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
// `GetVaultNotificationsError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-glacier-glacier.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `GetVaultNotificationsError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.glacier#GetVaultNotifications")]
#[derive(Debug)]
pub enum GetVaultNotificationsErrorModeled {
    InvalidParameterValueException(InvalidParameterValueException),
    MissingParameterValueException(MissingParameterValueException),
    NoLongerSupportedException(NoLongerSupportedException),
    ResourceNotFoundException(ResourceNotFoundException),
    ServiceUnavailableException(ServiceUnavailableException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::get_vault_notifications::GetVaultNotificationsError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type GetVaultNotificationsError = crate::scaffolded::Scaffolded<GetVaultNotificationsErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<GetVaultNotificationsErrorModeled> for GetVaultNotificationsError {
    fn from(modeled: GetVaultNotificationsErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `GetVaultNotifications` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct GetVaultNotifications<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> GetVaultNotifications<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for GetVaultNotifications<Eng>
{
    type Input = GetVaultNotificationsInput;
    type Output = GetVaultNotificationsOutput;
    type Error = GetVaultNotificationsError;

    const NAME: &'static str = "GetVaultNotifications";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::GET_VAULT_NOTIFICATIONS_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::GET_VAULT_NOTIFICATIONS_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        use crate::state;
        let store = state::service_state(&*self.engine).ok_or_else(|| {
            GetVaultNotificationsErrorModeled::ServiceUnavailableException(state::service_unavailable(state::NO_STORE_MESSAGE))
        })?;
        let account = state::resolve_account(&input.account_id)
            .map_err(GetVaultNotificationsErrorModeled::InvalidParameterValueException)?;
        let region = state::request_region(ctx);
        let arn = state::vault_arn(&region, &input.vault_name);
        let missing = state::vault_not_found(&region, &input.vault_name);
        let store = store.lock();
        let vault = store.vaults.get(&(account, input.vault_name))
            .filter(|vault| vault.vault_arn == arn)
            .ok_or(GetVaultNotificationsErrorModeled::ResourceNotFoundException(missing))?;
        let config = vault.notification_config.clone().ok_or_else(|| {
            GetVaultNotificationsErrorModeled::ResourceNotFoundException(
                state::resource_not_found("The vault has no notification configuration"),
            )
        })?;
        Ok(GetVaultNotificationsOutput { vault_notification_config: Some(config) })
    }
}
