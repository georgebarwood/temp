use tablestg::*;

/// Token.
#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    /// Integer, e.g. 1945
    Int(i64),
    /// Start, Len.
    String(usize, usize),
    /// Start, Len.    
    Ident(usize, usize), // Start, Len
    Dot,
    LBra,
    RBra,
    Comma,
    Eof,
}

/// Reads tokens from byte string.
pub struct TokenReader<'a> {
    pub input: &'a [u8],
    pub pos: usize,
}

impl<'a> TokenReader<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }
    pub fn next_token(&mut self) -> Result<Token, E> {
        let mut c = self.cc();
        while c == b' ' || c == b'\n'
        // Skip spaces
        {
            c = self.getc();
        }
        match c {
            b'0'..=b'9' => self.num(),
            b'a'..=b'z' | b'A'..=b'Z' => self.ident(),
            b'\'' => self.string(),
            b'.' => {
                self.getc();
                Ok(Token::Dot)
            }
            b'(' => {
                self.getc();
                Ok(Token::LBra)
            }
            b')' => {
                self.getc();
                Ok(Token::RBra)
            }
            b',' => {
                self.getc();
                Ok(Token::Comma)
            }
            0 => Ok(Token::Eof),
            _ => { self.getc(); self.err("Unexpected char in input") },
        }
    }
    fn cc(&self) -> u8 {
        if self.pos >= self.input.len() {
            0
        } else {
            self.input[self.pos]
        }
    }
    fn getc(&mut self) -> u8 {
        self.pos += 1;
        if self.pos >= self.input.len() {
            return 0;
        }
        self.input[self.pos]
    }

    fn num(&mut self) -> Result<Token, E> {
        let mut result = (self.cc() - b'0') as i64;
        let mut c = self.getc();
        while let b'0'..=b'9' = c { 
            result = result * 10 + (c - b'0') as i64;
            c = self.getc();
        }
        Ok(Token::Int(result))
    }

    fn ident(&mut self) -> Result<Token, E> {
        let start = self.pos;
        loop {
            let c = self.cc();
            match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => {}
                _ => break,
            }
            self.getc();
        }
        Ok(Token::Ident(start, self.pos))
    }
    fn string(&mut self) -> Result<Token, E> {
        let mut c = self.getc();
        let start = self.pos;
        loop {
            match c {
                b'\'' => break,
                0 => return self.err("EOF reached in string"),
                _ => c = self.getc(),
            }
        }
        self.getc();
        Ok(Token::String(start, self.pos - 1))
    }
    fn err(&mut self, message: &str) -> Result<Token, E> {
        Err(E::new(message))
    }
}

#[derive(Debug)]
pub struct E {
    pub _message: LString,
}
impl E {
    pub fn new(s: &str) -> Self {
        Self {
            _message: LString::from(s),
        }
    }
}
