// SPDX-License-Identifier: CC0-1.0
use crate::bot_lifecycle::{call, create_body, create_bot, emulator};
use serde_json::json;

#[tokio::test]
async fn settings_follow_the_selected_bot_alias_and_survive_an_omitted_update() {
    let router = emulator();
    let bot = create_bot(&router, &create_body("LocaleWriter", 300, json!({}))).await;
    let other = create_bot(&router, &create_body("OtherWriter", 300, json!({}))).await;
    let aliases = format!("/bots/{bot}/botaliases");
    let settings = json!({"en_US":{"enabled":true,"codeHookSpecification":{"lambdaCodeHook":{
        "lambdaARN":"arn:aws:lambda:us-east-1:123456789012:function:example:1","codeHookInterfaceVersion":"1.0"}}},
        "de_DE":{"enabled":false}});
    let (status, created) = call(
        &router,
        "PUT",
        &aliases,
        Some(json!({"botAliasName":"Preview","botAliasLocaleSettings":settings})),
    )
    .await;
    assert_eq!(status, 202, "{created}");
    assert_eq!(created["botAliasLocaleSettings"], settings);
    assert!(created.get("botVersion").is_none());
    let target = format!("{aliases}/{}", created["botAliasId"].as_str().unwrap());
    let body = json!({"botAliasName":"Preview"});
    let (status, read) = call(&router, "PUT", &target, Some(body.clone())).await;
    assert_eq!(status, 202, "{read}");
    assert_eq!(read["botAliasLocaleSettings"], settings);
    let (status, _) = call(
        &router,
        "PUT",
        &target,
        Some(json!({"botAliasName":"Preview","botVersion":"99","botAliasLocaleSettings":{"en_US":{"enabled":false}}})),
    )
    .await;
    assert_eq!(status, 412);
    let (_, read) = call(&router, "PUT", &target, Some(body)).await;
    assert_eq!(read["botAliasLocaleSettings"], settings);
    let (status, invalid) = call(
        &router,
        "PUT",
        &target,
        Some(json!({"botAliasName":"Preview","botAliasLocaleSettings":{}})),
    )
    .await;
    assert_eq!(status, 400, "{invalid}");
    let test_body = json!({"botAliasName":"TestBotAlias"});
    let (_, configured) = call(&router,"PUT",&format!("{aliases}/TSTALIASID"),Some(json!({"botAliasName":"TestBotAlias","botAliasLocaleSettings":{"en_US":{"enabled":true}}}))).await;
    assert_eq!(configured["botVersion"], "DRAFT");
    assert_eq!(
        configured["botAliasLocaleSettings"],
        json!({"en_US":{"enabled":true}})
    );
    let (status, untouched) = call(
        &router,
        "PUT",
        &format!("/bots/{other}/botaliases/TSTALIASID"),
        Some(test_body),
    )
    .await;
    assert_eq!(status, 202, "{untouched}");
    assert!(untouched.get("botAliasLocaleSettings").is_none());
}
