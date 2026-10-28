use unilyrics::{parse, Format};

fn check(source: &str, format: Format, valid: bool) {
    let result = parse(source, Some(format));
    if valid {
        result.unwrap_or_else(|error| panic!("{source:?}: {error:?}"));
    } else {
        let code = match format {
            Format::Srt => "INVALID_SRT",
            Format::Webvtt => "INVALID_WEBVTT",
            _ => unreachable!(),
        };
        assert_eq!(result.unwrap_err().code, code, "{source:?}");
    }
}

#[test]
fn srt_requires_colons_in_clock_and_comma_before_milliseconds() {
    for a in [':', ',', '.'] {
        for b in [':', ',', '.'] {
            for c in [':', ',', '.'] {
                let stamp = format!("00{a}00{b}01{c}000");
                let valid = (a, b, c) == (':', ':', ',');
                for source in [
                    format!("1\n{stamp} --> 00:00:04,000\nvoice"),
                    format!("1\n00:00:00,000 --> {stamp}\nvoice"),
                ] {
                    check(&source, Format::Srt, valid);
                }
            }
        }
    }
}

#[test]
fn webvtt_requires_colons_in_clock_and_dot_before_milliseconds() {
    for a in [':', ',', '.'] {
        for b in [':', ',', '.'] {
            for (stamp, valid) in std::iter::once((format!("00{a}01{b}000"), (a, b) == (':', '.')))
                .chain(
                    [':', ',', '.']
                        .into_iter()
                        .map(|c| (format!("00{a}00{b}01{c}000"), (a, b, c) == (':', ':', '.'))),
                )
            {
                for source in [
                    format!("WEBVTT\n\n{stamp} --> 00:04.000\nvoice"),
                    format!("WEBVTT\n\n00:00.000 --> {stamp}\nvoice"),
                    format!("WEBVTT\n\n00:00.000 --> 00:04.000\nfirst <{stamp}>second"),
                ] {
                    check(&source, Format::Webvtt, valid);
                }
            }
        }
    }
}
