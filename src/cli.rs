//! Everything that faces the shell: argument parsing, exit codes, reading the
//! input, and the terminal messages that are not source diagnostics.
//!
//! Source diagnostics live in `diag`, so `main` is left holding nothing but
//! the pipeline.

use crate::compiler::Instr;
use crate::diag::Diagnostic;
use crate::lexer::{Token, TokenKind};
use crate::source::Source;
use colored::Colorize;
use std::io::{self, Read};
use std::process::ExitCode;

const USAGE: &str = "\
phoenix - a sequential stack language

USAGE:
    phoenix [OPTIONS] <FILE>
    phoenix [OPTIONS] -            read the program from stdin

OPTIONS:
    -c, --check                    analyse only, do not run
        --dump-tokens              print the token stream and exit
        --dump-opcodes             print the compiled program and exit
        --trace                    print each instruction and the stack
    -W, --deny-warnings            treat warnings as errors
    -v, --verbose                  print progress information
    -h, --help                     print this message
    -V, --version                  print the version

EXIT CODES:
    0   success
    1   the program did not compile
    2   the program failed at run time
    64  bad command line";

/// 64 is `EX_USAGE` from sysexits.h; the rest keep "the program is broken" and
/// "the program broke while running" apart, which scripts care about.
#[derive(Clone, Copy)]
pub enum Exit {
    Ok = 0,
    Compile = 1,
    Runtime = 2,
    Usage = 64,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit as u8)
    }
}

#[derive(Default)]
pub struct Options {
    /// `None` means stdin
    pub path: Option<String>,
    pub check: bool,
    pub dump_tokens: bool,
    pub dump_opcodes: bool,
    pub trace: bool,
    pub deny_warnings: bool,
    pub verbose: bool,
}

/// `Some(options)` to carry on, `None` when `--help` / `--version` already
/// said everything there was to say.
pub fn parse(args: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut opts = Options::default();
    let mut saw_input = false;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", USAGE);
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-c" | "--check" => opts.check = true,
            "--dump-tokens" => opts.dump_tokens = true,
            "--dump-opcodes" => opts.dump_opcodes = true,
            "--trace" => opts.trace = true,
            "-W" | "--deny-warnings" => opts.deny_warnings = true,
            "-v" | "--verbose" => opts.verbose = true,
            // a lone `-` is stdin, anything else starting with `-` is a flag
            "-" => {
                if saw_input {
                    return Err("more than one input given".to_string());
                }
                saw_input = true;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{}`", other));
            }
            other => {
                if saw_input {
                    return Err("more than one input given".to_string());
                }
                saw_input = true;
                opts.path = Some(other.to_string());
            }
        }
    }

    if !saw_input {
        return Err("no input file".to_string());
    }
    Ok(Some(opts))
}

pub fn read_source(opts: &Options) -> Result<Source, String> {
    match &opts.path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Ok(Source::new(path.clone(), text)),
            Err(err) => Err(format!("cannot read `{}`: {}", path, err)),
        },
        None => {
            let mut text = String::new();
            match io::stdin().read_to_string(&mut text) {
                Ok(_) => Ok(Source::new("<stdin>".to_string(), text)),
                Err(err) => Err(format!("cannot read stdin: {}", err)),
            }
        }
    }
}

/// `phoenix -` reads the whole of stdin as the program, so a program that also
/// wants to read input finds nothing there. Easy to hit, confusing to debug.
pub fn stdin_conflict(opts: &Options, tokens: &[Token]) -> Option<Diagnostic> {
    if opts.path.is_some() {
        return None;
    }
    let token = tokens.iter().find(|token| {
        matches!(
            token.kind,
            TokenKind::Read | TokenKind::Reada | TokenKind::IsEof | TokenKind::IsEofa
        )
    })?;
    Some(
        Diagnostic::warning(token.span, "input will be empty")
            .with_note("the program itself came from stdin; pass it as a file to leave stdin free"),
    )
}

pub fn error(msg: &str) {
    eprintln!("{}: {}", "error".red().bold(), msg);
}

pub fn usage_error(msg: &str) -> Exit {
    error(msg);
    eprintln!("\n{}", USAGE);
    Exit::Usage
}

/// Progress chatter, only under `-v`. On stderr so that a redirected stdout
/// holds the program's output and nothing else.
pub fn info(msg: &str) {
    eprintln!("{}", format!("[Info] {}", msg).green());
}

/// The requested output in `--dump-*` mode, so it goes to stdout and can be
/// piped. The program is not run in those modes, so nothing else competes.
pub fn dump_tokens(source: &Source, tokens: &[Token]) {
    for (idx, token) in tokens.iter().enumerate() {
        let loc = source.locate(token.span.start);
        println!("{:>4}  {}:{:<8}{:?}", idx, loc.line, loc.col, token.kind);
    }
}

pub fn dump_opcodes(source: &Source, code: &[Instr]) {
    for (idx, instr) in code.iter().enumerate() {
        let loc = source.locate(instr.span.start);
        println!("{:>4}  {}:{:<8}{:?}", idx, loc.line, loc.col, instr.op);
    }
}
