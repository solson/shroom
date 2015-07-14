use std::io::{self, stdin, stdout, Write};

// TODO(tsion): Use the readline library.
fn prompt(line: &mut String) -> io::Result<usize> {
    let current_dir = try!(std::env::current_dir());
    print!("{}> ", current_dir.display());
    try!(stdout().flush());
    stdin().read_line(line)
}

#[derive(Clone, Debug)]
enum Ast {
    Empty,
    Call { command: String, args: Vec<String> }
}

fn parse(line: &str) -> Ast {
    let mut words = line.split_whitespace();

    match words.next() {
        Some(command) => Ast::Call {
            command: String::from(command),
            args: words.map(String::from).collect()
        },
        None => Ast::Empty
    }
}

fn execute(ast: &Ast) -> io::Result<()> {
    match *ast {
        Ast::Empty => Ok(()),
        Ast::Call { ref command, ref args } => {
            match &**command {
                "cd" => {
                    if args.len() > 1 {
                        return Err(io::Error::new(io::ErrorKind::Other, "cd: too many arguments"));
                    }

                    if let Some(path) = args.get(0) {
                        std::env::set_current_dir(path)
                    } else if let Some(home) = std::env::home_dir() {
                        std::env::set_current_dir(home)
                    } else {
                        Err(io::Error::new(io::ErrorKind::Other, "cd: couldn't find home dir"))
                    }
                },

                "exit" => {
                    if args.len() > 1 {
                        return Err(io::Error::new(io::ErrorKind::Other,
                                                  "exit: too many arguments"));
                    }

                    if let Some(exit_code_str) = args.get(0) {
                        if let Ok(exit_code) = exit_code_str.parse() {
                            std::process::exit(exit_code);
                        } else {
                            Err(io::Error::new(io::ErrorKind::Other,
                                               "exit: couldn't parse exit code as integer"))
                        }
                    } else {
                        std::process::exit(0);
                    }
                }

                _ => {
                    std::process::Command::new(command).args(args).status().map(|_| ())
                },
            }
        },
    }
}

fn main() {
    let mut line = String::new();
    loop {
        prompt(&mut line).unwrap();
        let ast = parse(&line);
        if let Err(e) = execute(&ast) {
            println!("shroom: error: {}", e);
        }
        line.clear();
    }
}
