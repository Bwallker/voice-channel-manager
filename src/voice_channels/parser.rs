use eyre::{
    eyre,
    Result,
    WrapErr,
};

struct Parser<'a> {
    input:       &'a str,
    current_idx: usize,
    current_col: usize,
    current_row: usize,
    parts:       Vec<TemplatePart>,
}

impl<'a> Parser<'a> {
    fn starts_with(&self, s: &str) -> bool {
        self.input
            .get(self.current_idx..)
            .map_or(false, |ss| ss.starts_with(s))
    }

    fn current_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.current_idx).copied()
    }

    fn current_char(&self) -> Option<char> {
        self.input
            .get(self.current_idx..)
            .and_then(|s| s.chars().next())
    }

    fn advance(&mut self) {
        let Some(c) = self.current_char() else {
            return;
        };

        if let Some(b'\n') = self.current_byte() {
            self.current_col = 1;
            self.current_row += c.len_utf8();
        } else {
            self.current_col += 1;
        }
        self.current_idx += c.len_utf8();
    }

    fn new(input: &'a str) -> Self {
        Self {
            input,
            current_idx: 0,
            current_col: 1,
            current_row: 1,
            parts: vec![],
        }
    }

    fn parse(mut self) -> Result<Template> {
        while self.current_idx < self.input.len() {
            if self.current_byte() == Some(b'{') {
                let res = self.parse_braces().wrap_err_with(|| {
                    eyre!(
                        "Failed to parse braces at {}:{}",
                        self.current_col,
                        self.current_row
                    )
                })?;
                self.parts.push(res);
            } else {
                let res = self.parse_string().wrap_err_with(|| {
                    eyre!(
                        "Failed to parse string at {}:{}",
                        self.current_col,
                        self.current_row
                    )
                })?;
                self.parts.push(res);
            }
        }
        Ok(Template { parts: self.parts })
    }

    fn parse_string(&mut self) -> Result<TemplatePart> {
        let start_idx = self.current_idx;
        let mut contents = String::new();
        loop {
            while self.starts_with("{{") {
                self.advance();
                self.advance();
                contents.push('{');
                continue;
            }
            while self.starts_with("}}") {
                self.advance();
                self.advance();
                contents.push('}');
                continue;
            }
            let Some(c) = self.current_char() else { break };
            if c == '{' {
                break;
            }
            // println!("{} - {}:{}", self.current_idx, self.current_col, self.current_row);
            contents.push(c);
            self.advance();
        }

        if start_idx == self.current_idx {
            return Err(eyre!(
                "Unexpectedly parsed empty string, this should never happen! At {}:{} with \
                 current char: {}",
                self.current_col,
                self.current_row,
                self.current_char().unwrap_or('\0')
            ));
        }
        Ok(TemplatePart::String(contents))
    }

    fn parse_braces(&mut self) -> Result<TemplatePart> {
        assert_eq!(self.current_byte(), Some(b'{'));
        self.advance();
        let content = self.parse_template_content().wrap_err_with(|| {
            eyre!(
                "Failed to parse content of template at {}:{}",
                self.current_col,
                self.current_row
            )
        })?;

        if self.current_byte() != Some(b'}') {
            return Err(eyre!(
                "Expected '}}' at {}:{}, but found '{}'",
                self.current_col,
                self.current_row,
                self.current_char().unwrap_or('\0')
            ));
        }
        self.advance();
        Ok(content)
    }

    fn parse_template_content(&mut self) -> Result<TemplatePart> {
        let ret = Ok(
            match self.current_byte().ok_or_else(|| {
                eyre!(
                    "Unexpected end of input at {}:{}. Reached end of input while trying to parse \
                     template content.",
                    self.current_col,
                    self.current_row
                )
            })? {
                | b'#' => TemplatePart::ChannelNumber,
                | b'%' => TemplatePart::ChildrenInTotal,
                | _ =>
                    return Err(eyre!(
                        "Invalid template content at {}:{}. Expected one of '#' or '%' but found \
                         '{}'",
                        self.current_col,
                        self.current_row,
                        self.current_char().unwrap_or('\0')
                    )),
            },
        );
        self.advance();
        ret
    }
}

#[non_exhaustive]
#[derive(PartialEq, Eq, Debug)]
pub(crate) enum TemplatePart {
    ChannelNumber,
    ChildrenInTotal,
    String(String),
}

#[non_exhaustive]
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Template {
    pub(crate) parts: Vec<TemplatePart>,
}

pub(crate) fn parse_template(template: &str) -> Result<Template> {
    Parser::new(template).parse()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    #[rstest]
    #[case("Röstkanal", Template {
        parts: vec![TemplatePart::String("Röstkanal".into())]
    })]
    #[case("Röstkanal {#}#", Template {
        parts: vec![TemplatePart::String("Röstkanal ".into()), TemplatePart::ChannelNumber, TemplatePart::String("#".into())]
    })]
    #[case("Röstkanal {{#}}#", Template {
        parts: vec![TemplatePart::String("Röstkanal {#}#".into())]
    })]
    #[case("Röstkanal {{{#}}}#", Template {
        parts: vec![TemplatePart::String("Röstkanal {".into()), TemplatePart::ChannelNumber, TemplatePart::String("}#".into())]
    })]
    fn test_parses(#[case] input: &str, #[case] expected: Template) {
        assert_eq!(expected, parse_template(input).unwrap(),);
    }
}
