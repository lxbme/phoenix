use crate::compiler::{Instr, Opcode, Program};
use crate::diag::{Diagnostic, Span};
use std::collections::HashMap;
use std::io::{self, BufRead, StdinLock, Write};

/// How many call frames a trace shows before it starts counting instead. A
/// deep chain of calls would otherwise push the error itself off the screen.
const MAX_FRAMES: usize = 8;

/// The operand stack holds values and nothing else. An earlier version tagged
/// each entry with the variable it had been read from so that `STORE` could
/// find its target at run time; the target is a compile-time fact and now
/// travels inside the instruction, which is also what makes call frames
/// possible later -- a name tag would dangle the moment frames exist.
pub struct VM {
    opcodes: Vec<Instr>,
    /// entry address -> function name, used only when a trace is being built
    funcs: HashMap<usize, String>,
    current_idx: usize,
    func_ret_stack: Vec<usize>,
    var_table: HashMap<String, f64>,
    arr_table: HashMap<String, Vec<f64>>,
    main_stack: Vec<f64>,
    stopped: bool,
    /// `print` does not emit newlines, so the program's output may end in the
    /// middle of a line. Tracked here so the runner can finish the line.
    at_line_start: bool,
    /// Buffered so that `eof` can look ahead without consuming anything.
    input: StdinLock<'static>,
    /// Stack depth when the current instruction began. An instruction that
    /// takes two values may pop one successfully before failing, so the depth
    /// at the failing `pop` would understate what the program actually had.
    depth_on_entry: usize,
}

impl VM {
    pub fn new(program: Program) -> Self {
        VM {
            opcodes: program.code,
            funcs: program.funcs,
            current_idx: 0,
            func_ret_stack: Vec::new(),
            var_table: HashMap::new(),
            arr_table: HashMap::new(),
            main_stack: Vec::new(),
            stopped: false,
            at_line_start: true,
            input: io::stdin().lock(),
            depth_on_entry: 0,
        }
    }
}

impl VM {
    /// Where the VM is now. Every failure is reported against the instruction
    /// that was executing, which is the source the user has to look at.
    fn here(&self) -> Span {
        self.opcodes[self.current_idx].span
    }

    fn fail(&self, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.here(), msg)
    }

    /// The call trace at the moment of a failure, innermost call first.
    ///
    /// `func_ret_stack` holds the address to resume at, so the `CALL` that
    /// pushed it is the instruction just before -- and that instruction names
    /// the function it entered, which is all a frame needs.
    fn frames(&self) -> Vec<Diagnostic> {
        let depth = self.func_ret_stack.len();
        let mut frames: Vec<Diagnostic> = Vec::new();
        for ret in self.func_ret_stack.iter().rev() {
            // A runaway chain of calls would otherwise bury the error itself
            if frames.len() == MAX_FRAMES {
                let hidden = depth - MAX_FRAMES;
                if let Some(last) = frames.pop() {
                    frames.push(last.with_note(format!(
                        "... and {} more frame{}",
                        hidden,
                        if hidden == 1 { "" } else { "s" }
                    )));
                }
                break;
            }
            let call_site = ret.saturating_sub(1);
            let Some(Opcode::CALL(entry)) = self.opcodes.get(call_site).map(|instr| &instr.op)
            else {
                continue; // not a call after all; nothing honest to say about it
            };
            let name = match self.funcs.get(entry) {
                Some(name) => name.clone(),
                None => continue,
            };
            frames.push(Diagnostic::note(
                self.opcodes[call_site].span,
                format!("`{}` was called from here", name),
            ));
        }
        frames
    }

    pub fn step(&mut self) -> Result<(), Diagnostic> {
        if !self.stopped {
            self.depth_on_entry = self.main_stack.len();
            match self.opcodes[self.current_idx].op.clone() {
                Opcode::PUSHC(data) => {
                    self.push(data);
                    self.current_idx += 1;
                }
                Opcode::PUSHV(val) => {
                    // search var in var_table
                    let data = match self.var_table.get(&val) {
                        Some(data) => *data,
                        None => {
                            return Err(self
                                .fail(format!("undefined variable `{}`", val))
                                .with_note(
                                    "`var` runs where it is written; control never reached it",
                                ));
                        }
                    };
                    self.push(data);
                    self.current_idx += 1;
                }
                Opcode::NEW(val) => {
                    // init var in var_table
                    self.var_table.insert(val, 0.0);
                    self.current_idx += 1;
                }
                Opcode::STORE(val) => {
                    let data = self.pop()?;
                    match self.var_table.get_mut(&val) {
                        Some(slot) => *slot = data,
                        None => {
                            return Err(self
                                .fail(format!("undefined variable `{}`", val))
                                .with_note(
                                    "`var` runs where it is written; control never reached it",
                                ));
                        }
                    }
                    self.current_idx += 1;
                }

                Opcode::NEWARR(name, len) => {
                    // Like `NEW`, this runs where it is written, so an array
                    // declared inside a function is zeroed on every call.
                    self.arr_table.insert(name, vec![0.0; len]);
                    self.current_idx += 1;
                }
                Opcode::ALOAD(name) => {
                    let span = self.here();
                    let raw = self.pop()?;
                    let array = match self.arr_table.get(&name) {
                        Some(array) => array,
                        None => {
                            return Err(self
                                .fail(format!("undefined array `{}`", name))
                                .with_note(
                                    "`arr` runs where it is written; control never reached it",
                                ));
                        }
                    };
                    let element = array[check_index(raw, array.len(), &name, span)?];
                    self.push(element);
                    self.current_idx += 1;
                }
                Opcode::ASTORE(name) => {
                    let span = self.here();
                    // the index was pushed last, so it comes off first
                    let raw = self.pop()?;
                    let data = self.pop()?;
                    let array = match self.arr_table.get_mut(&name) {
                        Some(array) => array,
                        None => {
                            return Err(self
                                .fail(format!("undefined array `{}`", name))
                                .with_note(
                                    "`arr` runs where it is written; control never reached it",
                                ));
                        }
                    };
                    let slot = check_index(raw, array.len(), &name, span)?;
                    array[slot] = data;
                    self.current_idx += 1;
                }

                Opcode::ADD => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(opnum_0 + opnum_1);
                    self.current_idx += 1;
                }
                Opcode::SUB => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(opnum_0 - opnum_1);
                    self.current_idx += 1;
                }
                Opcode::MUL => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(opnum_0 * opnum_1);
                    self.current_idx += 1;
                }
                Opcode::DIV => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(opnum_0 / opnum_1);
                    self.current_idx += 1;
                }
                Opcode::EQ => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(if opnum_0 == opnum_1 { 1.0 } else { 0.0 });
                    self.current_idx += 1;
                }
                Opcode::NEQ => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(if opnum_0 != opnum_1 { 1.0 } else { 0.0 });
                    self.current_idx += 1
                }
                Opcode::GT => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(if opnum_0 > opnum_1 { 1.0 } else { 0.0 });
                    self.current_idx += 1;
                }
                Opcode::LT => {
                    let opnum_0 = self.pop()?;
                    let opnum_1 = self.pop()?;
                    self.push(if opnum_0 < opnum_1 { 1.0 } else { 0.0 });
                    self.current_idx += 1;
                }

                Opcode::INT => {
                    self.stopped = true;
                }
                Opcode::JMPNP(des) => {
                    let opnum = self.pop()?;
                    if opnum <= 0.0 {
                        self.current_idx = des;
                    } else {
                        self.current_idx += 1;
                    }
                }
                Opcode::JMP(des) => {
                    self.current_idx = des;
                }
                Opcode::CALL(func_idx) => {
                    self.func_ret_stack.push(self.current_idx + 1);
                    self.current_idx = func_idx;
                }
                Opcode::RET => {
                    self.current_idx = match self.func_ret_stack.pop() {
                        Some(idx) => idx,
                        None => {
                            return Err(self.fail("returned with no matching call"));
                        }
                    }
                }

                Opcode::PRINT => {
                    let opnum = self.pop()?;
                    print!("{}", opnum);
                    self.at_line_start = false;
                    self.current_idx += 1;
                }
                Opcode::PRINTA => {
                    let opnum = self.pop()?;
                    let character = safe_float_to_char(opnum);
                    match character {
                        Some(character) => print!("{}", character),
                        None => print!("<{}>", opnum.floor()),
                    }
                    self.at_line_start = character == Some('\n');
                    self.current_idx += 1;
                }

                Opcode::READ => {
                    let value = self.read_number()?;
                    self.push(value);
                    self.current_idx += 1;
                }
                Opcode::READA => {
                    // byte oriented, to match `printa`, which is ASCII only
                    let code = match self.read_byte()? {
                        Some(byte) => f64::from(byte),
                        None => -1.0,
                    };
                    self.push(code);
                    self.current_idx += 1;
                }
                Opcode::ISEOF => {
                    let at_eof = self.at_eof_word()?;
                    self.push(if at_eof { 1.0 } else { 0.0 });
                    self.current_idx += 1;
                }
                Opcode::ISEOFA => {
                    let at_eof = self.at_eof_byte()?;
                    self.push(if at_eof { 1.0 } else { 0.0 });
                    self.current_idx += 1;
                }

                // Placeholders are all backfilled by the compiler and every
                // name is resolved by the analyzer, so reaching this is a bug
                // in the toolchain rather than in the program being run.
                op => {
                    return Err(self
                        .fail(format!("internal error: unresolved instruction {:?}", op))
                        .with_note("this is a compiler bug, not a mistake in the program"));
                }
            }
        }
        Ok(())
    }

    /// stdout is line buffered, so a prompt written with `print` would sit in
    /// the buffer while the program waits for input. Flush before every read.
    fn flush_output(&self) {
        let _ = io::stdout().flush();
    }

    fn peek_input(&mut self) -> Result<&[u8], Diagnostic> {
        // copied out before `input` is borrowed, so the span is available on
        // the error path without holding a second borrow of `self`
        let span = self.here();
        self.input
            .fill_buf()
            .map_err(|err| Diagnostic::new(span, format!("cannot read stdin: {}", err)))
    }

    /// Byte level: is there another byte at all? Pairs with `reada`.
    fn at_eof_byte(&mut self) -> Result<bool, Diagnostic> {
        self.flush_output();
        Ok(self.peek_input()?.is_empty())
    }

    /// Word level: is there another number? Pairs with `read`, so the
    /// whitespace between numbers must not count as input. It is discarded
    /// here -- which is why mixing `eof` with `reada` loses whitespace.
    fn at_eof_word(&mut self) -> Result<bool, Diagnostic> {
        self.flush_output();
        self.skip_whitespace()?;
        Ok(self.peek_input()?.is_empty())
    }

    fn skip_whitespace(&mut self) -> Result<(), Diagnostic> {
        loop {
            let (eaten, found) = match self.peek_input()? {
                [] => return Ok(()),
                buf => match buf.iter().position(|byte| !byte.is_ascii_whitespace()) {
                    Some(offset) => (offset, true),
                    None => (buf.len(), false),
                },
            };
            self.input.consume(eaten);
            if found {
                return Ok(());
            }
        }
    }

    fn read_byte(&mut self) -> Result<Option<u8>, Diagnostic> {
        self.flush_output();
        let byte = match self.peek_input()? {
            [] => None,
            buf => Some(buf[0]),
        };
        if byte.is_some() {
            self.input.consume(1);
        }
        Ok(byte)
    }

    /// Skips whitespace, then takes everything up to the next whitespace.
    /// End of input is an error here -- `eof` is how a program avoids it.
    fn read_number(&mut self) -> Result<f64, Diagnostic> {
        self.flush_output();
        self.skip_whitespace()?;
        if self.peek_input()?.is_empty() {
            return Err(self.fail("read: unexpected end of input"));
        }

        let mut token = String::new();
        loop {
            let (piece, eaten, done) = match self.peek_input()? {
                [] => (String::new(), 0, true),
                buf => match buf.iter().position(|byte| byte.is_ascii_whitespace()) {
                    Some(offset) => (
                        String::from_utf8_lossy(&buf[..offset]).into_owned(),
                        offset,
                        true,
                    ),
                    None => (String::from_utf8_lossy(buf).into_owned(), buf.len(), false),
                },
            };
            self.input.consume(eaten);
            token.push_str(&piece);
            if done {
                break;
            }
        }

        let span = self.here();
        token
            .parse::<f64>()
            .map_err(|_| Diagnostic::new(span, format!("read: `{}` is not a number", token)))
    }

    fn pop(&mut self) -> Result<f64, Diagnostic> {
        match self.main_stack.pop() {
            Some(dig) => Ok(dig),
            None => {
                let (what, needs) = self.opcodes[self.current_idx].op.stack_demand();
                let had = self.depth_on_entry;
                Err(self.fail("stack underflow").with_note(format!(
                    "{} needs {} value{}, but the stack {}",
                    what,
                    needs,
                    if needs == 1 { "" } else { "s" },
                    if had == 0 {
                        String::from("was empty")
                    } else {
                        format!("held {}", had)
                    },
                )))
            }
        }
    }

    fn push(&mut self, data: f64) {
        self.main_stack.push(data);
    }
}

pub fn run_opcode(program: Program, trace: bool) -> Result<(), Vec<Diagnostic>> {
    let mut vm = VM::new(program);
    // An empty program has no instruction 0; stepping would index out of bounds.
    let mut outcome = Ok(());
    while !vm.stopped && vm.current_idx < vm.opcodes.len() {
        if trace {
            // stderr, so it never mixes into the program's own output
            eprintln!(
                "{:>4}  {:<24}{:?}",
                vm.current_idx,
                format!("{:?}", vm.opcodes[vm.current_idx].op),
                vm.main_stack
            );
        }
        if let Err(diag) = vm.step() {
            // the error, then the calls that led to it, innermost first
            let mut diags = vec![diag];
            diags.extend(vm.frames());
            outcome = Err(diags);
            break;
        }
    }
    // finish the line the program left open, on both the ok and the error path
    if !vm.at_line_start {
        println!();
    }
    outcome
}

/// Every value is an `f64`, so an index can be fractional, negative or NaN.
/// None of those is rounded into something plausible: silently flooring a bad
/// index turns "I computed the wrong subscript" into "I read my neighbour's
/// data", which is the hardest kind of bug to find in a program like this.
fn check_index(raw: f64, len: usize, name: &str, span: Span) -> Result<usize, Diagnostic> {
    // NaN and the infinities have a NaN `fract`, so they fail this test too
    if raw.fract() != 0.0 {
        return Err(Diagnostic::new(
            span,
            format!("`{}` index {} is not a whole number", name, raw),
        ));
    }
    if raw < 0.0 || raw >= len as f64 {
        return Err(Diagnostic::new(
            span,
            format!("`{}` index {} is out of bounds (length {})", name, raw, len),
        )
        .with_note(format!("valid indices are 0 to {}", len - 1)));
    }
    Ok(raw as usize)
}

fn safe_float_to_char(f: f64) -> Option<char> {
    let floored = f.floor();
    if floored >= 0.0 && floored <= 126.0 {
        Some(floored as u8 as char)
    } else {
        None
    }
}
