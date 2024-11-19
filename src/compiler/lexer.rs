pub mod token;
pub mod token_stack;

use crate::compiler::lexer::token::{StaticToken, Token, STATIC_TOKEN_MAP};
use std::fmt::Display;
use std::path::PathBuf;
use std::string::String;
use crate::compiler::parser::syntax_tree::Literal;

#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

pub trait HasLocation {
    fn location(&self) -> &Location;
}

#[derive(Debug, Clone)]
pub struct SrcToken {
    pub(crate) token: Token,
    pub(crate) location: Location,
}

impl HasLocation for SrcToken {
    fn location(&self) -> &Location {
        &self.location
    }
}

pub fn lex(input_path: &PathBuf) -> Vec<SrcToken> {
    let input: Vec<char> = std::fs::read_to_string(input_path)
        .unwrap()
        .chars()
        .collect();
    let mut offset = 0;
    let mut tokens = Vec::new();
    let mut line = 1;
    let mut column = 1;

    while offset < input.len() {
        if input[offset].is_whitespace() {
            if input[offset] == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            offset += 1;
            continue;
        }
        let (token, new_offset) = if input[offset] == '"' {
            read_string(&input, offset)
        } else if input[offset] == '\'' {
            read_char(&input, offset)
        } else {
            read_token(&input, offset)
        };
        tokens.push(SrcToken {
            token,
            location: Location { line, column },
        });
        column += new_offset - offset;
        offset = new_offset;
    }
    tokens.push(SrcToken {
        token: Token::EOF,
        location: Location { line, column },
    });

    tokens
}

fn read_string(input: &Vec<char>, offset: usize) -> (Token, usize) {
    let mut string = String::new();
    let mut i = offset + 1;

    while i < input.len() {
        if input[i] == '"' {
            return (Token::String(string), i + 1);
        } else if input[i] == '\\' {
            i += 1;
            if i >= input.len() {
                panic!("Unterminated string");
            }
            match input[i] {
                'n' => string.push('\n'),
                'r' => string.push('\r'),
                't' => string.push('\t'),
                '\\' => string.push('\\'),
                '"' => string.push('"'),
                _ => panic!("Invalid escape sequence"),
            }
        } else {
            string.push(input[i]);
        }
        i += 1;
    }

    panic!("Unterminated string");
}

fn read_char(input: &Vec<char>, offset: usize) -> (Token, usize) {
    let c = input[offset + 1];
    if input[offset + 2] != '\'' {
        panic!("Invalid character");
    }
    if c.len_utf8() != 1 {
        panic!("Invalid character");
    }
    let mut buffer = [0; 1];
    c.encode_utf8(&mut buffer);
    (Token::Literal(Literal::Char(buffer[0] as i8)), offset + 3)
}

fn read_token(input: &Vec<char>, offset: usize) -> (Token, usize) {
    for len in (1..=StaticToken::MAX_LENGTH).rev() {
        if offset + len > input.len() {
            continue;
        }
        let substr: String = input[offset..offset + len].iter().collect();
        if let Some(token) = STATIC_TOKEN_MAP.get(&substr) {
            return (Token::Static(token.clone()), offset + len);
        }
    }

    let mut substr = String::new();
    let mut last_valid = None;

    for i in offset..input.len() {
        substr.push(input[i]);
        match Token::token_from_str(&substr) {
            Ok(token) => last_valid = Some((token, i + 1)),
            Err(true) => break,
            Err(false) => (),
        }
    }

    if let Some((token, new_offset)) = last_valid {
        (token, new_offset)
    } else {
        panic!("Invalid token: {}", substr);
    }
}
