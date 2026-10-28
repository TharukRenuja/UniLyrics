use std::collections::BTreeSet;
use unilyrics::*;

#[test]
fn rich_sources_have_explicit_target_losses_and_restorable_snapshots() {
    let sources: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/conformance.json")).unwrap();
    let matrix: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/conversion_matrix.json")).unwrap();
    let mut checked = 0;
    for row in matrix {
        let source = sources
            .iter()
            .find(|case| case["name"] == row["source_case"])
            .unwrap();
        let document: Document = serde_json::from_value(source["document"].clone()).unwrap();
        let original = document.clone();
        for (name, expected) in row["targets"].as_object().unwrap() {
            let format: Format = serde_json::from_value(name.clone().into()).unwrap();
            let compatible = export(&document, format, Policy::Compatible);
            let strict = export(&document, format, Policy::Strict);
            let preserved = export_preserved(&document, format, &ProjectionOptions::default());
            if let Some(code) = expected["error"].as_str() {
                assert_eq!(compatible.unwrap_err().code, code, "{name}");
                assert_eq!(strict.unwrap_err().code, code, "{name}");
                assert_eq!(preserved.unwrap_err().code, code, "{name}");
            } else {
                let compatible = compatible.unwrap();
                let codes: BTreeSet<_> = compatible.losses.iter().map(|loss| &loss.code).collect();
                let expected: BTreeSet<String> =
                    serde_json::from_value(expected["loss_codes"].clone()).unwrap();
                assert_eq!(
                    codes.into_iter().cloned().collect::<BTreeSet<_>>(),
                    expected,
                    "{name}"
                );
                if compatible.losses.is_empty() {
                    assert_eq!(strict.unwrap().output, compatible.output, "{name}");
                } else {
                    assert_eq!(strict.unwrap_err().code, "LOSSY_CONVERSION", "{name}");
                }
                validate(&parse(&compatible.output, Some(format)).unwrap().document).unwrap();
                let preserved = preserved.unwrap();
                assert_eq!(preserved.output, compatible.output, "{name}");
                assert_eq!(preserved.losses, compatible.losses, "{name}");
                let restored =
                    restore_preserved(&preserved.output, format, &preserved.sidecar).unwrap();
                assert_eq!(restored.document, original, "{name}");
                assert_eq!(
                    restore_preserved(&(preserved.output + "\n"), format, &preserved.sidecar)
                        .unwrap_err()
                        .code,
                    "SIDECAR_MISMATCH"
                );
            }
            assert_eq!(document, original, "{name}: input mutated");
            checked += 1;
        }
    }
    assert_eq!(checked, 16);
}
