/*
Intel 8080 disassembler written in rust
*/

mod errors;
mod emulator;

use std::env;
use std::path::PathBuf;

use errors::EmulatorError;
use emulator::Intel8080;


fn get_input_file() -> Result<PathBuf, EmulatorError> {

    // Skip first arg that has the executable path
    let iter = match env::args().nth(1) {
        Some(i) => {
            i
        },
        
        None => {
            return Err(EmulatorError::FilePathNotGiven);
        },
    };
    
    let file_path = PathBuf::from(&iter);

    if !file_path.exists() {
        return Err(EmulatorError::FilePathNotFound(iter));
    }

    Ok(file_path)
}


fn main() -> Result<(), EmulatorError>{
    println!("\n### Initializing emulator! ###\n");

    let path = get_input_file()?;
    let mut cpu = Intel8080::new();

    cpu.read_rom_to_mem(path)?;
    //cpu.emulate();

    cpu.test();

    println!("\n### Emulator exiting! ###");
    Ok(())
}
