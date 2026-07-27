use crate::lexer::Token;

pub fn analyzer(tokens: &Vec<Token>) -> Result<(), &str> {
    check_bracket_pairs(tokens)?;
    Ok(())
}

fn check_bracket_pairs(tokens: &Vec<Token>) -> Result<(), &str> {
    let mut left_brackets_count: i32 = 0;
    for token in tokens {
        if *token == Token::LeftBracket {
            left_brackets_count += 1
        }
        if *token == Token::RightBracket {
            left_brackets_count -= 1
        }
    }
    if left_brackets_count == 0 {
        return Ok(());
    } else {
        return Err("The Brackets are not in pairs.");
    }
}
