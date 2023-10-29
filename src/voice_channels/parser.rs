use eyre::{eyre, Result, WrapErr};

struct Parser<'a> {
    input: &'a str,
    current_idx: usize,
    current_col: usize,
    current_row: usize,
    parts: Vec<TemplatePart<'a>>,
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
        if let Some(b'\n') = self.current_byte() {
            self.current_col = 1;
            self.current_row += 1;
        } else {
            self.current_col += 1;
        }
        self.current_idx += 1;
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

    fn parse(mut self) -> Result<Template<'a>> {
        while self.current_idx < self.input.len() {
            while self.starts_with("{{") {
                self.advance();
                self.advance();
            }
            if self.current_byte() != Some(b'{') {
                self.advance()
            } else {
                let res = self.parse_braces();
                self.parts.push(res.wrap_err_with(|| {
                    eyre!(
                        "Failed to parse braces at {}:{}",
                        self.current_col,
                        self.current_row
                    )
                })?);
            }
        }
        Ok(Template { parts: self.parts })
    }

    fn parse_braces(&mut self) -> Result<TemplatePart<'static>> {
        assert_eq!(self.current_byte(), Some(b'{'));
        self.advance();
        let content = self.parse_template_content().wrap_err_with(|| {
            eyre!(
                "Failed to parse content of template at {}:{}",
                self.current_col,
                self.current_row
            )
        })?;

        while self.starts_with("}}") {
            self.advance();
            self.advance();
        }

        if self.current_byte() != Some(b'}') {
            return Err(eyre!(
                "Expected '}}' at {}:{}, but found '{}'",
                self.current_col,
                self.current_row,
                self.current_char().unwrap_or('\0')
            ));
        }
        Ok(content)
    }

    fn parse_template_content(&mut self) -> Result<TemplatePart<'static>> {
        Ok(match self.current_byte().ok_or_else(|| eyre!("Unexpected end of input at {}:{}. Reached end of input while trying to parse template content.", self.current_col, self.current_row))? {
            b'#' => TemplatePart::ChannelNumber,
            b'%' => TemplatePart::ChildrenInTotal,
            b'?' => TemplatePart::ConnectedUsersNumber,
            b'c' => TemplatePart::ConnectedUserCapacity,
            _ => return Err(eyre!(
                "Invalid template content at {}:{}. Expected one of '#', '%', but found '{}'",
                self.current_col,
                self.current_row,
                self.current_char().unwrap_or('\0')
            )),
        })
    }
}

pub enum TemplatePart<'a> {
    ChannelNumber,
    ChildrenInTotal,
    ConnectedUsersNumber,
    ConnectedUserCapacity,
    String(&'a str),
}

#[non_exhaustive]
pub struct Template<'a> {
    pub parts: Vec<TemplatePart<'a>>,
}

pub fn parse_template(template: &str) -> Result<Template> {
    Parser::new(template).parse()
}
