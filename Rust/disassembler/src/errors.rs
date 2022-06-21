use std::fmt::{Display, Formatter, Result, Debug};
use std::io;

pub enum DisassemblerError {
    FilePathNotGiven,
    FilePathNotFound(String),
    FileCantOpen(String),
}

fn get_err_msg(err: &DisassemblerError) -> String {
    match err {
        DisassemblerError::FilePathNotGiven => format!("File path was not given!"),
        DisassemblerError::FilePathNotFound(s) => format!("File path '{s}' was not valid!"),
        DisassemblerError::FileCantOpen(s) => format!("Couldn't open file '{s}'!"),
    }
}

impl Display for DisassemblerError {
  fn fmt(&self, f: &mut Formatter) -> Result {
    write!(f, "{}", get_err_msg(self))
  }
}

impl Debug for DisassemblerError {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", get_err_msg(self))
    }
}

impl From<io::Error> for DisassemblerError {
    fn from(error: io::Error) -> Self {
        DisassemblerError::FileCantOpen(error.to_string()) 
    }
}