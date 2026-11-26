// SPDX-License-Identifier: CC0-1.0

//! Delete a DRAFT custom slot type from its owning locale.
//! Existing slot references are protected unless the caller explicitly skips that check.

//! # Modeled contract for `DeleteSlotType`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2/smithy-model/com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2.json")
//! ```
//!
//! Deletes a slot type from a bot locale. If a slot is using the slot type, Amazon Lex
//! throws a ResourceInUseException exception. To avoid the exception, set the
//! skipResourceInUseCheck parameter to true.
//!
//! ## Input
//!
//! * `botId` (Id, len 10..10, pattern ^[0-9a-zA-Z]+$) **required** — The identifier of the
//!   bot associated with the slot type.
//! * `botVersion` (DraftBotVersion, len 5..5, pattern ^DRAFT$) **required** — The version of
//!   the bot associated with the slot type.
//! * `localeId` (LocaleId) **required** — The identifier of the language and locale that the
//!   slot type will be deleted from. The string must match one of the supported locales. For
//!   more information, see Supported languages.
//! * `skipResourceInUseCheck` (SkipResourceInUseCheck, default false) **required** — By
//!   default, the DeleteSlotType operations throws a ResourceInUseException exception if you
//!   try to delete a slot type used by a slot. Set the skipResourceInUseCheck parameter to
//!   true to skip this check and remove the slot type even if a slot uses it.
//! * `slotTypeId` (Id, len 10..10, pattern ^[0-9a-zA-Z]+$) **required** — The identifier of
//!   the slot type to delete.
//!
//! ## Errors
//!
//! * `ConflictException` (HTTP 409) — The action that you tried to perform couldn't be
//!   completed because the resource is in a conflicting state. For example, deleting a bot
//!   that is in the CREATING state. Try your request again.
//! * `InternalServerException` (HTTP 500) — The service encountered an unexpected condition.
//!   Try your request again.
//! * `PreconditionFailedException` (HTTP 412) — Your request couldn't be completed because
//!   one or more request fields aren't valid. Check the fields in your request and try again.
//! * `ServiceQuotaExceededException` (HTTP 402) — You have reached a quota for your bot.
//! * `ThrottlingException` (HTTP 429) — Your request rate is too high. Reduce the frequency
//!   of requests.
//! * `ValidationException` (HTTP 400) — One of the input parameters in your request isn't
//!   valid. Check the parameters and try your request again.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `DeleteSlotTypeError` enum is never produced.
smithy_macros::smithy_model!(
    "smithy-model/com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2.json"
);

#[derive(Debug, Clone, Default)]
pub struct DeleteSlotTypeOutput;

impl DeleteSlotTypeOutput {
    // `with_http` carries this operation's modelled response status. Remove it and
    // the status falls back to the protocol default, so a modelled 204 answers 200.
    pub const SCHEMA: ::aws_smithy_schema::Schema = ::aws_smithy_schema::Schema::new_struct(
        ::aws_smithy_schema::ShapeId::from_static("smithy.api#Unit", "smithy.api", "Unit"),
        ::aws_smithy_schema::ShapeType::Structure,
        &[],
    )
    .with_http(::aws_smithy_schema::traits::HttpTrait::new(
        "DELETE",
        "/bots/{botId}/botversions/{botVersion}/botlocales/{localeId}/slottypes/{slotTypeId}",
        Some(204u16),
    ));
}

pub static DELETE_SLOT_TYPE_OUTPUT_SCHEMA: &::aws_smithy_schema::Schema =
    &DeleteSlotTypeOutput::SCHEMA;

impl ::aws_smithy_schema::serde::SerializableStruct for DeleteSlotTypeOutput {
    fn serialize_members(
        &self,
        _s: &mut dyn ::aws_smithy_schema::serde::ShapeSerializer,
    ) -> Result<(), ::aws_smithy_schema::serde::SerdeError> {
        Ok(())
    }
}

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `DeleteSlotTypeError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.lexmodelsv2#DeleteSlotType")]
#[derive(Debug)]
pub enum DeleteSlotTypeErrorModeled {
    ConflictException(ConflictException),
    InternalServerException(InternalServerException),
    PreconditionFailedException(PreconditionFailedException),
    ServiceQuotaExceededException(ServiceQuotaExceededException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::delete_slot_type::DeleteSlotTypeError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type DeleteSlotTypeError = crate::scaffolded::Scaffolded<DeleteSlotTypeErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<DeleteSlotTypeErrorModeled> for DeleteSlotTypeError {
    fn from(modeled: DeleteSlotTypeErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `DeleteSlotType` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct DeleteSlotType<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> DeleteSlotType<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for DeleteSlotType<Eng>
{
    type Input = DeleteSlotTypeInput;
    type Output = DeleteSlotTypeOutput;
    type Error = DeleteSlotTypeError;

    const NAME: &'static str = "DeleteSlotType";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::DELETE_SLOT_TYPE_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema = DELETE_SLOT_TYPE_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 204u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let state = crate::state::service_state(&*self.engine).ok_or_else(|| {
            DeleteSlotTypeErrorModeled::InternalServerException(InternalServerException {
                message: Some("the Lex Models V2 store is unavailable".into()),
            })
        })?;
        let mut store = state.lock();
        if store
            .slot_type(
                &input.bot_id,
                &input.bot_version,
                &input.locale_id,
                &input.slot_type_id,
            )
            .is_none()
        {
            // An absent type cannot be removed, including through a different locale.
            return Ok(DeleteSlotTypeOutput);
        }
        if !input.skip_resource_in_use_check
            && store.slots.values().any(|slot| {
                slot.bot_id == input.bot_id
                    && slot.bot_version == input.bot_version
                    && slot.locale_id == input.locale_id
                    && slot.slot_type_id.as_deref() == Some(&input.slot_type_id)
            })
        {
            return Err(
                DeleteSlotTypeErrorModeled::ConflictException(ConflictException {
                    message: Some("A slot in the locale still references this slot type.".into()),
                })
                .into(),
            );
        }
        store.slot_types.remove(&crate::state::versioned_key(
            &input.bot_version,
            &input.slot_type_id,
        ));
        Ok(DeleteSlotTypeOutput)
    }
}
