use unilyrics::*;

#[test]
fn shared_profiles_and_wire_contract() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/profiles.json")).unwrap();
    for case in cases {
        let request = &case["request"];
        let document: Document = serde_json::from_value(request["document"].clone()).unwrap();
        let original = document.clone();
        let reply: serde_json::Value =
            serde_json::from_str(&process(&request.to_string())).unwrap();
        if let Some(code) = case["error"].as_str() {
            assert_eq!(reply["error"]["code"], code, "{}", case["name"]);
        } else {
            assert_eq!(reply["ok"], true, "{}: {reply}", case["name"]);
            let result = &reply["result"];
            let value = if request["op"] == "project" {
                result["document"].clone()
            } else {
                serde_json::from_str::<serde_json::Value>(result["output"].as_str().unwrap())
                    .unwrap()
            };
            let expected = case
                .get("expected_document")
                .or_else(|| case.get("output_document"))
                .unwrap();
            assert_eq!(&value, expected, "{}", case["name"]);
            let codes: Vec<_> = result
                .get("diagnostics")
                .or_else(|| result.get("losses"))
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["code"].clone())
                .collect();
            assert_eq!(serde_json::json!(codes), case["codes"], "{}", case["name"]);
            let options = serde_json::from_value(
                request
                    .get("options")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            )
            .unwrap();
            if request["op"] == "project" {
                assert_eq!(
                    serde_json::to_value(project(&document, &options).unwrap()).unwrap(),
                    *result
                );
            } else {
                let policy = serde_json::from_value(
                    request
                        .get("policy")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("compatible")),
                )
                .unwrap();
                assert_eq!(
                    serde_json::to_value(
                        export_profile(&document, Format::Json, &options, policy).unwrap()
                    )
                    .unwrap(),
                    *result
                );
            }
        }
        assert_eq!(document, original);
    }
}

#[test]
fn profiles_never_invent_and_aggregate_target_losses() {
    let original = parse("Hello", Some(Format::Plain)).unwrap().document;
    let options = ProjectionOptions {
        profile: Profile::Word,
        features: None,
    };
    let result = project(&original, &options).unwrap();
    assert_eq!(result.document, original);
    assert!(result.diagnostics.is_empty());
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/profiles.json")).unwrap();
    let document = serde_json::from_value(cases[0]["request"]["document"].clone()).unwrap();
    let result = export_profile(
        &document,
        Format::Plain,
        &ProjectionOptions {
            profile: Profile::Plain,
            features: None,
        },
        Policy::Compatible,
    )
    .unwrap();
    assert!(result
        .losses
        .iter()
        .any(|loss| loss.code == "PROFILE_METADATA_DROPPED"));
    assert!(result
        .losses
        .iter()
        .any(|loss| loss.code == "CUE_IDS_CHANGED"));
    assert_eq!(result.output, "සිංහල 👩🏽‍💻 missing\nback");
    let invalid = Document {
        version: 999,
        ..Document::default()
    };
    assert_eq!(
        project(&invalid, &ProjectionOptions::default())
            .unwrap_err()
            .code,
        "UNSUPPORTED_VERSION"
    );
}
