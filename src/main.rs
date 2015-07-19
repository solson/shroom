use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::iter::Peekable;
use std::process::{Command, ExitStatus};
use std::str::Chars;

extern crate itertools;
use itertools::Itertools;

// TODO(tsion): Use the readline library.
fn prompt(line: &mut String) -> io::Result<usize> {
    let current_dir = try!(std::env::current_dir());
    print!("{}> ", current_dir.display());
    try!(io::stdout().flush());
    io::stdin().read_line(line)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Expr {
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Ast {
    Empty,
    Call { command: String, args: Vec<Vec<Expr>> }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Newline,
    Whitespace,
    Text(String),
}

#[derive(Clone)]
struct Lexer<'a> {
    input: &'a str,
    iter: Peekable<Chars<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParseError {
    UnclosedDelimiter(char),
    UnexpectedChar(char),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::UnclosedDelimiter(c) => write!(f, "unclosed delimiter: {}", c),
            ParseError::UnexpectedChar(c) => write!(f, "unexpected character: {}", c),
        }
    }
}

impl<'a> Lexer<'a> {
    fn new(input: &str) -> Lexer {
        Lexer {
            input: input,
            iter: input.chars().peekable(),
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.iter.peek().cloned()
    }

    fn is_whitespace(c: char) -> bool {
        c == ' ' || c == '\t'
    }

    fn is_unquoted_text(c: char) -> bool {
        match c {
            'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '+' | '/' | '_' | '.' | ',' => true,
            _ => false,
        }
    }

    fn lex_whitespace(&mut self) -> Result<Token, ParseError> {
        while let Some(c) = self.peek_char() {
            if !Lexer::is_whitespace(c) { break; }
            self.iter.next();
        }

        Ok(Token::Whitespace)
    }

    fn lex_unquoted_text(&mut self) -> Result<Token, ParseError> {
        let mut text = String::new();

        while let Some(c) = self.peek_char() {
            if !Lexer::is_unquoted_text(c) { break; }
            text.push(c);
            self.iter.next();
        }

        Ok(Token::Text(text))
    }

    fn lex_quoted_text(&mut self) -> Result<Token, ParseError> {
        let delimiter = self.peek_char().unwrap();
        self.iter.next();

        let mut text = String::new();

        loop {
            if let Some(c) = self.peek_char() {
                self.iter.next();
                match c {
                    '\\' => { unimplemented!() },
                    c if c == delimiter => break,
                    c => { text.push(c) },
                }
            } else {
                return Err(ParseError::UnclosedDelimiter(delimiter))
            }
        }

        Ok(Token::Text(text))
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, ParseError>;

    fn next(&mut self) -> Option<Result<Token, ParseError>> {
        self.peek_char().map(|peek_char| {
            match peek_char {
                c if Lexer::is_whitespace(c)    => self.lex_whitespace(),
                c if Lexer::is_unquoted_text(c) => self.lex_unquoted_text(),
                '\r' | '\n'                     => { self.iter.next(); Ok(Token::Newline) },
                '"' | '\''                      => self.lex_quoted_text(),
                c                               => Err(ParseError::UnexpectedChar(c))
            }
        })
    }
}

#[derive(Clone)]
struct Parser<'a> {
    lexer: Lexer<'a>,
}

impl<'a> Parser<'a> {
    fn new(input: &str) -> Parser {
        Parser { lexer: Lexer::new(input) }
    }

    fn parse(&mut self) -> Result<Ast, ParseError> {
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

    fn parse_call(&mut self, command: String) -> Result<Ast, ParseError> {
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

struct Builtin {
    name: &'static str,
    min_args: usize,
    max_args: usize,
    func: fn(&[String]) -> i32,
}

fn result_to_exit_code(cmd: &'static str, result: io::Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            writeln!(&mut io::stderr(), "shroom: {}: {}", cmd, e).unwrap();
            1
        },
    }
}

fn builtin_cd(args: &[String]) -> i32 {
    if let Some(path) = args.get(0) {
        result_to_exit_code("cd", std::env::set_current_dir(path))
    } else if let Some(home) = std::env::home_dir() {
        result_to_exit_code("cd", std::env::set_current_dir(home))
    } else {
        writeln!(&mut io::stderr(), "shroom: cd: couldn't find home dir").unwrap();
        1
    }
}

fn builtin_exit(args: &[String]) -> i32 {
    if let Some(exit_code_str) = args.get(0) {
        match exit_code_str.parse() {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(e) => {
                writeln!(&mut io::stderr(), "shroom: exit: can't parse exit code: {}", e).unwrap();
                1
            },
        }
    } else {
        std::process::exit(0);
    }
}

fn execute(ast: &Ast) -> i32 {
    let mut builtins = HashMap::new();

    builtins.insert("cd", Builtin {
        name: "cd",
        min_args: 0,
        max_args: 1,
        func: builtin_cd,
    });

    builtins.insert("exit", Builtin {
        name: "exit",
        min_args: 0,
        max_args: 1,
        func: builtin_exit,
    });

    match *ast {
        Ast::Empty => 0,

        Ast::Call { ref command, ref args } => {
            // Evaluate argument expressions.
            let evaluated_args: Vec<String> = args.iter().map(|arg| {
                arg.iter().map(|expr| {
                    match *expr {
                        Expr::Text(ref text) => text,
                    }
                }).join("")
            }).collect();

            if let Some(builtin) = builtins.get(&command[..]) {
                if args.len() < builtin.min_args {
                    writeln!(&mut io::stderr(), "shroom: {}: not enough arguments",
                             builtin.name).unwrap();
                    1
                } else if args.len() > builtin.max_args {
                    writeln!(&mut io::stderr(), "shroom: {}: too many arguments",
                             builtin.name).unwrap();
                    1
                } else {
                    (builtin.func)(&evaluated_args)
                }
            } else {
                match Command::new(command).args(&evaluated_args).status() {
                    Ok(exit_status) => {
                        #[cfg(unix)]
                        fn exit_signal(exit_status: &ExitStatus) -> Option<i32> {
                            use std::os::unix::process::ExitStatusExt;
                            exit_status.signal()
                        }

                        #[cfg(not(unix))]
                        fn exit_signal(_exit_status: &ExitStatus) -> Option<i32> {
                            None
                        }

                        if let Some(code) = exit_status.code() {
                            code
                        } else if let Some(signal) = exit_signal(&exit_status) {
                            128 + signal
                        } else {
                            127
                        }
                    },

                    Err(e) => {
                        writeln!(&mut io::stderr(), "shroom: {}: {}", command, e).unwrap();
                        127
                    },
                }
            }
        },
    }
}

fn main() {
    let mut line = String::new();
    loop {
        prompt(&mut line).unwrap();

        match Parser::new(&line).parse() {
            Ok(ast) => {
                let exit_code = execute(&ast);
                if exit_code != 0 {
                    println!("shroom: exit code: {}", exit_code);
                }
            },

            Err(parse_error) => {
                println!("shroom: parse error: {}", parse_error);
            },
        }

        line.clear();
    }
}
