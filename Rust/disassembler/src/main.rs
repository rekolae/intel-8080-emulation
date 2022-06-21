/*
Intel 8080 disassembler written in rust
*/

mod errors;
mod disassembler;

use std::env;
use std::path::PathBuf;

use errors::DisassemblerError;


fn get_input_file() -> Result<PathBuf, DisassemblerError> {

    // Skip first arg that has the executable path
    let iter = match env::args().nth(1) {
        Some(i) => {
            i
        },
        
        None => {
            return Err(DisassemblerError::FilePathNotGiven);
        },
    };
    
    let file_path = PathBuf::from(&iter);

    if !file_path.exists() {
        return Err(DisassemblerError::FilePathNotFound(iter));
    }

    Ok(file_path)
}


fn main() -> Result<(), DisassemblerError>{

    println!("\n### Initializing disassembler! ###\n");

    let path = get_input_file()?;
    disassembler::disassemble(path)?;

    println!("### Disassembler exiting! ###");

    Ok(())
}
