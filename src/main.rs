use std::collections::HashMap;
use std::io::{self, Write};
use std::iter::Peekable;
use std::process::{Command, ExitStatus};
use std::str::CharIndices;

// TODO(tsion): Use the readline library.
fn prompt(line: &mut String) -> io::Result<usize> {
    let current_dir = try!(std::env::current_dir());
    print!("{}> ", current_dir.display());
    try!(io::stdout().flush());
    io::stdin().read_line(line)
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
                '\r' | '\n'                  => { self.iter.next(); Token::Newline },
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
    func: fn(&[String]) -> i32,
}

fn result_to_exit_code(result: io::Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            writeln!(&mut io::stderr(), "cd: {}", e).unwrap();
            1
        },
    }
}

fn builtin_cd(args: &[String]) -> i32 {
    if let Some(path) = args.get(0) {
        result_to_exit_code(std::env::set_current_dir(path))
    } else if let Some(home) = std::env::home_dir() {
        result_to_exit_code(std::env::set_current_dir(home))
    } else {
        writeln!(&mut io::stderr(), "cd: couldn't find home dir").unwrap();
        1
    }
}

fn builtin_exit(args: &[String]) -> i32 {
    if let Some(exit_code_str) = args.get(0) {
        match exit_code_str.parse() {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(e) => {
                writeln!(&mut io::stderr(), "exit: can't parse exit code: {}", e).unwrap();
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
            if let Some(builtin) = builtins.get(&command[..]) {
                if args.len() < builtin.min_args {
                    writeln!(&mut io::stderr(), "{}: not enough arguments", builtin.name).unwrap();
                    1
                } else if args.len() > builtin.max_args {
                    writeln!(&mut io::stderr(), "{}: too many arguments", builtin.name).unwrap();
                    1
                } else {
                    (builtin.func)(args)
                }
            } else {
                match Command::new(command).args(args).status() {
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
                        writeln!(&mut io::stderr(), "shroom: {}", e).unwrap();
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
        let ast = Parser::new(&line).parse();
        let exit_code = execute(&ast);
        if exit_code != 0 {
            println!("shroom: exit code: {}", exit_code);
        }
        line.clear();
    }
}
