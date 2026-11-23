// SPDX-License-Identifier: CC0-1.0

//! Engine state for the com-amazonaws-chimesdkvoice-chimesdktelephonyservice service.
//!
//! The Voice Connector is this service's primary resource: `CreateVoiceConnector`,
//! `GetVoiceConnector`, `ListVoiceConnectors`, `UpdateVoiceConnector` and
//! `DeleteVoiceConnector` all read and write the one [`VoiceConnectorRecord`] map held
//! here, so every one of them answers from persisted state rather than from its own
//! request. The helpers beside the map (id minting, the derived outbound host name and
//! ARN, the record -> wire-shape projection and the opaque pagination token) live here
//! for the same reason: five operation files share them, and a second copy of any of
//! them would let two operations disagree about one resource.
//!
//! The Voice Connector GROUP is the second resource, and it REFERENCES the first: a group
//! routes inbound calls to the Voice Connectors its items name. Its records live in a map
//! beside the connectors ([`VoiceConnectorGroupRecord`]), and the question a group's write
//! asks of the connector store — does every connector this group routes to exist — is
//! answered in one place ([`ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState::first_absent_voice_connector`]).
//!
//! One thing this service reads is NOT in the struct below, because the other Chime
//! services read and write it too and the engine keys state by type: the family's
//! ARN-keyed TAG directory ([`chime_tags`]), which holds the tags of every Voice
//! Connector — including the ones a create was given.

use std::collections::HashMap;

use ::aws_smithy_types::DateTime;

use crate::ops::types::{
    BadRequestException, NotFoundException, ResourceLimitExceededException,
    ServiceFailureException, VoiceConnector, VoiceConnectorGroup, VoiceConnectorItem,
};

/// The Region a Voice Connector is created in when the request names none.
///
/// From the model: `VoiceConnector$AwsRegion` documents "Default: us-east-1", and
/// `VoiceConnectorAwsRegion` enumerates the ten Regions the member accepts.
pub const DEFAULT_AWS_REGION: &str = "us-east-1";

/// Suffix of the outbound host name derived for a Voice Connector:
/// `<VoiceConnectorId>.voiceconnector.chime.aws`.
pub const OUTBOUND_HOST_SUFFIX: &str = "voiceconnector.chime.aws";

/// The `arnNamespace` this service's ARNs carry (`OPERATIONS.md`, `## Wire protocol`).
pub const ARN_NAMESPACE: &str = "chime";

/// The resource TYPE of a Voice Connector's ARN: `arn:aws:chime:<region>:<account>:vc/<id>`,
/// as [`VoiceConnectorRecord::new`] derives it. It is the one resource type this service
/// mints, so it is the one an ARN has to carry for this store to say whether the resource
/// exists.
pub const VOICE_CONNECTOR_RESOURCE_TYPE: &str = "vc";

/// The resource TYPE of a Voice Connector GROUP's ARN:
/// `arn:aws:chime:<region>:<account>:vcg/<id>`, as [`VoiceConnectorGroupRecord::new`]
/// derives it — the group counterpart of [`VOICE_CONNECTOR_RESOURCE_TYPE`].
pub const VOICE_CONNECTOR_GROUP_RESOURCE_TYPE: &str = "vcg";

/// How many tags one resource of THIS service carries at once: the bound its own model
/// declares on `TagList` (`@length(min: 1, max: 50)`,
/// `smithy_query(query="operation",operation="TagResource")`), which is the family's
/// usual one — so the shared directory's default bound is this service's bound too.
pub const MAX_TAGS_PER_RESOURCE: usize = ::service_common::chime_tags::MAX_TAGS_PER_RESOURCE;

/// Page size `ListVoiceConnectors` uses when the request names no `MaxResults`: the
/// maximum the model allows that member (`ResultMax`, range 1..100).
pub const MAX_LIST_PAGE_SIZE: i32 = 100;

/// The most Voice Connectors this emulator holds at once.
///
/// `voice_connectors` grows by one record per `CreateVoiceConnector` request and shrinks
/// only on an explicit delete, so without a ceiling its entry COUNT — and with it the
/// per-request ordering `ListVoiceConnectors` computes — is set by how many requests a
/// caller makes. AWS bounds the same thing with a per-account Voice Connector service
/// quota and refuses a create past it; the quota's value is adjustable and is not in the
/// model, so this number is the EMULATOR's own bound rather than a claim about AWS's,
/// while the refusal it produces is the error the model declares for exactly this case:
/// `ResourceLimitExceededException` (HTTP 400, "The request exceeds the resource limit.").
pub const MAX_VOICE_CONNECTORS: usize = 500;

/// The most Voice Connector groups this emulator holds at once.
///
/// The same bound, for the same reason, as [`MAX_VOICE_CONNECTORS`]: `voice_connector_groups`
/// grows by one record per `CreateVoiceConnectorGroup` request and shrinks only on an
/// explicit delete, so without a ceiling its entry count is set by how many requests a caller
/// makes. AWS bounds the same thing with an adjustable per-account quota that is not in the
/// model, so this number is the EMULATOR's own bound, while the refusal it produces is the
/// error the model declares for exactly this case: `ResourceLimitExceededException`
/// (HTTP 400, "The request exceeds the resource limit.").
pub const MAX_VOICE_CONNECTOR_GROUPS: usize = 500;

/// The most Voice Connectors ONE group routes to.
///
/// From the model: `VoiceConnectorItem`'s own documentation states "Limit: 3
/// VoiceConnectorItems per Voice Connector group" (`smithy_query(query="shape",
/// shape="VoiceConnectorItem")`), which the list shape itself does not declare as a
/// `@length`, so it is enforced by the handler.
pub const MAX_VOICE_CONNECTOR_ITEMS_PER_GROUP: usize = 3;

// The failures the Voice Connector operations share.
//
// Each of the five handlers raises these through its OWN operation's error enum (the
// variants a model declares differ per operation), but the payload inside is the same
// refusal, and a payload built inline in five places drifts in wording. These
// constructors are the one place each payload's `Code` — a value of the model's
// `ErrorCode` enum — and its message are chosen.

/// `BadRequestException`'s payload: the request broke a rule the service enforces.
pub fn bad_request(message: impl Into<String>) -> BadRequestException {
    BadRequestException {
        code: Some("BadRequest".to_string()),
        message: Some(message.into()),
    }
}

/// The `Code` every not-found payload of this service carries: the `NotFound` value of the
/// model's `ErrorCode` enum (`smithy_query(query="shape", shape="ErrorCode")`).
pub const NOT_FOUND_CODE: &str = "NotFound";

/// `NotFoundException`'s payload for `message`: the `NotFound` code and that sentence.
///
/// The ONE place the code of a not-found refusal is chosen, so the sentences below — which
/// differ per resource, because the wire words them per resource — cannot disagree about the
/// code a client branches on.
pub fn not_found(message: String) -> NotFoundException {
    NotFoundException {
        code: Some(NOT_FOUND_CODE.to_string()),
        message: Some(message),
    }
}

/// `NotFoundException`'s payload for a well-formed id that names no stored connector: the
/// resource class that could not be resolved, and the id the request gave, quoted.
///
/// The fixed half is the sentence the wire answers a read, an update or a delete of an id no
/// connector holds with, recorded in the
/// `chime-sdk-voice-voice-connector-not-found-and-list` and
/// `chime-sdk-voice-voice-connector-not-found-errors` traces of the `simparity-wire-aws-v1`
/// corpus; the id is the request's.
pub fn voice_connector_not_found(voice_connector_id: &str) -> NotFoundException {
    not_found(format!(
        "Could not find VoiceConnector with Id '{voice_connector_id}'"
    ))
}

/// `NotFoundException`'s payload for a well-formed id that names no stored connector group.
///
/// The same two parts as a connector's — the resource class and the quoted id — in the
/// sentence the wire words for a GROUP, which is not the connector's sentence: a refusal
/// states the resource class it could not resolve, and each class's sentence is stated here
/// once rather than per operation. The fixed half is the one the wire answers a
/// `GetVoiceConnectorGroup` for an id no group holds with, recorded in the
/// `chime-sdk-voice-voice-connector-group-validation-and-not-found` trace of the
/// `simparity-wire-aws-v1` corpus; the id is the request's.
pub fn voice_connector_group_not_found(voice_connector_group_id: &str) -> NotFoundException {
    not_found(format!(
        "Could not find VoiceConnectorGroup for voice connector group id \
         '{voice_connector_group_id}'"
    ))
}

/// `ServiceFailureException`'s payload for a state store that could not be reached —
/// the one case a handler cannot serve from state at all.
pub fn store_unavailable() -> ServiceFailureException {
    ServiceFailureException {
        code: Some("ServiceFailure".to_string()),
        message: Some("the Voice Connector store is unavailable".to_string()),
    }
}

/// `ResourceLimitExceededException`'s payload for a store already holding
/// [`MAX_VOICE_CONNECTORS`] connectors.
pub fn voice_connector_limit_reached() -> ResourceLimitExceededException {
    ResourceLimitExceededException {
        code: Some("ResourceLimitExceeded".to_string()),
        message: Some(format!(
            "this account already holds the maximum of {MAX_VOICE_CONNECTORS} Voice Connectors"
        )),
    }
}

/// `ResourceLimitExceededException`'s payload for a store already holding
/// [`MAX_VOICE_CONNECTOR_GROUPS`] connector groups.
///
/// The CLASS and the status are the model's for exactly this case; the SENTENCE is this
/// emulator's own, worded like the connector ceiling's above, because no recording of this
/// service shows AWS's sentence for a quota refusal and neither the model nor the API
/// reference states one. A recording that shows it replaces this text.
pub fn voice_connector_group_limit_reached() -> ResourceLimitExceededException {
    ResourceLimitExceededException {
        code: Some("ResourceLimitExceeded".to_string()),
        message: Some(format!(
            "this account already holds the maximum of {MAX_VOICE_CONNECTOR_GROUPS} Voice \
             Connector groups"
        )),
    }
}

/// `ResourceLimitExceededException`'s payload for a group asked to route to more than
/// [`MAX_VOICE_CONNECTOR_ITEMS_PER_GROUP`] Voice Connectors; `requested` is how many the
/// request named.
///
/// The LIMIT is the model's own documentation (see
/// [`MAX_VOICE_CONNECTOR_ITEMS_PER_GROUP`]) and the class is the one the model declares for
/// a limit; the SENTENCE is this emulator's own, for the reason
/// [`voice_connector_group_limit_reached`] gives — no recording of this service shows AWS's
/// wording for a quota refusal.
pub fn voice_connector_items_limit_reached(requested: usize) -> ResourceLimitExceededException {
    ResourceLimitExceededException {
        code: Some("ResourceLimitExceeded".to_string()),
        message: Some(format!(
            "{requested} Voice Connectors were requested for this group, which is more than \
             the maximum of {MAX_VOICE_CONNECTOR_ITEMS_PER_GROUP} per Voice Connector group"
        )),
    }
}

/// `NotFoundException`'s payload for a tagging call whose `ResourceARN` names no resource
/// this service holds. Read by all three generic tagging operations, so the three cannot
/// word one refusal three ways.
pub fn tagged_resource_not_found(resource_arn: &str) -> NotFoundException {
    NotFoundException {
        code: Some("NotFound".to_string()),
        message: Some(format!("resource {resource_arn} does not exist")),
    }
}

/// `ResourceLimitExceededException`'s payload for a tagging call that would take a
/// resource past [`MAX_TAGS_PER_RESOURCE`] tags; `requested` is how many tags the
/// resource would have ended up with.
pub fn tag_limit_reached(resource_arn: &str, requested: usize) -> ResourceLimitExceededException {
    ResourceLimitExceededException {
        code: Some("ResourceLimitExceeded".to_string()),
        message: Some(format!(
            "{requested} tags were requested for {resource_arn}, which is more than the \
             maximum of {MAX_TAGS_PER_RESOURCE} tags per resource"
        )),
    }
}

/// One persisted Voice Connector.
///
/// Mirrors the model's `VoiceConnector` structure, but with the members the service
/// always knows held as non-optional: a stored connector always has an id, a name, a
/// Region, an outbound host name, an encryption flag, an ARN and both timestamps.
/// `IntegrationType`/`NetworkType` stay optional because the create request may omit
/// them. The connector's TAGS are not here: they live in the family's shared ARN-keyed
/// directory ([`chime_tags`]), which is what the generic tagging routes read and write.
#[derive(Debug, Clone)]
pub struct VoiceConnectorRecord {
    pub logging_configuration: Option<crate::ops::types::LoggingConfiguration>,
    pub voice_connector_id: String,
    pub name: String,
    pub aws_region: String,
    pub outbound_host_name: String,
    pub require_encryption: bool,
    pub voice_connector_arn: String,
    pub integration_type: Option<String>,
    pub network_type: Option<String>,
    pub created_timestamp: DateTime,
    pub updated_timestamp: DateTime,
}

impl VoiceConnectorRecord {
    /// Build the record a `CreateVoiceConnector` request asks for. `outbound_host_name`
    /// and `voice_connector_arn` are DERIVED from the minted id (and the Region) rather
    /// than taken from the request, so every operation that returns this resource
    /// reports the same two values.
    pub fn new(
        voice_connector_id: String,
        name: String,
        aws_region: String,
        require_encryption: bool,
        created: DateTime,
    ) -> Self {
        let outbound_host_name = format!("{voice_connector_id}.{OUTBOUND_HOST_SUFFIX}");
        let voice_connector_arn = ::engine::emulator_arn(
            ARN_NAMESPACE,
            &aws_region,
            &format!("vc/{voice_connector_id}"),
        );
        Self {
            voice_connector_id,
            name,
            aws_region,
            outbound_host_name,
            require_encryption,
            voice_connector_arn,
            integration_type: None,
            network_type: None,
            logging_configuration: None,
            created_timestamp: created,
            updated_timestamp: created,
        }
    }

    /// Project the stored record onto the model's `VoiceConnector` shape — the one
    /// conversion every operation that returns a connector goes through, so a create,
    /// a get, a list and an update cannot report the same resource differently.
    pub fn to_shape(&self) -> VoiceConnector {
        VoiceConnector {
            voice_connector_id: Some(self.voice_connector_id.clone()),
            aws_region: Some(self.aws_region.clone()),
            name: Some(self.name.clone()),
            outbound_host_name: Some(self.outbound_host_name.clone()),
            require_encryption: Some(self.require_encryption),
            created_timestamp: Some(self.created_timestamp),
            updated_timestamp: Some(self.updated_timestamp),
            voice_connector_arn: Some(self.voice_connector_arn.clone()),
            integration_type: self.integration_type.clone(),
            network_type: self.network_type.clone(),
        }
    }
}

/// One persisted Voice Connector GROUP.
///
/// Mirrors the model's `VoiceConnectorGroup` structure with the members the service always
/// knows held as non-optional: a stored group always has an id, a name, an ARN and both
/// timestamps. `call_distribution_type` stays optional because the create request may omit
/// it, and the ITEMS are held as the `(VoiceConnectorId, Priority)` pairs the request named,
/// so the group reports the connectors it routes to in the order it was given them.
#[derive(Debug, Clone)]
pub struct VoiceConnectorGroupRecord {
    pub voice_connector_group_id: String,
    pub name: String,
    pub voice_connector_group_arn: String,
    pub call_distribution_type: Option<String>,
    pub voice_connector_items: Vec<(String, i32)>,
    pub created_timestamp: DateTime,
    pub updated_timestamp: DateTime,
}

impl VoiceConnectorGroupRecord {
    /// Build the record a `CreateVoiceConnectorGroup` request asks for. The ARN is DERIVED
    /// from the minted id rather than taken from the request, so every operation that
    /// returns this resource reports the same value.
    ///
    /// The group carries no Region member of its own (`smithy_query(query="shape",
    /// shape="VoiceConnectorGroup")`), and a group may hold connectors from several Regions,
    /// so its ARN is stamped with the Region a connector is created in when a request names
    /// none ([`DEFAULT_AWS_REGION`]).
    pub fn new(
        voice_connector_group_id: String,
        name: String,
        call_distribution_type: Option<String>,
        voice_connector_items: Vec<(String, i32)>,
        created: DateTime,
    ) -> Self {
        let voice_connector_group_arn = ::engine::emulator_arn(
            ARN_NAMESPACE,
            DEFAULT_AWS_REGION,
            &format!("{VOICE_CONNECTOR_GROUP_RESOURCE_TYPE}/{voice_connector_group_id}"),
        );
        Self {
            voice_connector_group_id,
            name,
            voice_connector_group_arn,
            call_distribution_type,
            voice_connector_items,
            created_timestamp: created,
            updated_timestamp: created,
        }
    }

    /// Project the stored record onto the model's `VoiceConnectorGroup` shape — the one
    /// conversion every operation that returns a group goes through, so a create and a get
    /// cannot report the same resource differently.
    pub fn to_shape(&self) -> VoiceConnectorGroup {
        VoiceConnectorGroup {
            voice_connector_group_id: Some(self.voice_connector_group_id.clone()),
            name: Some(self.name.clone()),
            voice_connector_items: Some(
                self.voice_connector_items
                    .iter()
                    .map(|(voice_connector_id, priority)| VoiceConnectorItem {
                        voice_connector_id: voice_connector_id.clone(),
                        priority: Some(*priority),
                    })
                    .collect(),
            ),
            created_timestamp: Some(self.created_timestamp),
            updated_timestamp: Some(self.updated_timestamp),
            voice_connector_group_arn: Some(self.voice_connector_group_arn.clone()),
            call_distribution_type: self.call_distribution_type.clone(),
        }
    }
}

/// Per-service state for com-amazonaws-chimesdkvoice-chimesdktelephonyservice. A handler reaches it through [`service_state`].
#[derive(Debug, Default)]
pub struct ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState {
    /// Every Voice Connector that currently exists, keyed by its `VoiceConnectorId`.
    pub voice_connectors: HashMap<String, VoiceConnectorRecord>,
    /// Counter the minted ids are derived from. Only ever used by
    /// [`Self::mint_voice_connector_id`]; it is NOT an id itself.
    pub next_connector_seq: u64,
    /// Every Voice Connector group that currently exists, keyed by its
    /// `VoiceConnectorGroupId`. Beside the connectors rather than inside them: a group
    /// REFERENCES connectors, and the same connector may be routed to by several groups.
    pub voice_connector_groups: HashMap<String, VoiceConnectorGroupRecord>,
    /// Counter the minted group ids are derived from, the counterpart of
    /// [`Self::next_connector_seq`] for [`Self::mint_voice_connector_group_id`].
    pub next_group_seq: u64,
}

impl ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState {
    /// Mint a fresh `VoiceConnectorId` that no stored connector holds.
    ///
    /// The model constrains the id to `^([a-z0-9]{21,22}|<uuid>)$`, so the id is the
    /// first 21 lowercase-hex characters of a digest of the counter: inside the
    /// character class, the modelled length, and distinct for every counter value. The
    /// caller must insert the record under the returned id while still holding the same
    /// state lock, or the id it returned names nothing.
    pub fn mint_voice_connector_id(&mut self) -> String {
        loop {
            self.next_connector_seq += 1;
            let candidate = voice_connector_id_for(self.next_connector_seq);
            if !self.voice_connectors.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// Mint a fresh `VoiceConnectorGroupId` that no stored group holds.
    ///
    /// The model constrains the id only to `\S` (`NonEmptyString`), so it is minted the
    /// same way a connector's is — 21 lowercase hex characters of a digest of the group
    /// counter — which satisfies that constraint and is distinct for every counter value.
    /// The caller must insert the record under the returned id while still holding the same
    /// state lock, or the id it returned names nothing.
    pub fn mint_voice_connector_group_id(&mut self) -> String {
        loop {
            self.next_group_seq += 1;
            let candidate = voice_connector_group_id_for(self.next_group_seq);
            if !self.voice_connector_groups.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// The FIRST id of `referenced` that no stored Voice Connector holds, in the order the
    /// request named them, or `None` when the store holds every one of them.
    ///
    /// One place answers "does this group's routing reference a connector that exists",
    /// so a create and any later write of the same references cannot disagree.
    pub fn first_absent_voice_connector<'a>(
        &self,
        referenced: impl IntoIterator<Item = &'a str>,
    ) -> Option<String> {
        referenced
            .into_iter()
            .find(|id| !self.voice_connectors.contains_key(*id))
            .map(str::to_string)
    }

    /// Every stored id in the one stable order `ListVoiceConnectors` pages over.
    ///
    /// Sorted by id, which is a total order over the keys themselves: it needs no second
    /// structure beside `voice_connectors` to stay in step with it, so a page boundary
    /// cannot drift from the map it indexes.
    pub fn voice_connector_ids_in_page_order(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.voice_connectors.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Whether a tagging call may address `arn`.
    ///
    /// The family's one rule for that question lives in
    /// [`::service_common::chime_tags::addresses_taggable_resource`]; this store answers
    /// the half of it only this service can — whether an ARN that names a resource THIS
    /// service mints (one of type [`VOICE_CONNECTOR_RESOURCE_TYPE`]) is one it still
    /// holds. An ARN of any other Chime resource type is another family member's, which
    /// this store cannot check.
    pub fn taggable(&self, arn: &str) -> bool {
        let own_resource_exists = ::service_common::arn::resource_id_of_type(
            arn,
            VOICE_CONNECTOR_RESOURCE_TYPE,
        )
        .map(|id| self.voice_connectors.contains_key(id));
        ::service_common::chime_tags::addresses_taggable_resource(arn, own_resource_exists)
    }
}

/// How many characters a minted id carries: the modelled `^[a-z0-9]{21,22}$` alternative's
/// lower bound, which also satisfies the `\S` a group id is constrained to.
const MINTED_ID_LENGTH: usize = 21;

/// The id one counter value mints under `label`: [`MINTED_ID_LENGTH`] lowercase hex
/// characters of a digest of the two.
///
/// ONE derivation for both resources, so the two id forms cannot drift; the LABEL is what
/// keeps them apart, so one counter value cannot mint the same id for a connector and for a
/// group.
fn minted_id(label: &str, seq: u64) -> String {
    use sha2::{Digest as _, Sha256};

    let digest = Sha256::digest(format!("{label}/{seq}").as_bytes());
    let mut id = hex::encode(digest);
    id.truncate(MINTED_ID_LENGTH);
    id
}

/// The id derived from one counter value, which satisfies the modelled
/// `^[a-z0-9]{21,22}$` alternative.
fn voice_connector_id_for(seq: u64) -> String {
    minted_id("chime-sdk-voice/voice-connector", seq)
}

/// The group id derived from one counter value, which satisfies the modelled `\S`
/// constraint on `VoiceConnectorGroupId`.
fn voice_connector_group_id_for(seq: u64) -> String {
    minted_id("chime-sdk-voice/voice-connector-group", seq)
}

/// The wall-clock instant to stamp on a record being created or updated.
pub fn current_timestamp() -> DateTime {
    let since_epoch = ::std::time::SystemTime::now()
        .duration_since(::std::time::UNIX_EPOCH)
        .unwrap_or_default();
    DateTime::from_secs_f64(since_epoch.as_secs_f64())
}

/// Prefix carried inside a pagination token, so a token from somewhere else is rejected
/// rather than read as an id.
const PAGE_TOKEN_PREFIX: &str = "voice-connector:";

/// Encode the opaque `NextToken` that resumes a listing after `last_id`.
///
/// Opaque to the caller (hex of a tagged byte string) because the model gives `NextToken`
/// no structure: a client must pass back what it was handed, not construct one.
pub fn encode_page_token(last_id: &str) -> String {
    hex::encode(format!("{PAGE_TOKEN_PREFIX}{last_id}"))
}

/// The id a page token resumes after, or `None` when the token was not one this service
/// issued.
pub fn decode_page_token(token: &str) -> Option<String> {
    let bytes = hex::decode(token).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    text.strip_prefix(PAGE_TOKEN_PREFIX).map(str::to_string)
}

/// This service's store in the shared engine: call it with a handler's `&*self.engine`.
///
/// Answer `None` with a modelled server error, never with a fresh engine.
pub fn service_state<Eng: Send + Sync + 'static>(
    engine: &Eng,
) -> Option<::engine::ServiceState<ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState>> {
    (engine as &dyn ::std::any::Any)
        .downcast_ref::<::engine::InMemoryEngine>()
        .map(::engine::InMemoryEngine::state::<ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState>)
}

/// The SHARED Chime TAG directory in the shared engine: call it with a handler's
/// `&*self.engine`.
///
/// A resource's tags are keyed by ARN and live in ONE directory
/// ([`::service_common::chime_tags::ChimeTagDirectory`]) rather than on the
/// [`VoiceConnectorRecord`]s of this state struct, because the family's generic tagging
/// routes are served by one member for every member's resource: a copy inside
/// [`ComAmazonawsChimesdkvoiceChimesdktelephonyserviceState`] would be invisible to the
/// sibling serving a tagging call, which would then report "no tags" for a connector that
/// has them.
///
/// LOCK ORDER: a writer takes [`service_state`] first and this directory second, and no
/// reader holds this one while taking another store, so the order cannot invert.
///
/// Answer `None` with a modelled server error, never with a fresh engine.
pub fn chime_tags<Eng: Send + Sync + 'static>(
    engine: &Eng,
) -> Option<::engine::ServiceState<::service_common::chime_tags::ChimeTagDirectory>> {
    (engine as &dyn ::std::any::Any)
        .downcast_ref::<::engine::InMemoryEngine>()
        .map(::engine::InMemoryEngine::state::<::service_common::chime_tags::ChimeTagDirectory>)
}
