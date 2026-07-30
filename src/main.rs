use std::io::{self, Write};
use std::process::ExitCode;

mod analyzer;
mod cli;
mod compiler;
mod diag;
mod lexer;
mod source;
mod vm;
use analyzer::analyzer;
use cli::Exit;
use compiler::compiler;
use diag::report;
use lexer::lexer;
use vm::run_opcode;

fn main() -> ExitCode {
    let exit = run();
    // `print!` output is buffered; make sure it is out before we hand back.
    let _ = io::stdout().flush();
    exit.into()
}

/// Drives the pipeline. Each stage collects every diagnostic it can find, but
/// a failing stage stops the run: feeding a broken token stream downstream
/// would only produce cascades of imaginary errors.
fn run() -> Exit {
    let opts = match cli::parse(std::env::args().skip(1)) {
        Ok(Some(opts)) => opts,
        Ok(None) => return Exit::Ok,
        Err(msg) => return cli::usage_error(&msg),
    };

    let source = match cli::read_source(&opts) {
        Ok(source) => source,
        Err(msg) => {
            cli::error(&msg);
            return Exit::Compile;
        }
    };

    let tokens = match lexer(&source) {
        Ok(tokens) => tokens,
        Err(diags) => {
            report(&source, &diags);
            return Exit::Compile;
        }
    };
    if opts.dump_tokens {
        cli::dump_tokens(&source, &tokens);
        return Exit::Ok;
    }

    let diags = analyzer(&tokens);
    let failed = if opts.deny_warnings {
        !diags.is_empty()
    } else {
        diags.iter().any(|diag| diag.is_error())
    };
    if !diags.is_empty() {
        report(&source, &diags);
    }
    if failed {
        return Exit::Compile;
    }
    if opts.verbose {
        cli::info("Grammar check passed.");
    }

    let opcodes = match compiler(tokens) {
        Ok(opcodes) => opcodes,
        Err(diags) => {
            report(&source, &diags);
            return Exit::Compile;
        }
    };
    if opts.dump_opcodes {
        cli::dump_opcodes(&opcodes);
        return Exit::Ok;
    }
    if opts.check {
        return Exit::Ok;
    }

    match run_opcode(opcodes, opts.trace) {
        Ok(_) => {
            if opts.verbose {
                cli::info("finished.");
            }
            Exit::Ok
        }
        Err(err) => {
            cli::error(&err);
            Exit::Runtime
        }
    }
}
