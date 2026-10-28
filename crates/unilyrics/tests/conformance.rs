use unilyrics::*;

#[test]
fn shared_fixtures() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/conformance.json")).unwrap();
    for case in cases {
        let format: Format = serde_json::from_value(case["format"].clone()).unwrap();
        let input = case["input"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| case["document"].to_string());
        let result = parse(&input, Some(format));
        if let Some(code) = case["error"].as_str() {
            assert_eq!(result.unwrap_err().code, code, "{}", case["name"]);
            continue;
        }
        let parsed = result.unwrap();
        if let Some(exports) = case["exports"].as_array() {
            for expected in exports {
                let format = serde_json::from_value(expected["format"].clone()).unwrap();
                let policy = serde_json::from_value(expected["policy"].clone()).unwrap();
                let encoded = export(&parsed.document, format, policy).unwrap();
                assert_eq!(encoded.output, expected["output"].as_str().unwrap());
                assert!(encoded.losses.is_empty());
            }
        }
        if case.get("document").is_some() {
            assert_eq!(
                serde_json::to_value(&parsed.document).unwrap(),
                case["document"]
            );
            assert_eq!(
                serde_json::to_value(inspect(&parsed.document).unwrap()).unwrap(),
                case["features"]
            );
            let plain = export(&parsed.document, Format::Plain, Policy::Compatible).unwrap();
            let actual: std::collections::BTreeSet<_> =
                plain.losses.iter().map(|loss| loss.code.as_str()).collect();
            let expected: std::collections::BTreeSet<_> = case["loss_codes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|code| code.as_str().unwrap())
                .collect();
            assert_eq!(actual, expected);
            assert_eq!(
                export(&parsed.document, Format::Plain, Policy::Strict)
                    .unwrap_err()
                    .code,
                "LOSSY_CONVERSION"
            );
        }
        let text: Vec<_> = parsed.document.cues.iter().map(Cue::text).collect();
        assert_eq!(serde_json::json!(text), case["text"], "{}", case["name"]);
        let starts: Vec<_> = parsed.document.cues.iter().map(|c| c.start_us).collect();
        assert_eq!(
            serde_json::json!(starts),
            case["starts"],
            "{}",
            case["name"]
        );
        let json = export(&parsed.document, Format::Json, Policy::Strict).unwrap();
        assert_eq!(
            parse(&json.output, Some(Format::Json)).unwrap().document,
            parsed.document
        );
    }
}

#[test]
fn exact_segment_boundaries_and_gap_round_trip() {
    let input = "[00:00]<00:00>你<00:00>好 <00:01>bro<00:01.2>ken<00:02><00:03>!<00:04>";
    let document = parse(input, Some(Format::EnhancedLrc)).unwrap().document;
    assert_eq!(document.cues[0].text(), "你好 broken!");
    assert_eq!(document.cues[0].segments.len(), 5);
    assert_eq!(document.cues[0].segments[3].end_us, Some(2_000_000));
    assert_eq!(document.cues[0].segments[4].start_us, Some(3_000_000));
    let output = export(&document, Format::EnhancedLrc, Policy::Strict).unwrap();
    assert_eq!(
        parse(&output.output, Some(Format::EnhancedLrc))
            .unwrap()
            .document,
        document
    );
}

#[test]
fn strict_and_compatible_losses_do_not_mutate() {
    let document = parse("[ar:Artist]\n[00:01]<00:01>Hello <00:02>world<00:03>", None)
        .unwrap()
        .document;
    let original = document.clone();
    assert_eq!(
        export(&document, Format::Plain, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    let result = export(&document, Format::Plain, Policy::Compatible).unwrap();
    assert_eq!(result.output, "Hello world");
    assert!(result.losses.iter().any(|d| d.code == "METADATA_DROPPED"));
    assert!(result.losses.iter().any(|d| d.code == "TIMING_DROPPED"));
    assert_eq!(document, original);
    assert_eq!(
        export(&document, Format::Lrc, Policy::Compatible)
            .unwrap()
            .losses[0]
            .code,
        "SEGMENT_TIMING_DROPPED"
    );
}

#[test]
fn literal_markup_and_whitespace() {
    let doc = parse("[00:01]<00:01> A <b>你</b> <00:02>好  ", None)
        .unwrap()
        .document;
    assert_eq!(doc.cues[0].text(), " A <b>你</b> 好  ");
    let input = "  literal\r\n\nend\n";
    let doc = parse(input, Some(Format::Plain)).unwrap().document;
    assert_eq!(
        export(&doc, Format::Plain, Policy::Strict).unwrap().output,
        input
    );
}

#[test]
fn malformed_and_versioned_requests() {
    let response: serde_json::Value =
        serde_json::from_str(&process(r#"{"op":"parse","input":"x","surprise":true}"#)).unwrap();
    assert_eq!(response["error"]["code"], "INVALID_REQUEST");
    let document = Document {
        version: 99,
        ..Document::default()
    };
    assert_eq!(validate(&document).unwrap_err().code, "UNSUPPORTED_VERSION");
    assert!(parse(&"x".repeat(MAX_INPUT_BYTES + 1), None).is_err());
    assert_eq!(detect("<tt/>").unwrap_err().code, "UNSUPPORTED_FORMAT");
    assert_eq!(detect("[00:72]bad").unwrap_err().code, "INVALID_TIMESTAMP");
}

#[test]
fn duplicates_metadata_offsets_and_parallel_cues() {
    let result = parse(
        "[ar:A]\n[ar:B]\n[offset:25]\n[00:01]Lead\n[00:01]Background",
        Some(Format::Lrc),
    )
    .unwrap();
    assert_eq!(result.document.metadata.len(), 3);
    assert_eq!(result.document.cues.len(), 2);
    assert_eq!(result.document.cues[0].start_us, Some(1_000_000));
    assert_eq!(result.document.cues[1].start_us, Some(1_000_000));
    assert_eq!(result.diagnostics[0].code, "OFFSET_NOT_APPLIED");
    let mut invalid = result.document;
    invalid.cues[1].id = invalid.cues[0].id.clone();
    assert_eq!(validate(&invalid).unwrap_err().code, "INVALID_ID");
}

#[test]
fn empty_markers_and_reserved_text() {
    for input in ["[00:01]<00:02>", "[00:01]<00:02><00:03>"] {
        let document = parse(input, Some(Format::EnhancedLrc)).unwrap().document;
        assert!(!document.cues[0].segments.is_empty());
        let output = export(&document, Format::EnhancedLrc, Policy::Strict).unwrap();
        assert_eq!(
            parse(&output.output, Some(Format::EnhancedLrc))
                .unwrap()
                .document,
            document
        );
    }
    for text in ["[00:01]literal", "literal <00:01> marker"] {
        let document = parse(text, Some(Format::Plain)).unwrap().document;
        assert_eq!(
            export(&document, Format::EnhancedLrc, Policy::Compatible)
                .unwrap_err()
                .code,
            "UNREPRESENTABLE_TEXT"
        );
    }
    assert_eq!(detect("[ar:Artist]").unwrap(), Format::Lrc);
    assert_eq!(detect("{broken").unwrap_err().code, "INVALID_JSON");
}

#[test]
fn explicit_ends_and_partial_timing_do_not_silently_change() {
    let mut doc = parse("[00:01]<00:01>A<00:02>B", Some(Format::EnhancedLrc))
        .unwrap()
        .document;
    doc.cues[0].segments[0].end_us = Some(2_000_000);
    let exported = export(&doc, Format::EnhancedLrc, Policy::Strict).unwrap();
    assert_eq!(
        parse(&exported.output, Some(Format::EnhancedLrc))
            .unwrap()
            .document,
        doc
    );
    doc.cues[0].segments[1].start_us = None;
    let exported = export(&doc, Format::EnhancedLrc, Policy::Compatible).unwrap();
    assert!(exported
        .losses
        .iter()
        .any(|d| d.code == "SEGMENT_STRUCTURE_CHANGED"));
    assert_eq!(
        export(&doc, Format::EnhancedLrc, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
}

fn rich_document() -> Document {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/conformance.json")).unwrap();
    serde_json::from_value(
        cases
            .iter()
            .find(|case| case["name"] == "rich_vocals_auxiliary")
            .unwrap()["document"]
            .clone(),
    )
    .unwrap()
}

#[test]
fn rich_relationship_validation() {
    let original = rich_document();
    let mutations: Vec<(&str, Document)> = vec![
        ("UNKNOWN_AGENT", {
            let mut d = original.clone();
            d.cues[0].agent_ids.push("missing".into());
            d
        }),
        ("UNKNOWN_AGENT", {
            let mut d = original.clone();
            d.auxiliary[0].segments[0].agent_ids.push("missing".into());
            d
        }),
        ("UNKNOWN_PARENT", {
            let mut d = original.clone();
            d.cues[1].parent_id = Some("s1".into());
            d
        }),
        ("CYCLIC_RELATIONSHIP", {
            let mut d = original.clone();
            d.cues[0].parent_id = Some("c2".into());
            d
        }),
        ("CYCLIC_RELATIONSHIP", {
            let mut d = original.clone();
            d.cues[0].parent_id = Some("c1".into());
            d
        }),
        ("UNKNOWN_TARGET", {
            let mut d = original.clone();
            d.auxiliary[0].target_ids.push("a1".into());
            d
        }),
        ("INVALID_ID", {
            let mut d = original.clone();
            d.auxiliary[0].segments[0].id = Some("s1".into());
            d
        }),
        ("INVALID_ID", {
            let mut d = original.clone();
            d.agents[0].id = "c1".into();
            d
        }),
        ("INVALID_LANGUAGE", {
            let mut d = original.clone();
            d.language = Some(String::new());
            d
        }),
        ("INVALID_ROLE", {
            let mut d = original.clone();
            d.cues[0].roles.push(String::new());
            d
        }),
        ("INVALID_AUXILIARY", {
            let mut d = original.clone();
            d.auxiliary[0].kind.clear();
            d
        }),
        ("INVALID_AGENT", {
            let mut d = original.clone();
            d.agents[0].kind = Some(String::new());
            d
        }),
        ("INVALID_INTERVAL", {
            let mut d = original.clone();
            d.auxiliary[0].segments[0].start_us = Some(2);
            d.auxiliary[0].segments[0].end_us = Some(1);
            d
        }),
    ];
    for (code, document) in mutations {
        assert_eq!(validate(&document).unwrap_err().code, code);
        assert_eq!(inspect(&document).unwrap_err().code, code);
        assert_eq!(
            export(&document, Format::Json, Policy::Compatible)
                .unwrap_err()
                .code,
            code
        );
    }
    let mut document = original.clone();
    document.auxiliary[0].target_ids.clear();
    validate(&document).unwrap();
    document.auxiliary[0].kind = "vendor:pronunciation".into();
    assert!(inspect(&document)
        .unwrap()
        .auxiliary_kinds
        .contains(&"vendor:pronunciation".into()));
    assert_eq!(rich_document(), original);
}

#[test]
fn large_parent_chain_is_bounded_and_nonrecursive() {
    let cues = (0..10_000)
        .map(|index| Cue {
            id: format!("c{index}"),
            parent_id: if index == 9999 {
                None
            } else {
                Some(format!("c{}", index + 1))
            },
            ..Cue::default()
        })
        .collect();
    let mut document = Document {
        cues,
        ..Document::default()
    };
    assert!(inspect(&document).unwrap().relationships);
    document.cues[9999].parent_id = Some("c0".into());
    assert_eq!(validate(&document).unwrap_err().code, "CYCLIC_RELATIONSHIP");
    document.cues[9999].parent_id = None;
    document.cues[0].roles = vec!["lead".into(); 100_001];
    assert_eq!(validate(&document).unwrap_err().code, "DOCUMENT_TOO_LARGE");
}

#[test]
fn empty_cue_and_segment_structure_changes_are_reported() {
    let empty_segment = parse("", Some(Format::Plain)).unwrap().document;
    let empty_segments = parse("", Some(Format::EnhancedLrc)).unwrap().document;
    for (document, format) in [
        (&empty_segment, Format::EnhancedLrc),
        (&empty_segments, Format::Plain),
        (&empty_segments, Format::Lrc),
    ] {
        let result = export(document, format, Policy::Compatible).unwrap();
        assert!(result
            .losses
            .iter()
            .any(|d| d.code == "SEGMENT_STRUCTURE_CHANGED"));
        assert_eq!(
            export(document, format, Policy::Strict).unwrap_err().code,
            "LOSSY_CONVERSION"
        );
    }
    let no_cues = unilyrics::Document::default();
    for format in [Format::Plain, Format::Lrc, Format::EnhancedLrc] {
        let result = export(&no_cues, format, Policy::Compatible).unwrap();
        assert_eq!(result.losses[0].code, "CUE_BOUNDARIES_CHANGED");
        assert_eq!(
            export(&no_cues, format, Policy::Strict).unwrap_err().code,
            "LOSSY_CONVERSION"
        );
    }
}

#[test]
fn metadata_only_lrc_remains_lossless() {
    let document = parse("[ar:Artist]", Some(Format::Lrc)).unwrap().document;
    for format in [Format::Lrc, Format::EnhancedLrc] {
        let result = export(&document, format, Policy::Strict).unwrap();
        assert!(result.losses.is_empty());
        assert_eq!(
            parse(&result.output, Some(format)).unwrap().document,
            document
        );
    }
}
