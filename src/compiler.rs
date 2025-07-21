use std::collections::HashMap;
use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // basic operators
    ADD,
    SUB,
    MUL,
    DIV,
    EQ,
    NEQ,
    GT,
    LT,

    // var & stack operations
    NEW(String),   // new var
    PUSHC(f64),    // push const
    PUSHV(String), // push var
    STORE,         // store var

    // control flow
    INT,           // interrupt
    JMPNP(usize),    // jump if not positive (include 0.0)
    JMP(usize),      // jump
    JMPPH,         // placeholder for jump actions
    CALL(usize),     // call function
    CALLPH(String),// function call placeholder
    RET,           // return

    PRINT,
    PRINTA,        // print ASCII
}

enum Context {
    DowBlk(usize),
    IfBlk(usize),
    ElseBlk(usize),
    FuncBlk
}


pub fn compiler(tokens: Vec<Token>) -> Result<Vec<Opcode>, String> {
    let mut result: Vec<Opcode> = Vec::new();
    // extract function fragments
    let (mut tokens, func_slices) = function_picker(tokens);
    for mut func_slice in func_slices {
	tokens.append(&mut func_slice);
    } // append functions to tokens

    // content control stack 
    let mut context_stack: Vec<Context> = Vec::new();
    // function index table
    let mut func_idx_table: HashMap<String, usize> = HashMap::new();
    
    let mut idx = 0;
    while idx < tokens.len() {
	match &tokens[idx] {
	    Token::EOF => { result.push(Opcode::INT) },
	    Token::Placeholder => {},
	    Token::Operator(op_char) => {
		result.push(make_operator(*op_char)?);
	    },
	    Token::Identifier(id) => {
		result.push(Opcode::PUSHV(id.clone()));
	    },
	    Token::Digit(data) => {
		result.push(Opcode::PUSHC(*data));
	    },
	    Token::Var => {
		match &tokens[idx+1] {
		    Token::Identifier(id) => {
			result.push(Opcode::NEW(id.clone()));
			idx += 1; // skip var name
		    },
		    _ => { return Err(String::from("Invalid var statement")) }
		}
	    },
	    Token::Print => { result.push(Opcode::PRINT) },
	    Token::Printa => { result.push(Opcode::PRINTA) },
	    Token::Dollar => {
		match &tokens[idx+1] {
		    Token::Identifier(id) => {
			match func_idx_table.get(id) {
			    Some(func_idx) => { result.push(Opcode::CALL(*func_idx)) },
			    None => { result.push(Opcode::CALLPH(id.clone())); }
			}
			idx += 1; // skip func name
		    },
		    _ => { return Err(String::from("Invalid function call."))}
		}
	    },
	    Token::Dow => {
		context_stack.push(Context::DowBlk(result.len()));
		idx += 1 ; // skip LeftBracket
	    },
	    Token::If => {
		context_stack.push(Context::IfBlk(result.len()));
		result.push(Opcode::JMPPH); // placeholder for JUMNP
		idx += 1; // skip LeftBracket
	    },
	    Token::Def => {
		match &tokens[idx+1] {
		    Token::Identifier(id) => {
			context_stack.push(Context::FuncBlk);
			func_idx_table.insert(id.clone(), result.len()); // note function idx
			for idx in 0..result.len() {
			    if result[idx] == Opcode::CALLPH(id.clone()) {
				result[idx] = Opcode::CALL(result.len()); // backfill CALLPH
			    }
			}
			idx += 1; // skip function name 
		    },
		    _ => { return Err(String::from("Invalid def statement")) }
		}
	    }
	    Token::RightBracket => {
		match context_stack.pop() {
		    Some(Context::DowBlk(head)) => {
			result.push(Opcode::JMPNP(head))  // jump to head of dow
		    },
		    Some(Context::IfBlk(jmpnp_idx)) => {
			result.push(Opcode::JMPPH); // place holder for JMP before else
			// backfill idx for JMPNP in if
			result[jmpnp_idx] = Opcode::JMPNP(result.len());
			// note idx of place holder
			context_stack.push(Context::ElseBlk(result.len()-1))
		    },
		    Some(Context::ElseBlk(jmp_idx)) => {
			result[jmp_idx] = Opcode::JMP(result.len());
		    },
		    Some(Context::FuncBlk) => {
			result.push(Opcode::RET);
		    }
		    _ => { return Err(String::from("Invalid Brackets")) }
		}
	    }
	    _ => {},
	}
	idx += 1;
    }
    
    Ok(result)
}

fn make_operator(op_char: char) -> Result<Opcode, String> {
    let op = match op_char {
	'+' => { Opcode::ADD },
	'-' => { Opcode::SUB },
	'*' => { Opcode::MUL },
	'/' => { Opcode::DIV },
	'=' => { Opcode::EQ },
	'~' => { Opcode::NEQ },
	'>' => { Opcode::GT },
	'<' => { Opcode::LT},
	'!' => { Opcode::STORE },
	_ => { return Err(format!("Unexpected operator {}", &op_char)); }
    };
    return Ok(op)
}

fn function_picker(mut tokens: Vec<Token>) -> (Vec<Token>, Vec<Vec<Token>>) {
    let mut func_slices: Vec<Vec<Token>> = Vec::new();
    // status
    let mut def_mode = false;
    let mut bracket_counter: i32 = 0;
    let mut single_func: Vec<Token> = Vec::new();
    for idx in 0..tokens.len() {
	// status transfer
	match tokens[idx] {
	    Token::Def => { def_mode = true },
	    Token::LeftBracket => {
		if def_mode { bracket_counter += 1; }
	    },
	    Token::RightBracket => {
		if def_mode { bracket_counter -= 1; }
		if def_mode && bracket_counter == 0 { def_mode = false; }
	    },
	    _ => {}
	}
	// logic
	if def_mode {
	    single_func.push(tokens[idx].clone());
	    tokens[idx] = Token::Placeholder;
	} else {
	    if !single_func.is_empty() {
		single_func.push(Token::RightBracket);
		tokens[idx] = Token::Placeholder;
		func_slices.push(single_func.clone());
		single_func.clear();
	    }
	}
    }
    tokens.retain(|token| !matches!(token, Token::Placeholder));
    (tokens, func_slices)
}
