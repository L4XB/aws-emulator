// SPDX-License-Identifier: CC0-1.0

//! @op ListBotLocales
//! Lists the stored locales for one existing bot version.
//! ---
//!
//! # Modeled contract for `ListBotLocales`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2/smithy-model/com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2.json")
//! ```
//!
//! Gets a list of locales for the specified bot.
//!
//! ## Input
//!
//! * `botId` (Id, len 10..10, pattern ^[0-9a-zA-Z]+$) **required** — The identifier of the
//!   bot to list locales for.
//! * `botVersion` (BotVersion, len 1..5, pattern ^(DRAFT|[0-9]+)$) **required** — The version
//!   of the bot to list locales for.
//! * `filters` (BotLocaleFilters, len 1..1) — Provides the specification for a filter used to
//!   limit the response to only those locales that match the filter specification. You can
//!   only specify one filter and one value to filter on.
//! * `maxResults` (MaxResults, range 1..1000) — The maximum number of aliases to return in
//!   each page of results. If there are fewer results than the max page size, only the actual
//!   number of results are returned.
//! * `nextToken` (NextToken) — If the response from the ListBotLocales operation contains
//!   more results than specified in the maxResults parameter, a token is returned in the
//!   response. Use that token as the nextToken parameter to return the next page of results.
//! * `sortBy` (BotLocaleSortBy) — Specifies sorting parameters for the list of locales. You
//!   can sort by locale name in ascending or descending order.
//!
//! ## Output
//!
//! * `botId` (Id, len 10..10, pattern ^[0-9a-zA-Z]+$) — The identifier of the bot to list
//!   locales for.
//! * `botLocaleSummaries` (BotLocaleSummaryList) — Summary information for the locales that
//!   meet the filter criteria specified in the request. The length of the list is specified
//!   in the maxResults parameter of the request. If there are more locales available, the
//!   nextToken field contains a token to get the next page of results.
//! * `botVersion` (BotVersion, len 1..5, pattern ^(DRAFT|[0-9]+)$) — The version of the bot.
//! * `nextToken` (NextToken) — A token that indicates whether there are more results to
//!   return in a response to the ListBotLocales operation. If the nextToken field is present,
//!   you send the contents as the nextToken parameter of a ListBotLocales operation request
//!   to get the next page of results.
//!
//! ## Errors
//!
//! * `InternalServerException` (HTTP 500) — The service encountered an unexpected condition.
//!   Try your request again.
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
// `ListBotLocalesError` enum is never produced.
smithy_macros::smithy_model!(
    "smithy-model/com-amazonaws-lexmodelsv2-lexmodelbuildingservicev2.json"
);

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `ListBotLocalesError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.lexmodelsv2#ListBotLocales")]
#[derive(Debug)]
pub enum ListBotLocalesErrorModeled {
    InternalServerException(InternalServerException),
    ServiceQuotaExceededException(ServiceQuotaExceededException),
    ThrottlingException(ThrottlingException),
    ValidationException(ValidationException),
}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::list_bot_locales::ListBotLocalesError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type ListBotLocalesError = crate::scaffolded::Scaffolded<ListBotLocalesErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<ListBotLocalesErrorModeled> for ListBotLocalesError {
    fn from(modeled: ListBotLocalesErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `ListBotLocales` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct ListBotLocales<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> ListBotLocales<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for ListBotLocales<Eng>
{
    type Input = ListBotLocalesInput;
    type Output = ListBotLocalesOutput;
    type Error = ListBotLocalesError;

    const NAME: &'static str = "ListBotLocales";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_BOT_LOCALES_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::LIST_BOT_LOCALES_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = ctx;
        let state = crate::state::service_state(&*self.engine).ok_or_else(|| {
            ListBotLocalesErrorModeled::InternalServerException(InternalServerException {
                message: Some("Lex Models V2 store unavailable".into()),
            })
        })?;
        let mut store = state.lock();
        if !store.bot_version_exists(&input.bot_id, &input.bot_version) {
            return Err(invalid("Unknown bot or version"));
        }
        let descending = match input.sort_by.as_ref() {
            None => false,
            Some(s) if s.attribute == "BotLocaleName" && s.order == "Ascending" => false,
            Some(s) if s.attribute == "BotLocaleName" && s.order == "Descending" => true,
            _ => return Err(invalid("Invalid locale sort")),
        };
        let filter = match input.filters.as_deref() {
            None => None,
            Some([f])
                if f.name == "BotLocaleName"
                    && f.values.len() == 1
                    && matches!(f.operator.as_str(), "EQ" | "CO") =>
            {
                Some((f.operator.as_str(), f.values[0].as_str()))
            }
            _ => return Err(invalid("Expected one BotLocaleName filter with one value")),
        };
        let scope = (&input.bot_id, &input.bot_version, descending, filter);
        let mut rows = Vec::new();
        for row in store.bot_locales.values_mut() {
            if row.bot_id != input.bot_id || row.bot_version != input.bot_version {
                continue;
            }
            if let Some((op, value)) = filter {
                if (op == "EQ" && row.locale_name != value)
                    || (op == "CO" && !row.locale_name.contains(value))
                {
                    continue;
                }
            }
            row.settle();
            rows.push(row);
        }
        rows.sort_by(|a, b| {
            let order = (&a.locale_name, &a.locale_id).cmp(&(&b.locale_name, &b.locale_id));
            if descending {
                order.reverse()
            } else {
                order
            }
        });
        let cursor = |row: &crate::state::BotLocale| {
            crate::state::encode_page_token(&serde_json::json!([scope, row.locale_id]).to_string())
        };
        let offset = match input.next_token.as_ref() {
            Some(token) => rows
                .iter()
                .position(|row| cursor(row) == *token)
                .ok_or_else(|| invalid("Invalid locale continuation token"))?,
            None => 0,
        };
        let limit = input.max_results.unwrap_or(100) as usize;
        let next_token = rows.get(offset + limit).map(|row| cursor(row));
        let summaries = rows
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|row| BotLocaleSummary {
                locale_id: Some(row.locale_id.clone()),
                locale_name: Some(row.locale_name.clone()),
                description: row.description.clone(),
                bot_locale_status: Some(row.bot_locale_status.clone()),
                last_updated_date_time: Some(crate::state::to_date_time(
                    row.last_updated_date_time,
                )),
                last_build_submitted_date_time: row
                    .last_build_submitted_date_time
                    .map(crate::state::to_date_time),
            })
            .collect();
        Ok(ListBotLocalesOutput {
            bot_id: Some(input.bot_id),
            bot_version: Some(input.bot_version),
            next_token,
            bot_locale_summaries: Some(summaries),
        })
    }
}

fn invalid(message: &str) -> ListBotLocalesError {
    ListBotLocalesErrorModeled::ValidationException(ValidationException {
        message: Some(message.into()),
    })
    .into()
}
