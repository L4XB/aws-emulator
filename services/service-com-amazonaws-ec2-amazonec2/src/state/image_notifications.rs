// SPDX-License-Identifier: CC0-1.0

//! Best-effort notification for the private AMI persisted by RegisterImage.
//! AWS documents this operation as one source of an available EC2 AMI State Change.
//! https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/monitor-ami-events.html

use smithy_server_runtime::EmbeddedEmulator;
use tokio::sync::Semaphore;

static SLOTS: Semaphore = Semaphore::const_new(64);

pub(crate) fn publish_registered_image(
    callback: EmbeddedEmulator,
    region: String,
    image_id: String,
) {
    let Ok(slot) = SLOTS.try_acquire() else {
        tracing::warn!(%image_id, "AMI notification publisher is busy");
        return;
    };
    tokio::spawn(async move {
        let _slot = slot;
        let partition = match super::partition_prefix_of(&region) {
            Some("cn-") => "aws-cn",
            Some("us-gov-") => "aws-us-gov",
            Some("us-iso-") => "aws-iso",
            Some("us-isob-") => "aws-iso-b",
            Some("eu-isoe-") => "aws-iso-e",
            Some("us-isof-") => "aws-iso-f",
            _ => "aws",
        };
        let resource = format!("arn:{partition}:ec2:{region}::image/{image_id}");
        let detail = serde_json::json!({
            "ImageId": image_id,
            "State": "available",
            "ErrorMessage": "",
            // As for EBS volume notifications, ec2Query assigns the HTTP response ID after the handler returns.
            "RequestId": service_common::query_error_envelope::new_request_id(),
        });
        let entry = aws_sdk_eventbridge::types::PutEventsRequestEntry::builder()
            .source(super::EC2_EVENT_SOURCE)
            .event_bus_name(super::DEFAULT_EVENT_BUS)
            .detail_type("EC2 AMI State Change")
            .resources(resource)
            .detail(detail.to_string())
            .build();
        let client = service_common::eventbridge_client::eventbridge_client(&callback, &region);
        match client.put_events().entries(entry).send().await {
            Ok(reply) if reply.failed_entry_count() == 0 => {}
            _ => {
                tracing::warn!(%image_id, %region, "Registered AMI notification did not reach EventBridge")
            }
        }
    });
}
