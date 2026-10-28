use unilyrics::*;

#[test]
fn overlapping_multiline_and_nonsequential_indices_round_trip() {
    let source = "0042\r\n00:00:01,250 --> 00:00:03,000\r\nසිංහල 👩🏽‍💻\r\n<b>世界</b>\r\n\r\n0042\r\n00:00:01,250 --> 00:00:02,000\r\né العربية\r\n";
    assert_eq!(detect(source).unwrap(), Format::Srt);
    let parsed = parse(source, None).unwrap();
    assert_eq!(parsed.document.cues.len(), 2);
    assert_eq!(parsed.document.cues[0].text(), "සිංහල 👩🏽‍💻\n<b>世界</b>");
    assert_eq!(parsed.document.cues[0].start_us, Some(1_250_000));
    assert_eq!(parsed.document.cues[1].start_us, Some(1_250_000));
    assert_eq!(parsed.document.cues[1].attributes["srt:index"], "0042");
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.code == "SRT_MARKUP_UNINTERPRETED"));
    let encoded = export(&parsed.document, Format::Srt, Policy::Strict).unwrap();
    assert!(encoded.losses.is_empty());
    assert_eq!(
        parse(&encoded.output, Some(Format::Srt)).unwrap().document,
        parsed.document
    );
}

#[test]
fn exact_timing_large_hours_empty_cues_and_literal_arrows() {
    let source = "1\n100:59:59,999 --> 101:00:00,000\n\n2\n00:00:00,000 --> 00:00:00,000\nnot --> a timestamp\n ";
    let document = parse(source, Some(Format::Srt)).unwrap().document;
    assert_eq!(document.cues[0].start_us, Some(363_599_999_000));
    assert_eq!(document.cues[0].text(), "");
    assert_eq!(document.cues[1].text(), "not --> a timestamp\n ");
    let output = export(&document, Format::Srt, Policy::Strict).unwrap();
    assert_eq!(
        parse(&output.output, Some(Format::Srt)).unwrap().document,
        document
    );
    assert!(parse("", Some(Format::Srt))
        .unwrap()
        .document
        .cues
        .is_empty());
    assert_eq!(detect("just --> plain text").unwrap(), Format::Plain);
    let carriage_return_source = "1\r00:00:01,000 --> 00:00:02,000\rvoice";
    assert_eq!(detect(carriage_return_source).unwrap(), Format::Srt);
    assert_eq!(
        parse(carriage_return_source, None).unwrap().document.cues[0].text(),
        "voice"
    );
}

#[test]
fn malformed_intervals_and_extensions_are_not_silently_accepted() {
    for source in [
        "1\n00:60:00,000 --> 01:00:00,000\nx",
        "1\n00:00:00.000 --> 00:00:01,000\nx",
        "1\n00:00:00,0000 --> 00:00:01,000\nx",
        "1\n00:00:00,000 --> 00:00:01,000 X1:0\nx",
        "1\n999999999999999999999999:00:00,000 --> 00:00:01,000\nx",
        "title\n00:00:00,000 --> 00:00:01,000\nx",
        "1\nmissing timing",
    ] {
        assert_eq!(
            parse(source, Some(Format::Srt)).unwrap_err().code,
            "INVALID_SRT",
            "{source}"
        );
    }
    assert_eq!(
        parse("1\n00:00:02,000 --> 00:00:01,000\nx", Some(Format::Srt))
            .unwrap_err()
            .code,
        "INVALID_INTERVAL"
    );
}

#[test]
fn export_never_guesses_ends_or_silently_rounds() {
    let lrc = parse("[00:01]voice", Some(Format::Lrc)).unwrap().document;
    assert_eq!(
        export(&lrc, Format::Srt, Policy::Compatible)
            .unwrap_err()
            .code,
        "MISSING_TIMING"
    );
    let mut document = parse("1\n00:00:01,000 --> 00:00:02,000\nvoice", Some(Format::Srt))
        .unwrap()
        .document;
    document.cues[0].start_us = Some(1_000_999);
    let original = document.clone();
    let encoded = export(&document, Format::Srt, Policy::Compatible).unwrap();
    assert!(encoded
        .losses
        .iter()
        .any(|d| d.code == "TIME_PRECISION_REDUCED"));
    assert_eq!(
        parse(&encoded.output, Some(Format::Srt))
            .unwrap()
            .document
            .cues[0]
            .start_us,
        Some(1_000_000)
    );
    assert_eq!(
        export(&document, Format::Srt, Policy::Strict)
            .unwrap_err()
            .code,
        "LOSSY_CONVERSION"
    );
    assert_eq!(document, original);
}

#[test]
fn rich_losses_and_unrepresentable_payloads_are_explicit() {
    let mut document = parse("1\n00:00:01,000 --> 00:00:02,000\nvoice", Some(Format::Srt))
        .unwrap()
        .document;
    document.cues[0].segments[0].start_us = Some(1_100_000);
    document.cues[0].segments[0].roles.push("syllable".into());
    document.language = Some("en".into());
    let encoded = export(&document, Format::Srt, Policy::Compatible).unwrap();
    for code in [
        "SEGMENT_TIMING_DROPPED",
        "ROLES_DROPPED",
        "LANGUAGE_DROPPED",
        "SRT_PROJECTION_CHANGED",
    ] {
        assert!(encoded.losses.iter().any(|loss| loss.code == code));
    }
    for text in [
        "first\n\nsecond",
        "\nleading",
        "trailing\n",
        "carriage\rreturn",
    ] {
        document.cues[0].segments[0].text = text.into();
        assert_eq!(
            export(&document, Format::Srt, Policy::Compatible)
                .unwrap_err()
                .code,
            "UNREPRESENTABLE_TEXT"
        );
    }
}
