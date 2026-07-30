use crate::compiler::Opcode;
use std::collections::HashMap;

/// The operand stack holds values and nothing else. An earlier version tagged
/// each entry with the variable it had been read from so that `STORE` could
/// find its target at run time; the target is a compile-time fact and now
/// travels inside the instruction, which is also what makes call frames
/// possible later -- a name tag would dangle the moment frames exist.
#[derive(Debug)]
pub struct VM {
    opcodes: Vec<Opcode>,
    current_idx: usize,
    func_ret_stack: Vec<usize>,
    var_table: HashMap<String, f64>,
    main_stack: Vec<f64>,
    stopped: bool,
}

impl VM {
    pub fn new(opcodes: Vec<Opcode>) -> Self {
        VM {
            opcodes,
            current_idx: 0,
            func_ret_stack: Vec::new(),
            var_table: HashMap::new(),
            main_stack: Vec::new(),
            stopped: false,
        }
    }
}

impl VM {
    pub fn step(&mut self) -> Result<(), String> {
        //println!("{:?}", self.main_stack);
        if !self.stopped {
            match self.opcodes[self.current_idx].clone() {
                Opcode::PUSHC(data) => {
                    self.push(data);
                    self.current_idx += 1;
                }
                Opcode::PUSHV(val) => {
                    // search var in var_table
                    let data = match self.var_table.get(&val) {
                        Some(data) => *data,
                        None => {
                            return Err(format!("undefined var: {}", val));
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
                            return Err(format!("undefined var: {}", val));
                        }
                    }
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
                            return Err(String::from("Stack Err: empty return stack"));
                        }
                    }
                }

                Opcode::PRINT => {
                    let opnum = self.pop()?;
                    print!("{}", opnum);
                    self.current_idx += 1;
                }
                Opcode::PRINTA => {
                    let opnum = self.pop()?;
                    match safe_float_to_char(opnum) {
                        Some(character) => print!("{}", character),
                        None => print!("<{}>", opnum.floor()),
                    }
                    self.current_idx += 1;
                }

                _ => {
                    return Err(format!(
                        "Invalid opcode: {:?}",
                        self.opcodes[self.current_idx]
                    ));
                }
            }
        }
        Ok(())
    }

    fn pop(&mut self) -> Result<f64, String> {
        match self.main_stack.pop() {
            Some(dig) => Ok(dig),
            None => Err(String::from("Stack err: empty stack")),
        }
    }

    fn push(&mut self, data: f64) {
        self.main_stack.push(data);
    }
}

pub fn run_opcode(opcodes: Vec<Opcode>) -> Result<(), String> {
    let mut vm = VM::new(opcodes);
    while !vm.stopped {
        vm.step()?;
    }
    Ok(())
}

fn safe_float_to_char(f: f64) -> Option<char> {
    let floored = f.floor();
    if floored >= 0.0 && floored <= 126.0 {
        Some(floored as u8 as char)
    } else {
        None
    }
}
