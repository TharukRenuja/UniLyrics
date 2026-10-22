use unilyrics::{export, parse, Format, Policy};

fn main() -> Result<(), unilyrics::Error> {
    let parsed = parse("[00:01]Hello 世界 🎵", Some(Format::Lrc))?;
    let converted = export(&parsed.document, Format::Json, Policy::Strict)?;
    println!("{}", converted.output);
    Ok(())
}
