//! Repeatable bounded mutation checks; not a substitute for coverage-guided fuzzing.
use unilyrics::{
    export, export_preserved, normalize, parse, project, restore_preserved, validate, Document,
    Format, Policy, Profile, ProjectionOptions,
};

const FORMATS: [Format; 8] = [
    Format::Plain,
    Format::Lrc,
    Format::EnhancedLrc,
    Format::Json,
    Format::Ttml,
    Format::TtmlLyrics,
    Format::Srt,
    Format::Webvtt,
];

fn next(state: &mut u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as usize
}

fn seeds() -> Vec<String> {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/conformance.json")).unwrap();
    cases
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            case["input"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| serde_json::to_string(&case["document"]).unwrap())
        })
        .collect()
}

fn retains(actual: &serde_json::Value, expected: &serde_json::Value, label: &str) {
    match (actual, expected) {
        (serde_json::Value::Object(a), serde_json::Value::Object(e)) => {
            assert_eq!(a.len(), e.len(), "{label}");
            for (key, value) in e {
                if key == "attributes" {
                    // Exported syntax can add provenance tags; original attributes must survive.
                    for (name, expected) in value.as_object().unwrap() {
                        assert_eq!(a[key].get(name), Some(expected), "{label}/{name}");
                    }
                } else {
                    retains(&a[key], value, label);
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{label}");
            for (a, e) in a.iter().zip(e) {
                retains(a, e, label);
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}

fn model_invariants(document: &Document, label: &str) {
    validate(document).unwrap_or_else(|e| panic!("invalid parsed model {label}: {e}"));
    let original = document.clone();
    let json = export(document, Format::Json, Policy::Strict).unwrap();
    assert_eq!(
        parse(&json.output, Some(Format::Json)).unwrap().document,
        *document,
        "{label}"
    );
    for profile in [
        Profile::Full,
        Profile::Line,
        Profile::Word,
        Profile::Karaoke,
        Profile::Plain,
        Profile::Translated,
    ] {
        let options = ProjectionOptions {
            profile,
            features: None,
        };
        let once = project(document, &options).unwrap();
        assert_eq!(
            project(&once.document, &options).unwrap().document,
            once.document,
            "{label}/{profile:?}"
        );
    }
    if let Ok(shifted) = normalize(document, 5) {
        assert_eq!(
            normalize(&shifted.document, -5).unwrap().document,
            *document,
            "{label}"
        );
    }
    for target in FORMATS {
        if let Ok(result) = export(document, target, Policy::Strict) {
            let reparsed = parse(&result.output, Some(target))
                .unwrap_or_else(|e| panic!("strict output is invalid {label}/{target:?}: {e}"));
            retains(
                &serde_json::to_value(&reparsed.document).unwrap(),
                &serde_json::to_value(document).unwrap(),
                &format!("strict changed source data {label}/{target:?}"),
            );
        }
        if let Ok(bundle) = export_preserved(document, target, &ProjectionOptions::default()) {
            assert_eq!(
                restore_preserved(&bundle.output, target, &bundle.sidecar)
                    .unwrap()
                    .document,
                *document,
                "{label}/{target:?}"
            );
        }
    }
    assert_eq!(*document, original, "input mutated {label}");
}

#[test]
fn bounded_mutations_are_deterministic_and_preserve_model_invariants() {
    let seeds = seeds();
    let alphabet: Vec<char> = "[]<>/:;.,{}\"'\\\0\n\r\t0123456789 abcසිංහල你好👩🏽‍💻مرحبا\u{301}"
        .chars()
        .collect();
    let mut state = 0x756e_696c_7972_6963;
    let mut successful = 0;
    for index in 0..1024 {
        let mut text: Vec<char> = seeds[index % seeds.len()].chars().take(192).collect();
        for _ in 0..1 + next(&mut state) % 8 {
            let pos = next(&mut state) % (text.len() + 1);
            match next(&mut state) % 3 {
                0 => text.insert(pos, alphabet[next(&mut state) % alphabet.len()]),
                1 if pos < text.len() => {
                    text.remove(pos);
                }
                _ => {
                    text.truncate(pos);
                }
            }
        }
        let text: String = text.into_iter().collect();
        for format in FORMATS.map(Some).into_iter().chain([None]) {
            let label =
                format!("seed=0x756e696c79726963 case={index} format={format:?} input={text:?}");
            let first = parse(&text, format);
            assert_eq!(first, parse(&text, format), "nondeterministic {label}");
            if let Ok(result) = first {
                successful += 1;
                model_invariants(&result.document, &label);
            }
        }
    }
    assert!(
        successful >= 1024,
        "plain parsing should exercise every generated input"
    );
}

#[test]
fn intact_structured_corpus_obeys_roundtrip_invariants() {
    for (index, text) in seeds().iter().enumerate() {
        for format in FORMATS {
            if let Ok(result) = parse(text, Some(format)) {
                model_invariants(&result.document, &format!("intact case={index}/{format:?}"));
            }
        }
    }
}
