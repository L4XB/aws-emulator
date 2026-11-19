// SPDX-License-Identifier: CC0-1.0

//! IaC-Eval fork (P3, DERIVED, NOT DOCUMENTED): `CreateDBInstance` with an EMPTY
//! `EngineVersion` (`EngineVersion=` on the wire) resolves like one that names none.
//!
//! The AWS provider for Terraform sends `EngineVersion=` for every `aws_db_instance` without
//! `engine_version` (internal/service/rds/instance.go v5.100.0,
//! `EngineVersion: aws.String(d.Get(names.AttrEngineVersion).(string))`). Upstream read the
//! empty value as the version "" and refused it ("Cannot find version  for mysql"); an
//! absent value already resolved to the engine's default version, which is what
//! `DescribeDBEngineVersions` with `DefaultOnly` publishes ("If no version is specified,
//! Amazon RDS defaults to a supported version, typically the most recent version", Amazon RDS
//! User Guide, *Supported MySQL major versions on Amazon RDS*).
//!
//! The rows differ along the engine (single-number and two-part major versions) and pin that
//! a value naming no version is still refused.

use crate::db_instances::{call, emulator, expect_error, expect_ok, optional_text, subtrees, text};

/// A compute class every engine below is orderable in.
const CLASS: &str = "db.t3.medium";

/// The version `DescribeDBEngineVersions(DefaultOnly)` publishes for `engine`.
async fn published_default(
    router: &smithy_server_runtime::MultiServiceRouter,
    engine: &str,
) -> String {
    let body = expect_ok(
        engine,
        call(
            router,
            "DescribeDBEngineVersions",
            &[("Engine", engine), ("DefaultOnly", "true")],
        )
        .await,
    );
    let found = subtrees(&body, "DBEngineVersion");
    assert_eq!(
        found.len(),
        1,
        "case {engine:?}: DefaultOnly must report exactly one version, got {body}"
    );
    text(&found[0], "EngineVersion")
}

#[tokio::test]
async fn an_empty_engine_version_resolves_to_the_engines_default() {
    let router = emulator();
    for (index, engine) in ["mysql", "postgres", "mariadb"].into_iter().enumerate() {
        let default_version = published_default(&router, engine).await;
        for (form, value) in [("empty", Some("")), ("absent", None)] {
            let case = format!("{engine}, EngineVersion {form}");
            let identifier = format!("fork-empty-version-{index}-{form}");
            let mut params: Vec<(&str, &str)> = vec![
                ("DBInstanceIdentifier", identifier.as_str()),
                ("Engine", engine),
                ("DBInstanceClass", CLASS),
                ("MasterUsername", "admin"),
                ("MasterUserPassword", "a-long-enough-password"),
                ("AllocatedStorage", "20"),
            ];
            if let Some(value) = value {
                params.push(("EngineVersion", value));
            }
            let created = expect_ok(&case, call(&router, "CreateDBInstance", &params).await);
            let instances = subtrees(&created, "DBInstance");
            assert_eq!(instances.len(), 1, "case {case:?}: one DBInstance expected in {created}");
            let reported = optional_text(&instances[0], "EngineVersion");
            assert_eq!(
                reported.as_deref(),
                Some(default_version.as_str()),
                "case {case:?}: expected the published default {default_version:?}, got \
                 {reported:?}"
            );
        }
    }

    // A value that names no version is still refused, as upstream refuses it.
    let refused = call(
        &router,
        "CreateDBInstance",
        &[
            ("DBInstanceIdentifier", "fork-bogus-version"),
            ("Engine", "mysql"),
            ("DBInstanceClass", CLASS),
            ("MasterUsername", "admin"),
            ("MasterUserPassword", "a-long-enough-password"),
            ("AllocatedStorage", "20"),
            ("EngineVersion", "0.0.0"),
        ],
    )
    .await;
    expect_error("bogus version", refused, 400, "InvalidParameterCombination");
}
