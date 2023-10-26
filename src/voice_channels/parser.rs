use eyre::{eyre, Result, WrapErr};

struct Parser<'a> {
    input: &'a str,
    current_idx: usize,
    current_col: usize,
    current_row: usize,
    parts: Vec<PatternPart<'a>>
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            current_idx: 0,
            current_col: 0,
            current_row: 0,
            parts: vec![]
        }
    }
    fn parse(&mut self) -> Result<Pattern<'a>> {
        let mut start_of_current = 0;
        for (i, b) in pattern.bytes().enumerate() {
            if b == b'{' {
                self.parse_braces()?;
                self.parts.push(self.parse_braces()?);
            } else {
                
            }
        }
        Ok(Pattern {
            parts
        })
    }
}

enum PatternPart<'a> {
    ChannelNumber,
    MemberCountCapacity,
    MemberCountNow,
    String(&'a str)
}

#[non_exhaustive]
pub struct Pattern<'a> {
    parts: Vec<PatternPart<'a>>
}



fn parse_pattern_content(content: &str) -> Result<PatternPart<'static>> {
    Ok(match content {
        "#" => PatternPart::ChannelNumber,
        "c" => PatternPart::MemberCountCapacity,
        "m" => PatternPart::MemberCountNow,
        _ => return Err(eyre!("Invalid pattern content: {content}"))
    })
}

fn parse_braces(braces: &str) -> Result<PatternPart<'static>> {
    let mut parts = braces.split(':');
    let content = parts.next().unwrap();
    let content = parse_pattern_content(content)?;
    let mut pattern = Pattern {
        parts: vec![content]
    };
    for part in parts {
        let content = parse_pattern_content(part)?;
        pattern.parts.push(content);
    }
    Ok(PatternPart::String(braces))
}

pub fn parse_pattern<'a>(pattern: &'a str) -> Result<Pattern<'a>> {
    Parser::new(pattern).parse()
}