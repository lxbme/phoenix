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
use cli::{Exit, MessageFormat, Options};
use compiler::compiler;
use diag::{Diagnostic, emit_json, render_all, report};
use lexer::lexer;
use source::Source;
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
/// The single place diagnostics leave the program, so the chosen format
/// applies to every stage without four copies of the same branch.
///
/// `tally` is the "aborting due to N errors" line. It belongs to a compile
/// stage, which reports a list of findings; a run-time failure is one event and
/// the program did run, so there is nothing being aborted.
fn emit(opts: &Options, source: &Source, diags: &[Diagnostic], tally: bool) {
    match opts.message_format {
        MessageFormat::Json => emit_json(source, diags),
        MessageFormat::Human if tally => report(source, diags),
        MessageFormat::Human => render_all(source, diags),
    }
}

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
            emit(&opts, &source, &diags, true);
            return Exit::Compile;
        }
    };
    if opts.dump_tokens {
        cli::dump_tokens(&source, &tokens);
        return Exit::Ok;
    }

    let mut diags = analyzer(&tokens);
    // only worth saying when the program is otherwise sound
    let conflict = if diags.iter().any(|diag| diag.is_error()) {
        None
    } else {
        cli::stdin_conflict(&opts, &tokens)
    };
    if let Some(diag) = conflict {
        diags.push(diag);
        diags.sort_by_key(|diag| diag.span.start);
    }
    let failed = if opts.deny_warnings {
        // notes are part of another diagnostic, never a finding of their own
        diags.iter().any(|diag| !diag.is_note())
    } else {
        diags.iter().any(|diag| diag.is_error())
    };
    if !diags.is_empty() {
        emit(&opts, &source, &diags, true);
    }
    if failed {
        return Exit::Compile;
    }
    if opts.verbose {
        cli::info("Grammar check passed.");
    }

    let program = match compiler(tokens) {
        Ok(program) => program,
        Err(diags) => {
            emit(&opts, &source, &diags, true);
            return Exit::Compile;
        }
    };
    if opts.dump_opcodes {
        cli::dump_opcodes(&source, &program.code);
        return Exit::Ok;
    }
    if opts.check {
        return Exit::Ok;
    }

    match run_opcode(program, opts.trace) {
        Ok(_) => {
            if opts.verbose {
                cli::info("finished.");
            }
            Exit::Ok
        }
        Err(diags) => {
            // the same rendering as a compile error, minus the tally: the
            // program did run, so there is nothing being "aborted"
            emit(&opts, &source, &diags, false);
            Exit::Runtime
        }
    }
}
