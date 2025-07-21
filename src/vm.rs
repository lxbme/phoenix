use crate::compiler::Opcode;
use std::collections::HashMap;
use std::ops;
use std::cmp;

#[derive(Debug)]
struct Item {
    var_name: Option<String>,
    data: f64
}

impl ops::Add for Item {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
	Item { var_name: None, data: self.data + rhs.data}
    }
}

impl ops::Sub for Item {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
	Item { var_name: None, data: self.data - rhs.data}
    }
}

impl ops::Mul for Item {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
	Item { var_name: None, data: self.data * rhs.data}
    }
}

impl ops::Div for Item {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
	Item { var_name: None, data: self.data / rhs.data}
    }
}

impl cmp::PartialEq for Item {
    fn eq(&self, other: &Self) -> bool{
	self.data == other.data
    }
}

impl cmp::PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

#[derive(Debug)]
pub struct VM {
    opcodes: Vec<Opcode>,
    current_idx: usize,
    func_ret_stack: Vec<usize>,
    var_table: HashMap<String, f64>,
    main_stack: Vec<Item>,
    stopped: bool
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
		    // build Item and push
		    self.push(Item {var_name: None, data});
		    self.current_idx += 1;
		},
		Opcode::PUSHV(val) => {
		    // search var in var_table and build Item
		    let data = match self.var_table.get(&val) {
			Some(data) => data,
			None => {return Err(format!("undefined var: {}", val.clone()));}
		    };
		    self.push(Item {var_name: Some(val), data: *data});
		    self.current_idx += 1;
		},
		Opcode::NEW(val) => {
		    // init var in var_table
		    self.var_table.insert(val, 0.0);
		    self.current_idx += 1;
		}
		Opcode::STORE => {
		    let op_var = self.pop()?; // first opnum should be a var
		    let op_some = self.pop()?; // second can be both
		    match op_var.var_name {
			Some(val) => {
			    self.var_table.insert(val, op_some.data);
			},
			None => {return Err(format!{"Top of stack should be var, but find {}", op_var.data}); }
		    };
		    self.current_idx += 1;
		}
		
		Opcode::ADD => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    self.push(opnum_0 + opnum_1);
		    self.current_idx += 1;
		},
		Opcode::SUB => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    self.push(opnum_0 - opnum_1);
		    self.current_idx += 1;
		},
		Opcode::MUL => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    self.push(opnum_0 * opnum_1);
		    self.current_idx += 1;
		},
		Opcode::DIV => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    self.push(opnum_0 / opnum_1);
		    self.current_idx += 1;
		},
		Opcode::EQ => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    if opnum_0 == opnum_1 {
			self.push(Item {var_name: None, data: 1.0});
		    } else {
			self.push(Item {var_name: None, data: 0.0});
		    };
		    self.current_idx += 1;
		},
		Opcode::NEQ => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    if opnum_0 != opnum_1 {
			self.push(Item {var_name: None, data: 1.0});
		    } else {
			self.push(Item {var_name: None, data: 0.0});
		    };
		    self.current_idx +=1
		},
		Opcode::GT => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    if opnum_0 > opnum_1 {
			self.push(Item {var_name: None, data: 1.0});
		    } else {
			self.push(Item {var_name: None, data: 0.0});
		    };
		    self.current_idx += 1;
		},
		Opcode::LT => {
		    let opnum_0 = self.pop()?;
		    let opnum_1 = self.pop()?;
		    if opnum_0 < opnum_1 {
			self.push(Item {var_name: None, data: 1.0});
		    } else {
			self.push(Item {var_name: None, data: 0.0});
		    };
		    self.current_idx += 1;
		},
		
		Opcode::INT => {
		    self.stopped = true;
		},
		Opcode::JMPNP(des) => {
		    let opnum = self.pop()?;
		    if opnum.data <= 0.0 {
			self.current_idx = des;
		    } else {
			self.current_idx += 1;
		    }
		},
		Opcode::JMP(des) => {
		    self.current_idx = des;
		},
		Opcode::CALL(func_idx) => {
		    self.func_ret_stack.push(self.current_idx+1);
		    self.current_idx = func_idx;
		},
		Opcode::RET => {
		    self.current_idx = match self.func_ret_stack.pop() {
			Some(idx) => idx,
			None => {return Err(String::from("Stack Err: empty return stack"));}
		    }
		},
		
		Opcode::PRINT => {
		    let opnum = self.pop()?;
		    print!("{}", opnum.data);
		    self.current_idx += 1;
		},
		Opcode::PRINTA => {
		    let opnum = self.pop()?;
		    match safe_float_to_char(opnum.data) {
			Some(character) => print!("{}", character),
			None => print!("<{}>", opnum.data.floor())
		    }
		    self.current_idx += 1;
		},
		
		_ => {
		    return Err(format!("Invalid opcode: {:?}",
				       self.opcodes[self.current_idx]));
		}
	    }
	}
	Ok(())
    }

    fn pop(&mut self) -> Result<Item, String> {
	match self.main_stack.pop() {
	    Some(dig) => Ok(dig),
	    None => { return Err(String::from("Stack err: empty stack")) }
	}
    }

    fn push(&mut self, item: Item) {
	self.main_stack.push(item);
    }
}

pub fn run_opcode(opcodes: Vec<Opcode>) -> Result<(), String> {
    let mut vm = VM::new(opcodes);
    while !vm.stopped {
	vm.step()?;
    };
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
