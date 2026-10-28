use unilyrics::*;

fn wrap(body: &str) -> String {
    format!(r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div>{body}</div></body></tt>"#)
}

#[test]
fn relative_sequential_timing_and_exact_precision() {
    let input = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body begin="1s"><div begin="1s" timeContainer="seq"><p dur="2s"><span begin="250ms" dur="500ms">one</span></p><p dur="1s">two</p></div></body></tt>"#;
    let document = parse(input, Some(Format::Ttml)).unwrap().document;
    assert_eq!(document.cues.len(), 2);
    assert_eq!(document.cues[0].start_us, Some(2_000_000));
    assert_eq!(document.cues[0].end_us, Some(4_000_000));
    assert_eq!(document.cues[0].segments[0].start_us, Some(2_250_000));
    assert_eq!(document.cues[0].segments[0].end_us, Some(2_750_000));
    assert_eq!(document.cues[1].start_us, Some(4_000_000));
}

#[test]
fn music_absolute_background_and_auxiliary() {
    let input = wrap(
        r#"<p xmlns:m="http://www.w3.org/ns/ttml#metadata" xml:id="lead" begin="10s" end="12s"><span begin="10s" end="11s">声</span><span m:role="x-bg" begin="10.5s" end="11.5s">echo</span><span m:role="x-translation" xml:lang="en" begin="10s" end="12s">voice</span></p>"#,
    );
    let document = parse(&input, Some(Format::TtmlLyrics)).unwrap().document;
    assert_eq!(document.cues.len(), 2);
    assert_eq!(document.cues[0].text(), "声");
    assert_eq!(document.cues[0].segments[0].start_us, Some(10_000_000));
    assert_eq!(document.cues[1].parent_id.as_deref(), Some("lead"));
    assert_eq!(document.cues[1].start_us, Some(10_500_000));
    let auxiliary = &document.auxiliary[0];
    assert_eq!(auxiliary.kind, "translation");
    assert_eq!(auxiliary.language.as_deref(), Some("en"));
    assert_eq!(auxiliary.target_ids, ["lead"]);
    assert_eq!(auxiliary.segments[0].start_us, Some(10_000_000));
    assert_eq!(detect(&input).unwrap_err().code, "AMBIGUOUS_TTML_DIALECT");
    let exported = export(&document, Format::TtmlLyrics, Policy::Strict).unwrap();
    assert!(exported.losses.is_empty());
    assert_eq!(
        parse(&exported.output, Some(Format::TtmlLyrics))
            .unwrap()
            .document,
        document
    );
}

#[test]
fn independent_auxiliary_tracks_and_background_targets_round_trip() {
    let input = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:i="http://music.apple.com/lyric-ttml-internal" xmlns:m="http://www.w3.org/ns/ttml#metadata" xml:space="preserve"><head><metadata><i:iTunesMetadata><i:translations><i:translation xml:lang="si"><i:text xml:id="tr" for="bg"><span begin="1s" end="2s">හඬ</span></i:text></i:translation></i:translations><i:transliterations><i:transliteration><i:text xml:id="roman" for="main"><span>koe</span></i:text></i:transliteration></i:transliterations></i:iTunesMetadata></metadata></head><body><div><p xml:id="lead" i:key="main" begin="10s" end="12s"><span>声</span><span xml:id="back" i:key="bg" m:role="x-bg" begin="10.5s" end="11.5s"><span>echo</span></span></p></div></body></tt>"#;
    let document = parse(input, Some(Format::TtmlLyrics)).unwrap().document;
    assert_eq!(document.auxiliary[0].target_ids, ["back"]);
    assert_eq!(document.auxiliary[0].segments[0].start_us, Some(1_000_000));
    assert_eq!(document.auxiliary[1].segments[0].start_us, None);
    let exported = export(&document, Format::TtmlLyrics, Policy::Strict).unwrap();
    assert_eq!(
        parse(&exported.output, Some(Format::TtmlLyrics))
            .unwrap()
            .document,
        document
    );
    let standard = export(&document, Format::Ttml, Policy::Compatible).unwrap();
    assert!(standard
        .losses
        .iter()
        .any(|loss| loss.code == "AUXILIARY_DROPPED"));
    assert!(standard
        .losses
        .iter()
        .any(|loss| loss.code == "RELATIONSHIPS_DROPPED"));
    assert_eq!(
        export(&document, Format::Ttml, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
}

#[test]
fn whitespace_unicode_comments_and_cdata() {
    let input = wrap(
        r#"<p>  hello   <span>世界</span>  </p><p xml:space="preserve"><span xml:id="word">é<!-- split -->🙂</span><br/> العربية <![CDATA[<!DOCTYPE literal>]]></p>"#,
    );
    let document = parse(&input, Some(Format::Ttml)).unwrap().document;
    assert_eq!(document.cues[0].text(), "hello 世界");
    assert_eq!(document.cues[1].text(), "é🙂\n العربية <!DOCTYPE literal>");
    assert_eq!(document.cues[1].segments[0].id.as_deref(), Some("word"));
    assert_eq!(document.cues[1].segments[0].text, "é🙂");
}

#[test]
fn invalid_and_unsupported_semantics_are_errors() {
    for (body, code) in [
        (r#"<p begin="0.0000001s">x</p>"#, "UNSUPPORTED_PRECISION"),
        (r#"<p begin="1f">x</p>"#, "UNSUPPORTED_TTML"),
        (r#"<p begin="00:60:00">x</p>"#, "INVALID_TTML_TIME"),
        (r#"<p repeatCount="2">x</p>"#, "UNSUPPORTED_TTML"),
        (r#"<p><unknown>x</unknown></p>"#, "UNSUPPORTED_TTML"),
        (
            r#"<p xml:id="same">x</p><p xml:id="same">y</p>"#,
            "INVALID_ID",
        ),
    ] {
        assert_eq!(
            parse(&wrap(body), Some(Format::Ttml)).unwrap_err().code,
            code,
            "{body}"
        );
    }
    let dtd = format!(
        "<!DOCTYPE tt SYSTEM 'file:///etc/passwd'>{}",
        wrap("<p>x</p>")
    );
    assert_eq!(
        parse(&dtd, Some(Format::Ttml)).unwrap_err().code,
        "UNSAFE_XML"
    );
    let deep = wrap(&format!(
        "<p>{}x{}</p>",
        "<span>".repeat(65),
        "</span>".repeat(65)
    ));
    assert_eq!(
        parse(&deep, Some(Format::Ttml)).unwrap_err().code,
        "XML_TOO_DEEP"
    );
}

#[test]
fn strict_export_preserves_supported_model() {
    let document = parse(&wrap(r#"<p xml:id="line" begin="1s" end="3s"><span begin="0s" end="1s">a &amp; b</span></p>"#), Some(Format::Ttml)).unwrap().document;
    let original = document.clone();
    let exported = export(&document, Format::Ttml, Policy::Strict).unwrap();
    assert!(exported.losses.is_empty());
    assert_eq!(
        parse(&exported.output, Some(Format::Ttml))
            .unwrap()
            .document,
        original
    );
    assert_eq!(document, original);
}

#[test]
fn music_metadata_and_unsupported_auxiliary_losses() {
    let mut document = parse(
        &wrap(r#"<p xml:id="line" begin="0s" end="1s">voice</p>"#),
        Some(Format::TtmlLyrics),
    )
    .unwrap()
    .document;
    document.metadata = vec![
        Metadata {
            key: "artist".into(),
            value: "A & B".into(),
        },
        Metadata {
            key: "artist".into(),
            value: "世界".into(),
        },
    ];
    let encoded = export(&document, Format::TtmlLyrics, Policy::Strict).unwrap();
    assert_eq!(
        parse(&encoded.output, Some(Format::TtmlLyrics))
            .unwrap()
            .document,
        document
    );
    document.auxiliary.push(Auxiliary {
        id: "pronunciation".into(),
        kind: "pronunciation".into(),
        language: None,
        target_ids: vec!["line".into()],
        segments: vec![Segment {
            text: "vɔɪs".into(),
            ..Segment::default()
        }],
        attributes: Attributes::new(),
    });
    let original = document.clone();
    let compatible = export(&document, Format::TtmlLyrics, Policy::Compatible).unwrap();
    assert_eq!(
        compatible
            .losses
            .iter()
            .map(|loss| loss.code.as_str())
            .collect::<Vec<_>>(),
        ["AUXILIARY_DROPPED", "TTML_PROJECTION_CHANGED"]
    );
    assert_eq!(
        export(&document, Format::TtmlLyrics, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    assert_eq!(document, original);
}
