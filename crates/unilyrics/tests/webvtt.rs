use unilyrics::*;

#[test]
fn voices_languages_inline_timing_and_styles_round_trip() {
    let input="WEBVTT lyrics\nX-TIMESTAMP-MAP=LOCAL:00:00:00.000,MPEGTS:900000\n\nSTYLE\n::cue { color: white }\n\nREGION\nid:main\nwidth:80%\n\nNOTE before\nretained comment\n\nverse\n00:01.000 --> 00:04.000 region:main align:start\n<v Alice><lang ja><b>声</b><00:02.000><c.word>世界</c></lang></v>\n\nverse\n00:02.000 --> 00:05.000\n<v Bob>සිංහල 👩🏽‍💻\n\nNOTE after\n";
    assert_eq!(detect(input).unwrap(), Format::Webvtt);
    let document = parse(input, None).unwrap().document;
    assert_eq!(document.cues.len(), 2);
    assert_eq!(document.agents.len(), 2);
    assert_eq!(document.agents[0].name.as_deref(), Some("Alice"));
    assert_eq!(document.cues[0].text(), "声世界");
    assert_eq!(document.cues[0].segments.len(), 2);
    assert_eq!(document.cues[0].segments[0].language.as_deref(), Some("ja"));
    assert_eq!(document.cues[0].segments[1].start_us, Some(2_000_000));
    assert_eq!(document.cues[0].segments[1].end_us, None);
    assert_eq!(document.cues[1].text(), "සිංහල 👩🏽‍💻");
    let output = export(&document, Format::Webvtt, Policy::Strict).unwrap();
    assert!(output.losses.is_empty());
    assert_eq!(
        parse(&output.output, Some(Format::Webvtt))
            .unwrap()
            .document,
        document
    );
}

#[test]
fn groups_ruby_entities_empty_spans_and_multiline_text() {
    let input="WEBVTT\n\n00:00.000 --> 00:03.000\n<c.a>x</c><c.a>y</c><b></b><ruby>漢<rt>kan</ruby>\n&lt;literal&gt; &amp; &nbsp;&lrm;&rlm;é\n";
    let document = parse(input, None).unwrap().document;
    assert!(document.cues[0].text().contains("漢kan\n<literal> &"));
    assert!(document.cues[0]
        .segments
        .iter()
        .any(|s| s.roles == ["ruby_text"]));
    assert!(document.cues[0].segments.iter().any(|s| s.text.is_empty()));
    let output = export(&document, Format::Webvtt, Policy::Strict).unwrap();
    assert_eq!(
        parse(&output.output, Some(Format::Webvtt))
            .unwrap()
            .document,
        document
    );
}

#[test]
fn timing_and_markup_failures_are_explicit() {
    for (input, code) in [
        ("WEBVTT\n\n00:01.000 --> 00:01.000\nx", "INVALID_WEBVTT"),
        (
            "WEBVTT\n\n00:01.000 --> 00:04.000\nx<00:01.000>y",
            "INVALID_WEBVTT",
        ),
        (
            "WEBVTT\n\n00:01.000 --> 00:04.000\nx<00:04.000>y",
            "INVALID_WEBVTT",
        ),
        (
            "WEBVTT\n\n00:01.000 --> 00:04.000\nx<00:03.000>y<00:02.000>z",
            "INVALID_WEBVTT",
        ),
        (
            "WEBVTT\n\n00:00.000 --> 00:01.000\n<b>x</i>",
            "INVALID_WEBVTT",
        ),
        (
            "WEBVTT\n\n00:00.000 --> 00:01.000\n<script>x</script>",
            "UNSUPPORTED_WEBVTT",
        ),
        (
            "WEBVTT\n\n00:00.000 --> 00:01.000\n&copy;",
            "UNSUPPORTED_WEBVTT",
        ),
        ("WEBVTT\n00:00.000 --> 00:01.000\nx", "INVALID_WEBVTT"),
    ] {
        assert_eq!(
            parse(input, Some(Format::Webvtt)).unwrap_err().code,
            code,
            "{input}"
        );
    }
    let nested = format!(
        "WEBVTT\n\n00:00.000 --> 00:01.000\n{}x{}",
        "<b>".repeat(65),
        "</b>".repeat(65)
    );
    assert_eq!(parse(&nested, None).unwrap_err().code, "WEBVTT_TOO_DEEP");
}

#[test]
fn plain_canonical_export_and_conversion_losses() {
    let mut document = parse("1\n00:00:01,000 --> 00:00:03,000\nvoice", Some(Format::Srt))
        .unwrap()
        .document;
    let result = export(&document, Format::Webvtt, Policy::Strict).unwrap();
    assert_eq!(
        parse(&result.output, Some(Format::Webvtt))
            .unwrap()
            .document,
        document
    );
    document.metadata.push(Metadata {
        key: "artist".into(),
        value: "unknown".into(),
    });
    let original = document.clone();
    assert_eq!(
        export(&document, Format::Webvtt, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    assert!(export(&document, Format::Webvtt, Policy::Compatible)
        .unwrap()
        .losses
        .iter()
        .any(|loss| loss.code == "WEBVTT_PROJECTION_CHANGED"));
    assert_eq!(document, original);
    let lrc = parse("[00:01]voice", Some(Format::Lrc)).unwrap().document;
    assert_eq!(
        export(&lrc, Format::Webvtt, Policy::Compatible)
            .unwrap_err()
            .code,
        "MISSING_TIMING"
    );
}

#[test]
fn consecutive_markers_and_settings_whitespace_are_retained() {
    let input =
        "WEBVTT\r\n\r\n00:01.000 --> 00:04.000   align:start  \r\nx<00:02.000><00:03.000>\r\n";
    let parsed = parse(input, None).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.code == "NEWLINES_NORMALIZED"));
    assert_eq!(
        parsed.document.cues[0]
            .segments
            .iter()
            .map(|s| s.start_us)
            .collect::<Vec<_>>(),
        [None, Some(2_000_000), Some(3_000_000)]
    );
    let output = export(&parsed.document, Format::Webvtt, Policy::Strict).unwrap();
    assert_eq!(
        parse(&output.output, None).unwrap().document,
        parsed.document
    );
}

#[test]
fn canonical_annotations_generate_voice_and_language_spans() {
    let document = Document {
        agents: vec![Agent {
            id: "V1".into(),
            name: Some("Alice".into()),
            kind: Some("voice".into()),
            attributes: Attributes::new(),
        }],
        cues: vec![Cue {
            id: "L1".into(),
            start_us: Some(1_000_000),
            end_us: Some(4_000_000),
            segments: vec![
                Segment {
                    text: "声".into(),
                    start_us: Some(1_500_000),
                    agent_ids: vec!["V1".into()],
                    language: Some("ja".into()),
                    ..Segment::default()
                },
                Segment {
                    text: "世界".into(),
                    start_us: Some(2_000_000),
                    agent_ids: vec!["V1".into()],
                    language: Some("ja".into()),
                    ..Segment::default()
                },
            ],
            ..Cue::default()
        }],
        ..Document::default()
    };
    let original = document.clone();
    let output = export(&document, Format::Webvtt, Policy::Strict).unwrap();
    assert!(output.losses.is_empty());
    assert_eq!(output.output.matches("<v Alice>").count(), 1);
    assert_eq!(output.output.matches("<lang ja>").count(), 1);
    let mut round_trip = parse(&output.output, Some(Format::Webvtt))
        .unwrap()
        .document;
    for segment in &mut round_trip.cues[0].segments {
        segment.attributes.remove("webvtt:tags");
    }
    assert_eq!(round_trip, document);
    assert_eq!(document, original);
    let mut initial_anchor = document.clone();
    initial_anchor.cues[0].segments[0].start_us = initial_anchor.cues[0].start_us;
    let compatible = export(&initial_anchor, Format::Webvtt, Policy::Compatible).unwrap();
    assert!(compatible
        .losses
        .iter()
        .any(|loss| loss.code == "SEGMENT_START_IMPLICIT"));
    assert_eq!(
        parse(&compatible.output, None).unwrap().document.cues[0].text(),
        "声世界"
    );
    assert_eq!(
        export(&initial_anchor, Format::Webvtt, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
}

#[test]
fn inherited_annotations_and_unsupported_multi_voice_are_explicit() {
    let mut document = parse("1\n00:00:01,000 --> 00:00:03,000\nvoice", Some(Format::Srt))
        .unwrap()
        .document;
    document.language = Some("en".into());
    let output = export(&document, Format::Webvtt, Policy::Compatible).unwrap();
    assert!(output.output.contains("<lang en>"));
    assert_eq!(
        parse(&output.output, None).unwrap().document.cues[0].segments[0]
            .language
            .as_deref(),
        Some("en")
    );
    assert!(output
        .losses
        .iter()
        .any(|loss| loss.code == "WEBVTT_PROJECTION_CHANGED"));
    document.language = None;
    for id in ["V1", "V2"] {
        document.agents.push(Agent {
            id: id.into(),
            name: Some(id.into()),
            kind: Some("voice".into()),
            attributes: Attributes::new(),
        });
    }
    document.cues[0].segments[0].agent_ids = vec!["V1".into(), "V2".into()];
    let output = export(&document, Format::Webvtt, Policy::Compatible).unwrap();
    assert!(output
        .losses
        .iter()
        .any(|loss| loss.code == "VOCAL_AGENTS_DROPPED"));
    assert_eq!(
        export(&document, Format::Webvtt, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
}
