use unilyrics::*;

#[test]
fn shared_normalization_fixtures_and_atomicity() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/normalization.json")).unwrap();
    for case in cases {
        let document: Document = serde_json::from_value(case["document"].clone()).unwrap();
        let original = document.clone();
        let offset = case["offset_us"].as_i64().unwrap_or(0);
        let result = normalize(&document, offset);
        let request =
            serde_json::json!({"op":"normalize", "document":document, "offset_us":offset});
        let reply: serde_json::Value =
            serde_json::from_str(&process(&request.to_string())).unwrap();
        assert_eq!(document, original, "input mutated: {}", case["name"]);
        if let Some(code) = case["error"].as_str() {
            assert_eq!(result.unwrap_err().code, code, "{}", case["name"]);
            assert_eq!(reply["error"]["code"], code);
        } else {
            let result = result.unwrap();
            assert_eq!(
                serde_json::to_value(&result.document).unwrap(),
                case["expected_document"],
                "{}",
                case["name"]
            );
            assert_eq!(
                serde_json::to_value(
                    result
                        .diagnostics
                        .iter()
                        .map(|d| &d.code)
                        .collect::<Vec<_>>()
                )
                .unwrap(),
                case["diagnostic_codes"]
            );
            assert_eq!(reply["result"], serde_json::to_value(result).unwrap());
        }
    }
}

#[test]
fn invalid_offsets_and_requests() {
    let document = Document::default();
    assert_eq!(
        normalize(&document, i64::MIN).unwrap_err().code,
        "INVALID_OFFSET"
    );
    assert_eq!(
        normalize(&document, i64::MAX).unwrap_err().code,
        "INVALID_OFFSET"
    );
    for offset in [
        serde_json::json!(true),
        serde_json::json!(0.5),
        serde_json::json!("1"),
    ] {
        let request =
            serde_json::json!({"op":"normalize", "document":document, "offset_us":offset});
        let reply: serde_json::Value =
            serde_json::from_str(&process(&request.to_string())).unwrap();
        assert_eq!(reply["error"]["code"], "INVALID_REQUEST");
    }
}
