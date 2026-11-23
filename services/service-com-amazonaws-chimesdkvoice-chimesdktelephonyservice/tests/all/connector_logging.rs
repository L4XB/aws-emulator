// SPDX-License-Identifier: CC0-1.0

use super::voice_connector_lifecycle::{call, emulator};
use serde_json::json;

#[tokio::test]
async fn logging_configuration_is_bound_to_the_connector() {
    let router = emulator();
    let mut paths = Vec::new();
    for name in ["logging-first", "logging-second"] {
        let reply = call(
            &router,
            "POST",
            "/voice-connectors",
            Some(json!({"Name": name, "RequireEncryption": true})),
        )
        .await;
        assert_eq!(reply.status, 201);
        let body = reply.json("create");
        let id = body["VoiceConnector"]["VoiceConnectorId"].as_str().unwrap();
        paths.push(format!("/voice-connectors/{id}/logging-configuration"));
    }
    for (index, sip, media) in [(0, true, false), (1, true, true), (0, false, true)] {
        let config =
            json!({"LoggingConfiguration":{"EnableSIPLogs":sip,"EnableMediaMetricLogs":media}});
        let reply = call(&router, "PUT", &paths[index], Some(config.clone())).await;
        assert_eq!(reply.status, 200);
        assert_eq!(reply.json("put"), config);
        let read = call(&router, "GET", &paths[index], None).await;
        assert_eq!(read.status, 200);
        assert_eq!(read.json("get"), config);
    }
    let second = call(&router, "GET", &paths[1], None).await;
    assert_eq!(
        second.json("second")["LoggingConfiguration"],
        json!({"EnableSIPLogs":true,"EnableMediaMetricLogs":true})
    );
    let deleted = paths[0].strip_suffix("/logging-configuration").unwrap();
    assert_eq!(call(&router, "DELETE", deleted, None).await.status, 204);
    for (method, body) in [
        ("GET", None),
        (
            "PUT",
            Some(json!({"LoggingConfiguration":{"EnableSIPLogs":true}})),
        ),
    ] {
        let reply = call(&router, method, &paths[0], body).await;
        assert_eq!(reply.status, 404);
        assert_eq!(reply.error_code("deleted"), "NotFoundException");
    }
}
