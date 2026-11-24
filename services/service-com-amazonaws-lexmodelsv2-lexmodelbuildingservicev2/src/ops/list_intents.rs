// SPDX-License-Identifier: CC0-1.0

//! @op ListIntents
//! Lists stored intent summaries within a single existing bot version and locale.

use super::types::*;

smithy_macros::smithy_model!(
    "smithy-model/com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2.json"
);

#[smithy_macros::operation_error("com.amazonaws.lexmodelsv2#ListIntents")]
#[derive(Debug)]
pub enum ListIntentsErrorModeled {
    InternalServerException(InternalServerException),
    ServiceQuotaExceededException(ServiceQuotaExceededException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::list_intents::ListIntentsError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type ListIntentsError = crate::scaffolded::Scaffolded<ListIntentsErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<ListIntentsErrorModeled> for ListIntentsError {
    fn from(modeled: ListIntentsErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `ListIntents` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct ListIntents<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> ListIntents<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for ListIntents<Eng>
{
    type Input = ListIntentsInput;
    type Output = ListIntentsOutput;
    type Error = ListIntentsError;

    const NAME: &'static str = "ListIntents";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_INTENTS_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_INTENTS_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let state = crate::state::service_state(&*self.engine).ok_or_else(|| {
            ListIntentsErrorModeled::InternalServerException(InternalServerException {
                message: Some("the Lex Models V2 store is unavailable".into()),
            })
        })?;
        let (attribute, order) = input
            .sort_by
            .as_ref()
            .map_or(("IntentName", "Ascending"), |sort| {
                (sort.attribute.as_str(), sort.order.as_str())
            });
        if !matches!(attribute, "IntentName" | "LastUpdatedDateTime")
            || !matches!(order, "Ascending" | "Descending")
        {
            return Err(invalid("Invalid intent sort attribute or order"));
        }
        let filter = match input.filters.as_deref() {
            None => None,
            Some([f])
                if f.name == "IntentName"
                    && matches!(f.operator.as_str(), "EQ" | "CO")
                    && f.values.len() == 1 =>
            {
                Some((f.operator.as_str(), f.values[0].as_str()))
            }
            _ => {
                return Err(invalid(
                    "Expected one IntentName filter with one value and operator EQ or CO",
                ));
            }
        };
        let limit = input
            .max_results
            .unwrap_or(crate::state::MAX_LIST_PAGE as i32);
        if !(1..=crate::state::MAX_LIST_PAGE as i32).contains(&limit) {
            return Err(invalid("maxResults must be between 1 and 1000"));
        }
        let scope = serde_json::json!([
            "ListIntents",
            input.bot_id,
            input.bot_version,
            input.locale_id,
            attribute,
            order,
            filter
        ]);
        let store = state.lock();
        if !store.bot_locale_exists(&input.bot_id, &input.bot_version, &input.locale_id) {
            return Err(invalid("The requested bot version and locale do not exist"));
        }
        let mut intents: Vec<_> = store
            .intents_in_locale(&input.bot_id, &input.bot_version, &input.locale_id)
            .filter(|intent| match filter {
                Some(("EQ", value)) => intent.intent_name == value,
                Some((_, value)) => intent.intent_name.contains(value),
                None => true,
            })
            .collect();
        intents.sort_by(|a, b| {
            let by_field = if attribute == "IntentName" {
                a.intent_name.cmp(&b.intent_name)
            } else {
                a.last_updated_date_time.cmp(&b.last_updated_date_time)
            };
            let by_key = by_field.then_with(|| a.intent_id.cmp(&b.intent_id));
            if order == "Descending" {
                by_key.reverse()
            } else {
                by_key
            }
        });
        let mut start = 0;
        if let Some(token) = input.next_token.as_deref() {
            let (query, id): (serde_json::Value, String) = crate::state::decode_page_token(token)
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or_else(|| invalid("Invalid ListIntents nextToken"))?;
            if scope != query {
                return Err(invalid("nextToken does not belong to this listing"));
            }
            start = intents
                .iter()
                .position(|intent| intent.intent_id == id)
                .ok_or_else(|| invalid("The next intent no longer belongs to this listing"))?;
        }
        let end = (start + limit as usize).min(intents.len());
        let next_token = intents.get(end).map(|intent| {
            crate::state::encode_page_token(
                &serde_json::json!([scope, intent.intent_id]).to_string(),
            )
        });
        let summaries = intents[start..end]
            .iter()
            .map(|intent| IntentSummary {
                intent_id: Some(intent.intent_id.clone()),
                intent_name: Some(intent.intent_name.clone()),
                intent_display_name: intent.intent_display_name.clone(),
                description: intent.description.clone(),
                parent_intent_signature: intent.parent_intent_signature.clone(),
                input_contexts: intent.input_contexts.clone(),
                output_contexts: intent.output_contexts.clone(),
                last_updated_date_time: Some(crate::state::to_date_time(
                    intent.last_updated_date_time,
                )),
            })
            .collect();
        Ok(ListIntentsOutput {
            bot_id: Some(input.bot_id),
            bot_version: Some(input.bot_version),
            locale_id: Some(input.locale_id),
            intent_summaries: Some(summaries),
            next_token,
        })
    }
}

fn invalid(message: &str) -> ListIntentsError {
    ListIntentsErrorModeled::ValidationException(ValidationException {
        message: Some(message.to_owned()),
    })
    .into()
}
