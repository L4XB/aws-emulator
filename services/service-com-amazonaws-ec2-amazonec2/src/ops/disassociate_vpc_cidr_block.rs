// SPDX-License-Identifier: CC0-1.0

//! @op DisassociateVpcCidrBlock
//! Release one secondary IPv4 or IPv6 range from its owning VPC.
//!
//! The response reports the accepted transition. The in-memory store completes the
//! release under the same lock as its dependency checks, so subsequent allocations
//! and describes agree on which ranges the VPC still owns.
//!
//! # Modeled contract for `DisassociateVpcCidrBlock`
//!
//! What AWS documents for this operation, from this service's Smithy model. HTML is
//! stripped and long entries are clipped at ` …`. For the FULL text of any one item,
//! ask the model rather than reading the raw JSON:
//!
//! ```text
//! smithy_query(query="doc",shape="<Shape>",member="<Member>",model_path="services/service-com-amazonaws-ec2-amazonec2/smithy-model/com-amazonaws-ec2-amazonec2.json")
//! ```
//!
//! Disassociates a CIDR block from a VPC. To disassociate the CIDR block, you must specify
//! its association ID. You can get the association ID by using DescribeVpcs. You must
//! detach or delete all gateways and resources that are associated with the CIDR block
//! before you can disassociate it. You cannot disassociate the CIDR block with which you
//! originally created the VPC (the primary CIDR block).
//!
//! ## Input
//!
//! * `AssociationId` (VpcCidrAssociationId) **required** — The association ID for the CIDR
//!   block.
//!
//! ## Output
//!
//! * `CidrBlockAssociation` (VpcCidrBlockAssociation) — Information about the IPv4 CIDR block
//!   association.
//! * `Ipv6CidrBlockAssociation` (VpcIpv6CidrBlockAssociation) — Information about the IPv6
//!   CIDR block association.
//! * `VpcId` (String) — The ID of the VPC.

#[allow(unused_imports)]
use super::types::*;

// `#[operation_error]` below expands to an invocation of the LOCAL declarative
// `smithy!` macro, which this line is what emits into the module. Without it the
// expansion fails with `cannot find macro 'smithy' in this scope` and the
// `DisassociateVpcCidrBlockError` enum is never produced.
smithy_macros::smithy_model!("smithy-model/com-amazonaws-ec2-amazonec2.json");

// The operation's MODELED errors. `#[operation_error]` accepts only variants the
// model declares for this operation, so the scaffold's `NotImplemented` cannot live
// here — hence the `DisassociateVpcCidrBlockError` alias below. An operation that models no errors
// generates an UNINHABITED enum here, which is correct and expected: there is nothing
// to construct until the model says otherwise.
#[smithy_macros::operation_error("com.amazonaws.ec2#DisassociateVpcCidrBlock")]
#[derive(Debug)]
pub enum DisassociateVpcCidrBlockErrorModeled {}

/// This operation's error channel, as the generated `impl Operation` names it
/// (`crate::ops::disassociate_vpc_cidr_block::DisassociateVpcCidrBlockError`). Wrapping the modeled enum is what
/// lets a scaffolded operation answer 501 for one whose model declares no errors at all.
pub type DisassociateVpcCidrBlockError =
    crate::scaffolded::Scaffolded<DisassociateVpcCidrBlockErrorModeled>;

/// Lets `?` and `.into()` lift a modeled failure into the error channel, so an
/// implemented handler writes `Err(SomeModeledError.into())` rather than naming the
/// wrapper variant. See `crate::scaffolded` for how to drop the wrapper entirely once
/// this operation is implemented.
impl From<DisassociateVpcCidrBlockErrorModeled> for DisassociateVpcCidrBlockError {
    fn from(modeled: DisassociateVpcCidrBlockErrorModeled) -> Self {
        crate::scaffolded::Scaffolded::Modeled(modeled)
    }
}

/// The `DisassociateVpcCidrBlock` operation.
///
/// Owns the shared engine (the constructor seam) and implements
/// [`smithy_server_runtime::Operation`]. Generic over the engine type `Eng`, so this
/// crate names no concrete engine type: a migrated service that needs state picks a
/// concrete engine where it mounts the service, not here.
pub struct DisassociateVpcCidrBlock<Eng> {
    engine: ::std::sync::Arc<Eng>,
}

impl<Eng> DisassociateVpcCidrBlock<Eng> {
    /// Construct the operation from the shared engine. INFALLIBLE — the engine is a
    /// construction-time dependency, so engine access is guaranteed by the time any
    /// request runs. Called by the generated `EmulatedService::new(engine)` constructor.
    pub fn new(engine: ::std::sync::Arc<Eng>) -> Self {
        Self { engine }
    }
}

impl<Eng: ::std::marker::Send + ::std::marker::Sync + 'static> smithy_server_runtime::Operation
    for DisassociateVpcCidrBlock<Eng>
{
    type Input = DisassociateVpcCidrBlockInput;
    type Output = DisassociateVpcCidrBlockOutput;
    type Error = DisassociateVpcCidrBlockError;

    const NAME: &'static str = "DisassociateVpcCidrBlock";
    const INPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::DISASSOCIATE_VPC_CIDR_BLOCK_INPUT_SCHEMA;
    const OUTPUT_SCHEMA: &'static ::aws_smithy_schema::Schema =
        super::types::DISASSOCIATE_VPC_CIDR_BLOCK_OUTPUT_SCHEMA;
    const SUCCESS_STATUS: u16 = 200u16;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        use crate::scaffolded::Scaffolded;
        use crate::state;

        let region = state::request_region(ctx.parts());
        let shared = state::require_service_state(&*self.engine)?;
        let mut store = shared.lock();
        store.materialize_due_transitions();

        let association_id = input.association_id.as_str();
        let found = store.vpcs.iter().find_map(|(vpc_id, vpc)| {
            if !store.vpc_is_in_region(Some(vpc_id), &region) {
                return None;
            }
            let ipv4 = vpc
                .cidr_block_association_set
                .iter()
                .flatten()
                .find(|a| a.association_id.as_deref() == Some(association_id))
                .and_then(|a| a.cidr_block.as_ref())
                .map(|cidr| (cidr, false));
            let ipv6 = vpc
                .ipv6_cidr_block_association_set
                .iter()
                .flatten()
                .find(|a| a.association_id.as_deref() == Some(association_id))
                .and_then(|a| a.ipv6_cidr_block.as_ref())
                .map(|cidr| (cidr, true));
            ipv4.or(ipv6).map(|(cidr, ipv6)| {
                (
                    vpc_id.clone(),
                    cidr.clone(),
                    ipv6,
                    !ipv6 && vpc.cidr_block.as_ref() == Some(cidr),
                )
            })
        });
        let Some((vpc_id, cidr, ipv6, primary)) = found else {
            return Err(Scaffolded::service_error(
                "InvalidVpcCidrBlockAssociationID.NotFound",
                400,
                format!("The VPC CIDR block association '{association_id}' does not exist."),
            ));
        };
        if primary {
            return Err(Scaffolded::service_error("OperationNotPermitted", 400,
                format!("The primary IPv4 CIDR block '{cidr}' cannot be disassociated from VPC '{vpc_id}'.")));
        }
        let in_use = store
            .subnets
            .values()
            .filter(|subnet| subnet.vpc_id.as_ref() == Some(&vpc_id))
            .any(|subnet| {
                if ipv6 {
                    subnet
                        .ipv6_cidr_block_association_set
                        .iter()
                        .flatten()
                        .any(|a| {
                            state::cidr_association_holds_its_block(
                                a.ipv6_cidr_block_state
                                    .as_ref()
                                    .and_then(|s| s.state.as_deref()),
                            ) && a
                                .ipv6_cidr_block
                                .as_deref()
                                .is_some_and(|s| state::ipv6_cidr_blocks_overlap(&cidr, s))
                        })
                } else {
                    subnet
                        .cidr_block
                        .as_deref()
                        .is_some_and(|s| state::ipv4_cidr_blocks_overlap(&cidr, s))
                }
            });
        if in_use {
            return Err(state::dependency_violation(format!(
                "The VPC CIDR block association '{association_id}' has dependencies and cannot be disassociated.")));
        }

        let transition = Some(VpcCidrBlockState {
            state: Some(state::CIDR_BLOCK_STATE_DISASSOCIATING.to_string()),
            status_message: None,
        });
        let mut output = DisassociateVpcCidrBlockOutput {
            vpc_id: Some(vpc_id.clone()),
            ..Default::default()
        };
        let vpc = store
            .vpcs
            .get_mut(&vpc_id)
            .expect("selected under the same lock");
        if ipv6 {
            let entries = vpc
                .ipv6_cidr_block_association_set
                .as_mut()
                .expect("selected IPv6 collection");
            let index = entries
                .iter()
                .position(|a| a.association_id.as_deref() == Some(association_id))
                .expect("selected association");
            let mut released = entries.remove(index);
            released.ipv6_cidr_block_state = transition;
            output.ipv6_cidr_block_association = Some(released);
        } else {
            let entries = vpc
                .cidr_block_association_set
                .as_mut()
                .expect("selected IPv4 collection");
            let index = entries
                .iter()
                .position(|a| a.association_id.as_deref() == Some(association_id))
                .expect("selected association");
            let mut released = entries.remove(index);
            released.cidr_block_state = transition;
            output.cidr_block_association = Some(released);
        }
        for table in store
            .route_tables
            .values_mut()
            .filter(|table| table.vpc_id.as_ref() == Some(&vpc_id))
        {
            if let Some(routes) = &mut table.routes {
                routes.retain(|route| {
                    let destination = if ipv6 {
                        &route.destination_ipv6_cidr_block
                    } else {
                        &route.destination_cidr_block
                    };
                    !(route.origin.as_deref() == Some("CreateRouteTable")
                        && destination.as_ref() == Some(&cidr))
                });
            }
        }
        Ok(output)
    }
}
