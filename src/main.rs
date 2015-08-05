extern crate itertools;

use itertools::Itertools;
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::{Command, ExitStatus};

mod parser;
use parser::*;

// TODO(tsion): Use the readline library.
fn prompt(line: &mut String) -> io::Result<usize> {
    let current_dir = try!(std::env::current_dir());
    print!("{}> ", current_dir.display());
    try!(io::stdout().flush());
    io::stdin().read_line(line)
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
