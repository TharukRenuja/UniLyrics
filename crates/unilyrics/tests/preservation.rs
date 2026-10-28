use unilyrics::{
    export_preserved, parse, process, restore_preserved, Document, Format, Profile,
    ProjectionOptions,
};

fn rich_document() -> Document {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/profiles.json")).unwrap();
    serde_json::from_value(cases[0]["request"]["document"].clone()).unwrap()
}

#[test]
fn rich_snapshot_restores_profile_removals_and_target_losses() {
    let document = rich_document();
    let original = document.clone();
    for format in [
        Format::Plain,
        Format::Lrc,
        Format::EnhancedLrc,
        Format::Json,
        Format::Ttml,
        Format::TtmlLyrics,
        Format::Webvtt,
    ] {
        for profile in [
            Profile::Full,
            Profile::Plain,
            Profile::Line,
            Profile::Translated,
        ] {
            let options = ProjectionOptions {
                profile,
                features: None,
            };
            // Some targets require timing that the selected profile intentionally removes.
            let result = match export_preserved(&document, format, &options) {
                Ok(result) => result,
                Err(error) if matches!(format, Format::Webvtt) => {
                    assert!(
                        matches!(error.code.as_str(), "INVALID_WEBVTT" | "MISSING_TIMING"),
                        "{error}"
                    );
                    continue;
                }
                Err(error) => panic!("{format:?}/{profile:?}: {error}"),
            };
            assert_eq!(result.sidecar.document, original);
            assert_eq!(
                restore_preserved(&result.output, format, &result.sidecar)
                    .unwrap()
                    .document,
                original
            );
            assert_eq!(
                result,
                export_preserved(&document, format, &options).unwrap()
            );
            if profile != Profile::Full {
                assert!(!result.losses.is_empty());
            }
        }
    }
    assert_eq!(document, original);
}

#[test]
fn restored_snapshot_requires_exact_pair_and_valid_model() {
    let document = rich_document();
    let result = export_preserved(&document, Format::Plain, &ProjectionOptions::default()).unwrap();
    assert_eq!(
        restore_preserved(
            &(result.output.clone() + "\n"),
            Format::Plain,
            &result.sidecar
        )
        .unwrap_err()
        .code,
        "SIDECAR_MISMATCH"
    );
    assert_eq!(
        restore_preserved(&result.output, Format::Lrc, &result.sidecar)
            .unwrap_err()
            .code,
        "SIDECAR_MISMATCH"
    );
    let mut sidecar = result.sidecar.clone();
    sidecar.version = 2;
    assert_eq!(
        restore_preserved(&result.output, Format::Plain, &sidecar)
            .unwrap_err()
            .code,
        "UNSUPPORTED_SIDECAR_VERSION"
    );
    sidecar = result.sidecar.clone();
    sidecar.document.version = 99;
    assert!(restore_preserved(&result.output, Format::Plain, &sidecar).is_err());
}

#[test]
fn protocol_bundle_is_self_contained_and_rejects_unknown_fields() {
    let document = rich_document();
    let request = serde_json::json!({"op":"export_preserved", "document":document, "format":"plain", "options":{"profile":"plain"}});
    let response: serde_json::Value = serde_json::from_str(&process(&request.to_string())).unwrap();
    assert_eq!(response["ok"], true);
    assert!(!response["result"]["losses"].as_array().unwrap().is_empty());
    let mut restore = serde_json::json!({"op":"restore_preserved", "input":response["result"]["output"], "format":"plain", "sidecar":response["result"]["sidecar"]});
    let restored: serde_json::Value = serde_json::from_str(&process(&restore.to_string())).unwrap();
    assert_eq!(
        restored["result"]["document"],
        serde_json::to_value(document).unwrap()
    );
    assert_eq!(
        restored["result"]["diagnostics"][0]["code"],
        "SIDECAR_RESTORED"
    );
    restore["sidecar"]["unexpected"] = true.into();
    let rejected: serde_json::Value = serde_json::from_str(&process(&restore.to_string())).unwrap();
    assert_eq!(rejected["error"]["code"], "INVALID_REQUEST");
}

#[test]
fn target_requirements_and_restore_budget_are_enforced() {
    let document = parse("hello", Some(Format::Plain)).unwrap().document;
    assert!(export_preserved(&document, Format::Srt, &ProjectionOptions::default()).is_err());
    let mut document = document;
    document.cues[0].segments[0].text = "x".repeat(400_000);
    assert_eq!(
        export_preserved(&document, Format::Plain, &ProjectionOptions::default())
            .unwrap_err()
            .code,
        "INPUT_TOO_LARGE"
    );
    let document = parse("1\n00:00:01,000 --> 00:00:02,000\nhello", Some(Format::Srt))
        .unwrap()
        .document;
    for format in [Format::Srt, Format::Webvtt] {
        let result = export_preserved(&document, format, &ProjectionOptions::default()).unwrap();
        assert_eq!(
            restore_preserved(&result.output, format, &result.sidecar)
                .unwrap()
                .document,
            document
        );
    }
}
