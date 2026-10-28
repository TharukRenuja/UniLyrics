use unilyrics::*;

struct WrappedJSON;
impl CustomFormat for WrappedJSON {
    fn matches(&self, input: &str) -> bool {
        input.starts_with("CUSTOM\n")
    }
    fn parse(&self, input: &str) -> Result<ParseResult, Error> {
        parse(
            input
                .strip_prefix("CUSTOM\n")
                .ok_or_else(|| Error::new("BAD_CUSTOM_HEADER", "Missing CUSTOM header."))?,
            Some(Format::Json),
        )
    }
    fn export(&self, document: &Document, _: Policy) -> Result<ExportResult, Error> {
        Ok(ExportResult {
            output: format!(
                "CUSTOM\n{}",
                export(document, Format::Json, Policy::Strict)?.output
            ),
            losses: vec![],
        })
    }
}

#[test]
fn rich_roundtrip_and_scoped_builtin_fallback() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/conformance.json")).unwrap();
    let mut registry = Registry::default();
    registry.register("x-wrapped-json", WrappedJSON).unwrap();
    for case in cases
        .into_iter()
        .filter(|case| case.get("document").is_some() && case.get("error").is_none())
    {
        let document: Document = serde_json::from_value(case["document"].clone()).unwrap();
        let original = document.clone();
        let result = registry
            .export(&document, "x-wrapped-json", Policy::Strict)
            .unwrap();
        assert!(result.losses.is_empty());
        assert_eq!(registry.detect(&result.output).unwrap(), "x-wrapped-json");
        assert_eq!(
            registry.parse(&result.output, None).unwrap().document,
            document
        );
        assert_eq!(
            registry
                .parse(&result.output, Some("x-wrapped-json"))
                .unwrap()
                .document,
            document
        );
        assert_eq!(document, original);
    }
    assert_eq!(registry.detect("[00:01]hi").unwrap(), "lrc");
    assert_eq!(
        registry.parse("hi", Some("plain")).unwrap(),
        parse("hi", Some(Format::Plain)).unwrap()
    );
    assert_eq!(
        registry
            .export(&Document::default(), "json", Policy::Strict)
            .unwrap(),
        export(&Document::default(), Format::Json, Policy::Strict).unwrap()
    );
    assert_eq!(
        Registry::default()
            .parse("CUSTOM\n{}", Some("x-wrapped-json"))
            .unwrap_err()
            .code,
        "UNKNOWN_FORMAT"
    );
}

#[test]
fn names_duplicates_and_ambiguous_detection() {
    let mut registry = Registry::default();
    for name in [
        "plain",
        "x-",
        "X-custom",
        "x-💻",
        &format!("x-{}", "a".repeat(63)),
    ] {
        assert_eq!(
            registry.register(name, WrappedJSON).unwrap_err().code,
            "INVALID_CUSTOM_FORMAT"
        );
    }
    registry.register("x-one", WrappedJSON).unwrap();
    assert_eq!(
        registry.register("x-one", WrappedJSON).unwrap_err().code,
        "DUPLICATE_FORMAT"
    );
    registry.register("x-two", WrappedJSON).unwrap();
    assert_eq!(
        registry.detect("CUSTOM\n{}").unwrap_err().code,
        "AMBIGUOUS_FORMAT"
    );
    assert_eq!(
        registry
            .parse("CUSTOM\n{\"version\":1,\"cues\":[]}", Some("x-one"))
            .unwrap()
            .document,
        Document::default()
    );
    struct Broad;
    impl CustomFormat for Broad {
        fn matches(&self, _: &str) -> bool {
            true
        }
    }
    let mut broad = Registry::default();
    broad.register("x-broad", Broad).unwrap();
    assert_eq!(
        broad.detect("[00:01]hi").unwrap_err().code,
        "AMBIGUOUS_FORMAT"
    );
    assert_eq!(
        broad.parse("[00:01]hi", Some("lrc")).unwrap().document.cues[0].start_us,
        Some(1_000_000)
    );
    assert_eq!(
        broad
            .export(&Document::default(), "x-broad", Policy::Compatible)
            .unwrap_err()
            .code,
        "UNSUPPORTED_OPERATION"
    );
}

#[test]
fn validation_limits_and_unproven_exports() {
    struct Bad;
    impl CustomFormat for Bad {
        fn parse(&self, _: &str) -> Result<ParseResult, Error> {
            Ok(ParseResult {
                document: Document {
                    version: 999,
                    ..Document::default()
                },
                diagnostics: vec![],
            })
        }
        fn export(&self, _: &Document, _: Policy) -> Result<ExportResult, Error> {
            Ok(ExportResult {
                output: "bad".into(),
                losses: vec![],
            })
        }
    }
    let mut registry = Registry::default();
    registry.register("x-bad", Bad).unwrap();
    assert!(registry.parse("hi", Some("x-bad")).is_err());
    assert_eq!(
        registry
            .parse(&"a".repeat(MAX_INPUT_BYTES + 1), Some("x-bad"))
            .unwrap_err()
            .code,
        "INPUT_TOO_LARGE"
    );
    let result = registry
        .export(&Document::default(), "x-bad", Policy::Compatible)
        .unwrap();
    assert_eq!(result.losses[0].code, "CUSTOM_ROUNDTRIP_UNVERIFIED");
    assert_eq!(
        registry
            .export(&Document::default(), "x-bad", Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    struct Changed;
    impl CustomFormat for Changed {
        fn parse(&self, _: &str) -> Result<ParseResult, Error> {
            parse("", Some(Format::Plain))
        }
        fn export(&self, _: &Document, _: Policy) -> Result<ExportResult, Error> {
            Ok(ExportResult {
                output: "".into(),
                losses: vec![],
            })
        }
    }
    registry.register("x-changed", Changed).unwrap();
    assert_eq!(
        registry
            .export(&Document::default(), "x-changed", Policy::Compatible)
            .unwrap()
            .losses[0]
            .code,
        "CUSTOM_ROUNDTRIP_CHANGED"
    );
    assert_eq!(
        registry
            .export(&Document::default(), "x-changed", Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    struct ExportOnly;
    impl CustomFormat for ExportOnly {
        fn export(&self, _: &Document, _: Policy) -> Result<ExportResult, Error> {
            Ok(ExportResult {
                output: "opaque".into(),
                losses: vec![],
            })
        }
    }
    registry.register("x-output", ExportOnly).unwrap();
    assert_eq!(
        registry
            .export(&Document::default(), "x-output", Policy::Compatible)
            .unwrap()
            .losses[0]
            .code,
        "CUSTOM_ROUNDTRIP_UNVERIFIED"
    );
    assert_eq!(
        registry
            .export(&Document::default(), "x-output", Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
}

#[test]
fn exporter_limits_and_own_losses_cannot_bypass_strict_policy() {
    struct TooLarge;
    impl CustomFormat for TooLarge {
        fn parse(&self, _: &str) -> Result<ParseResult, Error> {
            panic!("oversized output must be rejected first")
        }
        fn export(&self, _: &Document, _: Policy) -> Result<ExportResult, Error> {
            Ok(ExportResult {
                output: "x".repeat(MAX_INPUT_BYTES + 1),
                losses: vec![],
            })
        }
    }
    struct ReportedLoss;
    impl CustomFormat for ReportedLoss {
        fn parse(&self, input: &str) -> Result<ParseResult, Error> {
            WrappedJSON.parse(input)
        }
        fn export(&self, document: &Document, policy: Policy) -> Result<ExportResult, Error> {
            let mut result = WrappedJSON.export(document, policy)?;
            result.losses.push(Diagnostic::new(
                "CUSTOM_REPORTED_LOSS",
                "Adapter reports unsupported data.",
                None,
            ));
            Ok(result)
        }
    }
    let mut registry = Registry::default();
    registry.register("x-large", TooLarge).unwrap();
    registry.register("x-loss", ReportedLoss).unwrap();
    assert_eq!(
        registry
            .export(&Document::default(), "x-large", Policy::Compatible)
            .unwrap_err()
            .code,
        "INPUT_TOO_LARGE"
    );
    assert_eq!(
        registry
            .export(&Document::default(), "x-loss", Policy::Compatible)
            .unwrap()
            .losses[0]
            .code,
        "CUSTOM_REPORTED_LOSS"
    );
    assert_eq!(
        registry
            .export(&Document::default(), "x-loss", Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    let invalid = Document {
        version: 999,
        ..Document::default()
    };
    assert!(registry.export(&invalid, "x-loss", Policy::Strict).is_err());
    assert_eq!(
        registry
            .detect(&"x".repeat(MAX_INPUT_BYTES + 1))
            .unwrap_err()
            .code,
        "INPUT_TOO_LARGE"
    );
}

#[test]
fn host_callback_verification_protocol() {
    let document = Document::default();
    let result = ExportResult {
        output: "opaque".into(),
        losses: vec![],
    };
    for (op, value) in [
        (
            "validate_custom_parse",
            serde_json::json!({"document":document,"diagnostics":[]}),
        ),
        (
            "validate_custom_export",
            serde_json::to_value(&result).unwrap(),
        ),
    ] {
        let reply: serde_json::Value = serde_json::from_str(&process(
            &serde_json::json!({"op":op,"result":value}).to_string(),
        ))
        .unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"], value);
        let mut invalid = value;
        invalid["unknown"] = serde_json::json!(true);
        let reply: serde_json::Value = serde_json::from_str(&process(
            &serde_json::json!({"op":op,"result":invalid}).to_string(),
        ))
        .unwrap();
        assert_eq!(reply["error"]["code"], "INVALID_REQUEST");
    }
    for (roundtrip, policy, code) in [
        (Some(document.clone()), Policy::Strict, None),
        (
            None,
            Policy::Compatible,
            Some("CUSTOM_ROUNDTRIP_UNVERIFIED"),
        ),
        (
            Some(Document {
                version: 999,
                ..Document::default()
            }),
            Policy::Compatible,
            Some("CUSTOM_ROUNDTRIP_UNVERIFIED"),
        ),
    ] {
        let reply: serde_json::Value = serde_json::from_str(&process(&serde_json::json!({"op":"verify_custom_export","document":document,"result":result,"roundtrip":roundtrip,"policy":policy}).to_string())).unwrap();
        assert_eq!(reply["ok"], true);
        match code {
            Some(code) => assert_eq!(reply["result"]["losses"][0]["code"], code),
            None => assert_eq!(reply["result"]["losses"], serde_json::json!([])),
        }
    }
    let reply: serde_json::Value = serde_json::from_str(&process(&serde_json::json!({"op":"verify_custom_export","document":document,"result":result,"policy":"strict"}).to_string())).unwrap();
    assert_eq!(reply["error"]["code"], "LOSSY_CONVERSION");
}
