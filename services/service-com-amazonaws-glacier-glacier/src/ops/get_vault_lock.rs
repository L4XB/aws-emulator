// SPDX-License-Identifier: CC0-1.0

//! @op GetVaultLock
//! Read only the selected vault lock subresource.
//!
//! # Modeled contract for `GetVaultLock`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-glacier-glacier/smithy-model/com-amazonaws-glacier-glacier.json")
//! ```
//!
//! This operation retrieves the following attributes from the lock-policy subresource set
//! on the specified vault: The vault lock policy set on the vault. The state of the vault
//! lock, which is either InProgess or Locked. When the lock ID expires. The lock ID is used
//! to complete the vault locking process. When the vault lock was initiated and put into
//! the InProgress state. A vault lock is put into the InProgress state by calling
//! InitiateVaultLock. A vault lock is put into the Locked state by calling
//! CompleteVaultLock. You can abort the vault locking process by calling AbortVaultLock.
//! For more information about the vault locking process, Amazon Glacier Vault Lock. If
//! there is no vault lock policy set on the vault, the operation returns a 404 Not found
//! error. For more information about vault lock policies, Amazon Glacier Access Control
//! with Vault Lock Policies.
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
//! * `CreationDate` (string) — The UTC date and time at which the vault lock was put into the
//!   InProgress state.
//! * `ExpirationDate` (string) — The UTC date and time at which the lock ID expires. This
//!   value can be null if the vault lock is in a Locked state.
//! * `Policy` (string) — The vault lock policy as a JSON string, which uses "\" as an escape
//!   character.
//! * `State` (string) — The state of the vault lock. InProgress or Locked.
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
// `GetVaultLockError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-glacier-glacier.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `GetVaultLockError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.glacier#GetVaultLock")]
#[derive(Debug)]
pub enum GetVaultLockErrorModeled {
    InvalidParameterValueException(InvalidParameterValueException),
    MissingParameterValueException(MissingParameterValueException),
    NoLongerSupportedException(NoLongerSupportedException),
    ResourceNotFoundException(ResourceNotFoundException),
    ServiceUnavailableException(ServiceUnavailableException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::get_vault_lock::GetVaultLockError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type GetVaultLockError = crate::scaffolded::Scaffolded<GetVaultLockErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<GetVaultLockErrorModeled> for GetVaultLockError {
    fn from(modeled: GetVaultLockErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `GetVaultLock` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct GetVaultLock<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> GetVaultLock<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for GetVaultLock<Eng>
{
    type Input = GetVaultLockInput;
    type Output = GetVaultLockOutput;
    type Error = GetVaultLockError;

    const NAME: &'static str = "GetVaultLock";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::GET_VAULT_LOCK_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = super::types::GET_VAULT_LOCK_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        use crate::state;
        let store = state::service_state(&*self.engine).ok_or_else(|| {
            GetVaultLockErrorModeled::ServiceUnavailableException(state::service_unavailable(state::NO_STORE_MESSAGE))
        })?;
        let account = state::resolve_account(&input.account_id)
            .map_err(GetVaultLockErrorModeled::InvalidParameterValueException)?;
        let region = state::request_region(ctx);
        let arn = state::vault_arn(&region, &input.vault_name);
        let missing = state::vault_not_found(&region, &input.vault_name);
        let store = store.lock();
        let vault = store.vaults.get(&(account, input.vault_name))
            .filter(|vault| vault.vault_arn == arn)
            .ok_or(GetVaultLockErrorModeled::ResourceNotFoundException(missing))?;
        let now = aws_smithy_types::DateTime::from(std::time::SystemTime::now());
        vault.lock_policy.as_ref().filter(|policy| {
            policy.state.as_deref() != Some("InProgress") || policy.expiration_date.as_deref()
                .and_then(state::parse_iso8601)
                .is_some_and(|expiry| expiry > (now.secs(), now.subsec_nanos()))
        }).cloned().ok_or_else(|| {
            GetVaultLockErrorModeled::ResourceNotFoundException(
                state::resource_not_found("The vault has no lock policy"),
            ).into()
        })
    }
}
