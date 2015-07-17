use std::collections::HashMap;
use std::io::{self, stdin, stdout, Write};
use std::iter::Peekable;
use std::str::CharIndices;

// TODO(tsion): Use the readline library.
fn prompt(line: &mut String) -> io::Result<usize> {
    let current_dir = try!(std::env::current_dir());
    print!("{}> ", current_dir.display());
    try!(stdout().flush());
    stdin().read_line(line)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Ast {
    Empty,
    Call { command: String, args: Vec<String> }
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
    iter: Peekable<CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &str) -> Lexer {
        Lexer {
            input: input,
            iter: input.char_indices().peekable(),
        }
    }

    fn pos(&mut self) -> usize {
        self.iter.peek().map(|p| p.0).unwrap_or(self.input.len())
    }

    fn peek_char(&mut self) -> Option<char> {
        self.iter.peek().map(|p| p.1)
    }

    fn is_whitespace(c: char) -> bool {
        c == ' ' || c == '\t'
    }

    fn lex_whitespace(&mut self) -> Token {
        while let Some(c) = self.peek_char() {
            if !Lexer::is_whitespace(c) { break; }
            self.iter.next();
        }

        Token::Whitespace
    }

    fn lex_unquoted_text(&mut self) -> Token {
        let start = self.pos();

        while let Some(c) = self.peek_char() {
            if Lexer::is_whitespace(c) || c == '\r' || c == '\n' { break; }
            self.iter.next();
        }

        let end = self.pos();

        // TODO(tsion): Do this without allocation.
        Token::Text(String::from(&self.input[start..end]))
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.peek_char().map(|peek_char| {
            match peek_char {
                '\r' | '\n'                  => Token::Newline,
                c if Lexer::is_whitespace(c) => self.lex_whitespace(),
                _                            => self.lex_unquoted_text(),
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

    fn parse(&mut self) -> Ast {
        match self.lexer.next() {
            Some(Token::Whitespace) | Some(Token::Newline) => self.parse(),
            Some(Token::Text(command)) => self.parse_call(command),
            None => Ast::Empty,
        }
    }

    fn parse_call(&mut self, command: String) -> Ast {
        let mut args = vec![];

        for token in &mut self.lexer {
            match token {
                Token::Newline    => break,
                Token::Whitespace => {},
                Token::Text(arg)  => { args.push(arg); },
            }
        }

        Ast::Call { command: command, args: args }
    }
}

struct Builtin {
    name: &'static str,
    min_args: usize,
    max_args: usize,
    func: fn(&[String]) -> io::Result<()>,
}

fn builtin_cd(args: &[String]) -> io::Result<()> {
    if let Some(path) = args.get(0) {
        std::env::set_current_dir(path)
    } else if let Some(home) = std::env::home_dir() {
        std::env::set_current_dir(home)
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "cd: couldn't find home dir"))
    }
}

fn builtin_exit(args: &[String]) -> io::Result<()> {
    if let Some(exit_code_str) = args.get(0) {
        if let Ok(exit_code) = exit_code_str.parse() {
            std::process::exit(exit_code);
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "exit: couldn't parse exit code as integer"))
        }
    } else {
        std::process::exit(0);
    }
}

fn execute(ast: &Ast) -> io::Result<()> {
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
        Ast::Empty => Ok(()),

        Ast::Call { ref command, ref args } => {
            if let Some(builtin) = builtins.get(&command[..]) {
                if args.len() < builtin.min_args {
                    return Err(io::Error::new(io::ErrorKind::Other,
                                              format!("{}: not enough arguments", builtin.name)));
                }

                if args.len() > builtin.max_args {
                    return Err(io::Error::new(io::ErrorKind::Other,
                                              format!("{}: too many arguments", builtin.name)));
                }

                (builtin.func)(args)
            } else {
                std::process::Command::new(command).args(args).status().map(|_| ())
            }
        },
    }
}

fn main() {
    let mut line = String::new();
    loop {
        prompt(&mut line).unwrap();
        let ast = Parser::new(&line).parse();
        if let Err(e) = execute(&ast) {
            println!("shroom: error: {}", e);
        }
        line.clear();
    }
}
