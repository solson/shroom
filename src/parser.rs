use std::{error, fmt};
use std::io::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ast {
    Empty,
    Call { command: String, args: Vec<Vec<Expr>> }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    Newline,
    Whitespace,
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lexer<'src> {
    source: &'src str,
    position: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    UnclosedDelimiter,
    UnexpectedChar,
    UnexpectedEnd,
}

pub type ParseResult<T> = Result<T, ParseError>;

impl error::Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::UnclosedDelimiter => "unclosed delimiter",
            ParseError::UnexpectedChar    => "unexpected character",
            ParseError::UnexpectedEnd     => "unexpected end of input",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", error::Error::description(self))
    }
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Lexer<'src> {
        Lexer {
            source: source,
            position: 0,
        }
    }

    fn read_char(&mut self) -> Option<char> {
        let opt_c = self.source[self.position..].chars().next();

        if let Some(c) = opt_c {
            self.position += c.len_utf8();
        }

        opt_c
    }

    /// Step backwards one `char` in the input. Must not be called more times than `read_char` has
    /// been called.
    fn unread_char(&mut self) {
        assert!(self.position != 0);
        let (prev_pos, _) = self.source[..self.position].char_indices().next_back().unwrap();
        self.position = prev_pos;
    }

    fn is_whitespace(c: char) -> bool {
        c == ' ' || c == '\t'
    }

    fn is_unquoted_text(c: char) -> bool {
        match c {
            'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '+' | '/' | '_' | '.' => true,
            _ => false,
        }
    }

    fn skip_while<F>(&mut self, mut predicate: F) where F: FnMut(char) -> bool {
        while let Some(c) = self.read_char() {
            if !predicate(c) {
                self.unread_char();
                break;
            }
        }
    }

    fn lex_whitespace(&mut self) -> ParseResult<Token> {
        self.skip_while(Lexer::is_whitespace);
        Ok(Token::Whitespace)
    }

    fn lex_unquoted_text(&mut self) -> ParseResult<Token> {
        let start = self.position;
        self.skip_while(Lexer::is_unquoted_text);
        let end = self.position;

        let text = String::from(&self.source[start..end]);
        Ok(Token::Text(text))
    }

    fn lex_double_quoted_text(&mut self) -> ParseResult<Token> {
        let mut text = String::new();

        while let Some(c) = self.read_char() {
            match c {
                '"'  => return Ok(Token::Text(text)),
                '\\' => try!(self.lex_double_quote_escape(&mut text)),
                c => text.push(c),
            };
        }

        Err(ParseError::UnclosedDelimiter)
    }

    fn lex_double_quote_escape(&mut self, text: &mut String) -> ParseResult<()> {
        let escaped = try!(self.read_char().ok_or(ParseError::UnexpectedEnd));

        match escaped {
            '\\' | '"' => text.push(escaped),
            c => {
                text.push('\\');
                text.push(c);
            }
        }

        Ok(())
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = ParseResult<Token>;

    fn next(&mut self) -> Option<ParseResult<Token>> {
        self.read_char().map(|c| {
            match c {
                c if Lexer::is_whitespace(c)    => self.lex_whitespace(),
                c if Lexer::is_unquoted_text(c) => {
                    self.unread_char();
                    self.lex_unquoted_text()
                },
                '\r' | '\n'                     => Ok(Token::Newline),
                '"'                             => self.lex_double_quoted_text(),
                _                               => Err(ParseError::UnexpectedChar),
            }
        })
    }
}

#[derive(Clone)]
pub struct Parser<'src> {
    lexer: Lexer<'src>,
}

impl<'src> Parser<'src> {
    pub fn new(input: &'src str) -> Parser<'src> {
        Parser { lexer: Lexer::new(input) }
    }

    pub fn parse(&mut self) -> ParseResult<Ast> {
        if let Some(token_result) = self.lexer.next() {
            let token = try!(token_result);
            match token {
                Token::Whitespace | Token::Newline => self.parse(),
                Token::Text(command) => self.parse_call(command),
            }
        } else {
            Ok(Ast::Empty)
        }
    }

    fn parse_call(&mut self, command: String) -> ParseResult<Ast> {
        let mut args = vec![];
        let mut current_arg = vec![];

        for token_result in &mut self.lexer {
            let token = try!(token_result);
            match token {
                Token::Newline => {
                    if !current_arg.is_empty() {
                        args.push(current_arg);
                    }
                    break;
                },

                Token::Whitespace => {
                    if !current_arg.is_empty() {
                        args.push(current_arg);
                        current_arg = vec![];
                    }
                },

                Token::Text(text) => {
                    current_arg.push(Expr::Text(text));
                },
            }
        }

        Ok(Ast::Call { command: command, args: args })
    }
}

