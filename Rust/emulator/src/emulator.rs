use std::path::PathBuf;
use std::fs::read;

use crate::errors::EmulatorError;


pub struct Intel8080 {
    registers: Registers,
    mem: Vec<u8>,

    // Flag for when HLT (halt) instruction is executed
    halted: bool,
}

struct Registers {
    // Registers grouped in pairs
    a: u8,
    f: FlagRegister,

    b: u8,
    c: u8,

    d: u8,
    e: u8,

    h: u8,
    l: u8,

    // Special registers
    sp: u16,    // Stack Pointer
    pc: usize,  // Program Counter
    int: u8,    // Interrupt enable
}

#[derive(Debug)]
struct FlagRegister {
    sign: bool,         // Bit 7 Sign flag - Set if MSB of the result is 1, unset if not
    zero: bool,         // Bit 6 Zero flag - Set if result is 0, unset if not 
    // Always 0            Bit 5 Not used
    aux_carry: bool,    // Bit 4 Auxiliary Carry flag
    // Always 0            Bit 3 Not used
    parity: bool,       // Bit 2 Parity flag - Set if value is even, unset if not
    // Always 1            Bit 1 Not used
    carry: bool,        // Bit 0 Carry flag
}

impl Registers {
    pub fn get_reg_pair(&self, pair: &str) -> u16 {
        // Create a single u16 value from the u8 reg pairs by shifting the upper reg by 8
        let data: u16 = match pair {
            "BC" => {
                (self.b as u16) << 8 | self.c as u16
            },
            
            "DE" => {
                (self.d as u16) << 8 | self.e as u16
            },

            "HL" => {
                (self.h as u16) << 8 | self.l as u16
            },

            _ => {
                panic!("Unknown reg pair {}", pair);
            },
        };

        data
    }

    pub fn set_reg_pair(&mut self, reg_pair: &str, val: u16) {
        let (mut high, mut low) = match reg_pair {
            "BC" => {
                 (&mut self.b, &mut self.c)
            },
            
            "DE" => {
                (&mut self.d, &mut self.e)
            },

            "HL" => {
                (&mut self.h, &mut self.l)
            },

            _ => {
                panic!("Unknown reg pair {}", reg_pair);
            },
        };

        *high = (val >> 8) as u8;
        *low  = val as u8;
    }

    pub fn get_reg(&self, reg: &str) -> u8 {
        match reg {
            "B" => {
                self.b
            },
            
            "C" => {
                self.c
            },

            "D" => {
                self.d
            },

            "E" => {
                self.e
            },

            "H" => {
                self.h
            },

            "L" => {
                self.l
            },

            "A" => {
                self.a
            },

            _ => {
                panic!("Unknown reg {}", reg);
            },
        }
    }

    pub fn set_reg(&mut self, reg_name: &str, val: u8) {
        let mut reg = match reg_name {
            "B" => {
                &mut self.b
            },
            
            "C" => {
                &mut self.c
            },

            "D" => {
                &mut self.d
            },

            "E" => {
                &mut self.e
            },

            "H" => {
                &mut self.h
            },

            "L" => {
                &mut self.l
            },

            "A" => {
                &mut self.a
            },

            _ => {
                panic!("Unknown reg {}", reg_name);
            },
        };

        *reg = val;
    }
}

impl FlagRegister {

    // Return true if even amount of 1 in the value, false otherwise
    fn check_parity(&self, mut val: u8) -> bool {
        // This here is a bit of cool black magic! The shifting causes every bit to be accumulated to the LSB, so if the 
        // value has even number of ones the LSB will be zero, because of XOR! Let's use 0b11101010 as an example:

        // XOR: 11101010
        //      00001110
        // ->   11100100
        val ^= val >> 4;

        // XOR: 11100100
        //      00111001
        // ->   11011101
        val ^= val >> 2;

        // XOR: 11011101
        //      01101110
        // ->   10110011
        val ^= val >> 1;

        // Example value has 5 ones and the LSB is one, so it works! We just have to convert it to a bool
        val & 0b00000001 == 0
    }

    pub fn set_artihmetic_flags(&mut self, val: u8) {

        // If MSB is one, then set sign flag
        self.sign = val >> 7 == 1;

        // I think you can figure this one out
        self.zero = val == 0;

        // Check if value has even amount of ones
        self.parity = self.check_parity(val);
    }
}

impl Intel8080 {
    pub fn new() -> Self {

        // Initialize the CPU with 0 and false values
        Intel8080 {
            registers: Registers {
                a: 0x00,
                f: FlagRegister {
                    sign: false,
                    zero: false,
                    aux_carry: false,
                    parity: false,
                    carry: false,
                },
                
                b: 0x00,
                c: 0x00,
                
                d: 0x00,
                e: 0x00,
                
                h: 0x00,
                l: 0x00,
        
                sp: 0x0000,
                pc: 0x0000,
                int: 0x00,
            },
    
            // 2^16 = 64KB of memory
            mem: Vec::<u8>::with_capacity(0x10000),

            halted: false,
        }
    }

    // Read the whole rom into memory, if rom is in parts it must be combined manually into a single file
    pub fn read_rom_to_mem(&mut self, input_file: PathBuf) -> Result<(), EmulatorError> {
        // Should be a "free operation" because no memory needs to be allocated for the vec
        for byte in read(input_file)?.iter() {
            self.mem.push(*byte);
        }
    
        Ok(())
    }

    fn advance_pc(&mut self, val: usize) {
        self.registers.pc += val;
    }

    // No operation
    fn nop(&mut self) {
        self.advance_pc(1);
    }

    // LXI reg pair - Load to reg pair the immediate value from addr
    fn lxi(&mut self, reg_pair: &str) {
        let val: u16 = (self.mem[self.registers.pc + 1] as u16) << 8 | self.mem[self.registers.pc + 2] as u16;
        self.registers.set_reg_pair(reg_pair, val);
        
        self.advance_pc(3);
    }

    // STAX reg pair - Store accumulator to the mem addr in reg pair
    fn stax(&mut self, reg_pair: &str) {
        let mem_addr: usize = self.registers.get_reg_pair(reg_pair).into();
        self.mem[mem_addr] = self.registers.a;
        
        self.advance_pc(1);
    }

    // INX reg pair - Increment reg pair value
    fn inx(&mut self, reg_pair: &str) {
        self.registers.set_reg_pair(reg_pair, self.registers.get_reg_pair(reg_pair) + 1);
        self.advance_pc(1);
    }

    // INR reg - Increment reg value
    fn inr(&mut self, reg_name: &str) {
        
        let val: u8 = self.registers.get_reg(reg_name) + 1;
        self.registers.set_reg(reg_name, val);
        self.registers.f.set_artihmetic_flags(val);

        /*
        Check that the lower four bits are all 0 by ANDing 0xF to the value e.g.
            01110000 (Some value that was incremented by one)
            00001111 (0xF)
            00000000 -> True, carry happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (val & 0xf) == 0;
        
        self.advance_pc(1);
    }

    // DCR reg - Decrement reg value
    fn dcr(&mut self, reg_name: &str) {
        let val: u8 = self.registers.get_reg(reg_name) - 1;
        self.registers.set_reg(reg_name, val);
        self.registers.f.set_artihmetic_flags(val);

        /*
        Not quite sure why the flag is set when the decremented value's lower four bits are ones e.g.
            01101111 (Some value that was decremented by one)
            00001111 (0xF)
            00001111 -> False, because borrow I guess?
        */
        self.registers.f.aux_carry = (val & 0xF) != 0xF;

        self.advance_pc(1);
    }

    // MVI reg - Move immediate value to reg
    fn mvi(&mut self, reg_name: &str) {
        self.registers.set_reg(reg_name, self.mem[self.registers.pc + 1]);
        self.advance_pc(2);
    }

    // DAD reg pair - Add given register pair to register pair HL
    fn dad(&mut self, reg_pair: &str) {
        let val: u16 = self.registers.get_reg_pair(reg_pair) + self.registers.get_reg_pair("HL");
        self.registers.set_reg_pair("HL", val);

        // Check if adding the two reg pairs overflows over u16
        self.registers.f.carry = val > 0xFFFF;

        self.advance_pc(1);
    }

    // LDAX reg pair - Load to accumulator indirect value from reg pair
    fn ldax(&mut self, reg_pair: &str) {
        let mem_addr: usize = self.registers.get_reg_pair(reg_pair).into();
        self.registers.set_reg("A", self.mem[mem_addr]);

        self.advance_pc(1);
    }

    // DCX reg pair - Decrement reg pair value
    fn dcx(&mut self, reg_pair: &str) {
        self.registers.set_reg_pair(reg_pair, self.registers.get_reg_pair(reg_pair) - 1);
        self.advance_pc(1);
    }

    // Execute the matching opcode and set the registers to their corresponding state
    fn exec_opcode(&mut self) {
        match self.mem[self.registers.pc] {
        
            // 0x0x
            0x00 => {
                // NOP - No operation
                self.nop();
            },
            0x01 => {
                // LXI B - Load reg pair BC immediate
                self.lxi("BC");
            },
            0x02 => {
                // STAX B - Store accumulator to mem addr in reg pair BC
                self.stax("BC");
            },
            0x03 => {
                // INX B - Increment reg pair BC
                self.inx("BC");
            },
            0x04 => {
                // INR B - Increment reg B
                self.inr("B");
            },
            0x05 => {
                // DCR B - Decrement reg B
                self.dcr("B");
            },
            0x06 => {
                // MVI B - Move immediate B
                self.mvi("B");
            },
            0x07 => {
                // RLC - Rotate accumulator (reg A) left
                let val = self.registers.get_reg("A");

                // Copy the MSB to the carry flag 
                self.registers.f.carry = (val >> 7) == 1;

                // Rotate reg left by one and use OR to move the MSB as LSB
                let shifted_val: u8 = (val << 1) | (self.registers.f.carry as u8);
                self.registers.set_reg("A", shifted_val);

                self.advance_pc(1);
            },
            0x08 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x09 => {
                // DAD B - Add register pair BC to register pair HL
                self.dad("BC");
            },
            0x0a => {
                // LDAX B - Load accumulator indirect from reg pair BC
                self.ldax("BC");
            },
            0x0b => {
                // DCX B - Decrement reg pair BC
                self.dcx("BC");
            },
            0x0c => {
                // INR C - Increment reg C
                self.inr("C");
            },
            0x0d => {
                // DCR C
                self.dcr("C");
            },
            0x0e => {
                // MVI C - Move immediate C
                self.mvi("C");
            },
            0x0f => {
                // RRC - Rotate accumulator (reg A) right
                let val = self.registers.get_reg("A");

                // Copy the LSB to the carry flag 
                self.registers.f.carry = (val & 0x1) == 1;

                // Rotate reg right by one and use OR to move the LSB as MSB
                let shifted_val: u8 = (val >> 1) | ((self.registers.f.carry as u8) << 7);
                self.registers.set_reg("A", shifted_val);

                self.advance_pc(1);
            },
            /*
            // 0x1x
            0x10 => {println!("NOP*");},
            0x11 => {println!("{:<width$} #{:#04x}{:02x}", "LXI D", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x12 => {println!("STAX D");},
            0x13 => {println!("INX D");},
            0x14 => {println!("INR D");},
            0x15 => {println!("DCR D");},
            0x16 => {println!("{:<width$} #{:#04x}", "MVI D", bytes[pc+1]); opcode_offset=2;},
            0x17 => {println!("RAL");},
            0x18 => {println!("NOP*");},
            0x19 => {println!("DAD D");},
            0x1a => {println!("LDAX D");},
            0x1b => {println!("DCX D");},
            0x1c => {println!("INR E");},
            0x1d => {println!("DCR E");},
            0x1e => {println!("{:<width$} #{:#04x}", "MVI E", bytes[pc+1]); opcode_offset=2;},
            0x1f => {println!("RAR");},
    
            // 0x2x
            0x20 => {println!("NOP*");},
            0x21 => {println!("{:<width$} #{:#04x}{:02x}", "LXI H", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x22 => {println!("{:<width$} {:#04x}{:02x}", "SHLD", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x23 => {println!("INX H");},
            0x24 => {println!("INR H");},
            0x25 => {println!("DCR H");},
            0x26 => {println!("{:<width$} #{:#04x}", "MVI H", bytes[pc+1]); opcode_offset=2;},
            0x27 => {println!("DAA");},
            0x28 => {println!("NOP*");},
            0x29 => {println!("DAD H");},
            0x2a => {println!("{:<width$} {:#04x}{:02x}", "LHLD", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x2b => {println!("DCX H");},
            0x2c => {println!("INR L");},
            0x2d => {println!("DCR L");},
            0x2e => {println!("{:<width$} #{:#04x}", "MVI L", bytes[pc+1]); opcode_offset=2;},
            0x2f => {println!("CMA");},
    
            // 0x3x
            0x30 => {println!("NOP*");},
            0x31 => {println!("{:<width$} #{:#04x}{:02x}", "LXI SP", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x32 => {println!("{:<width$} {:#04x}{:02x}", "STA", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x33 => {println!("INX SP");},
            0x34 => {println!("INR M");},
            0x35 => {println!("DCR M");},
            0x36 => {println!("{:<width$} #{:#04x}", "MVI M", bytes[pc+1]); opcode_offset=2;},
            0x37 => {println!("STC");},
            0x38 => {println!("NOP*");},
            0x39 => {println!("DAD SP");},
            0x3a => {println!("{:<width$} {:#04x}{:02x}", "LDA", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0x3b => {println!("DCX SP");},
            0x3c => {println!("INR A");},
            0x3d => {println!("DCR A");},
            0x3e => {println!("{:<width$} #{:#04x}", "MVI A", bytes[pc+1]); opcode_offset=2;},
            0x3f => {println!("CMC");},
    
            // 0x4x
            0x40 => {println!("MOV B,B");},
            0x41 => {println!("MOV B,C");},
            0x42 => {println!("MOV B,D");},
            0x43 => {println!("MOV B,E");},
            0x44 => {println!("MOV B,H");},
            0x45 => {println!("MOV B,L");},
            0x46 => {println!("MOV B,M");},
            0x47 => {println!("MOV B,A");},
            0x48 => {println!("MOV C,B");},
            0x49 => {println!("MOV C,C");},
            0x4a => {println!("MOV C,D");},
            0x4b => {println!("MOV C,E");},
            0x4c => {println!("MOV C,H");},
            0x4d => {println!("MOV C,L");},
            0x4e => {println!("MOV C,M");},
            0x4f => {println!("MOV C,A");},
    
            // 0x5x
            0x50 => {println!("MOV D,B");},
            0x51 => {println!("MOV D,C");},
            0x52 => {println!("MOV D,D");},
            0x53 => {println!("MOV D,E");},
            0x54 => {println!("MOV D,H");},
            0x55 => {println!("MOV D,L");},
            0x56 => {println!("MOV D,M");},
            0x57 => {println!("MOV D,A");},
            0x58 => {println!("MOV E,B");},
            0x59 => {println!("MOV E,C");},
            0x5a => {println!("MOV E,D");},
            0x5b => {println!("MOV E,E");},
            0x5c => {println!("MOV E,H");},
            0x5d => {println!("MOV E,L");},
            0x5e => {println!("MOV E,M");},
            0x5f => {println!("MOV E,A");},
    
            // 0x6x
            0x60 => {println!("MOV H,B");},
            0x61 => {println!("MOV H,C");},
            0x62 => {println!("MOV H,D");},
            0x63 => {println!("MOV H,E");},
            0x64 => {println!("MOV H,H");},
            0x65 => {println!("MOV H,L");},
            0x66 => {println!("MOV H,M");},
            0x67 => {println!("MOV H,A");},
            0x68 => {println!("MOV L,B");},
            0x69 => {println!("MOV L,C");},
            0x6a => {println!("MOV L,D");},
            0x6b => {println!("MOV L,E");},
            0x6c => {println!("MOV L,H");},
            0x6d => {println!("MOV L,L");},
            0x6e => {println!("MOV L,M");},
            0x6f => {println!("MOV L,A");},
    
            // 0x7x
            0x70 => {println!("MOV M,B");},
            0x71 => {println!("MOV M,C");},
            0x72 => {println!("MOV M,D");},
            0x73 => {println!("MOV M,E");},
            0x74 => {println!("MOV M,H");},
            0x75 => {println!("MOV M,L");},
            0x76 => {println!("HLT");},
            0x77 => {println!("MOV M,A");},
            0x78 => {println!("MOV A,B");},
            0x79 => {println!("MOV A,C");},
            0x7a => {println!("MOV A,D");},
            0x7b => {println!("MOV A,E");},
            0x7c => {println!("MOV A,H");},
            0x7d => {println!("MOV A,L");},
            0x7e => {println!("MOV A,M");},
            0x7f => {println!("MOV A,A");},
    
            // 0x8x
            0x80 => {println!("ADD B");},
            0x81 => {println!("ADD C");},
            0x82 => {println!("ADD D");},
            0x83 => {println!("ADD E");},
            0x84 => {println!("ADD H");},
            0x85 => {println!("ADD L");},
            0x86 => {println!("ADD M");},
            0x87 => {println!("ADD A");},
            0x88 => {println!("ADC B");},
            0x89 => {println!("ADC C");},
            0x8a => {println!("ADC D");},
            0x8b => {println!("ADC E");},
            0x8c => {println!("ADC H");},
            0x8d => {println!("ADC L");},
            0x8e => {println!("ADC M");},
            0x8f => {println!("ADC A");},
    
            // 0x9x
            0x90 => {println!("SUB B");},
            0x91 => {println!("SUB C");},
            0x92 => {println!("SUB D");},
            0x93 => {println!("SUB E");},
            0x94 => {println!("SUB H");},
            0x95 => {println!("SUB L");},
            0x96 => {println!("SUB M");},
            0x97 => {println!("SUB A");},
            0x98 => {println!("SBB B");},
            0x99 => {println!("SBB C");},
            0x9a => {println!("SBB D");},
            0x9b => {println!("SBB E");},
            0x9c => {println!("SBB H");},
            0x9d => {println!("SBB L");},
            0x9e => {println!("SBB M");},
            0x9f => {println!("SBB A");},
    
            // 0xax
            0xa0 => {println!("ANA B");},
            0xa1 => {println!("ANA C");},
            0xa2 => {println!("ANA D");},
            0xa3 => {println!("ANA E");},
            0xa4 => {println!("ANA H");},
            0xa5 => {println!("ANA L");},
            0xa6 => {println!("ANA M");},
            0xa7 => {println!("ANA A");},
            0xa8 => {println!("XRA B");},
            0xa9 => {println!("XRA C");},
            0xaa => {println!("XRA D");},
            0xab => {println!("XRA E");},
            0xac => {println!("XRA H");},
            0xad => {println!("XRA L");},
            0xae => {println!("XRA M");},
            0xaf => {println!("XRA A");},
    
            // 0xbx
            0xb0 => {println!("ORA B");},
            0xb1 => {println!("ORA C");},
            0xb2 => {println!("ORA D");},
            0xb3 => {println!("ORA E");},
            0xb4 => {println!("ORA H");},
            0xb5 => {println!("ORA L");},
            0xb6 => {println!("ORA M");},
            0xb7 => {println!("ORA A");},
            0xb8 => {println!("CMP B");},
            0xb9 => {println!("CMP C");},
            0xba => {println!("CMP D");},
            0xbb => {println!("CMP E");},
            0xbc => {println!("CMP H");},
            0xbd => {println!("CMP L");},
            0xbe => {println!("CMP M");},
            0xbf => {println!("CMP A");},
    
            // 0xcx
            0xc0 => {println!("RNZ");},
            0xc1 => {println!("POP B");},
            0xc2 => {println!("{:<width$} {:#04x}{:02x}", "JNZ", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xc3 => {println!("{:<width$} {:#04x}{:02x}", "JMP", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xc4 => {println!("{:<width$} {:#04x}{:02x}", "CNZ", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xc5 => {println!("PUSH B");},
            0xc6 => {println!("{:<width$} #{:#04x}", "ADI", bytes[pc+1]); opcode_offset=2;},
            0xc7 => {println!("RST 0");},
            0xc8 => {println!("RZ");},
            0xc9 => {println!("RET");},
            0xca => {println!("{:<width$} {:#04x}{:02x}", "JZ", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xcb => {println!("{:<width$} {:#04x}{:02x}", "JMP*", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xcc => {println!("{:<width$} {:#04x}{:02x}", "CZ", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xcd => {println!("{:<width$} {:#04x}{:02x}", "CALL", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xce => {println!("{:<width$} #{:#04x}", "ACI", bytes[pc+1]); opcode_offset=2;},
            0xcf => {println!("RST 1");},
    
            // 0xdx
            0xd0 => {println!("RNC");},
            0xd1 => {println!("POP D");},
            0xd2 => {println!("{:<width$} {:#04x}{:02x}", "JNC", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xd3 => {println!("{:<width$} #{:#04x}", "OUT", bytes[pc+1]); opcode_offset=2;},
            0xd4 => {println!("{:<width$} {:#04x}{:02x}", "CNC", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xd5 => {println!("PUSH D");},
            0xd6 => {println!("{:<width$} #{:#04x}", "SUI", bytes[pc+1]); opcode_offset=2;},
            0xd7 => {println!("RST 2");},
            0xd8 => {println!("RC");},
            0xd9 => {println!("RET*");},
            0xda => {println!("{:<width$} {:#04x}{:02x}", "JC", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xdb => {println!("{:<width$} #{:#04x}", "IN", bytes[pc+1]); opcode_offset=2;},
            0xdc => {println!("{:<width$} {:#04x}{:02x}", "CC", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xdd => {println!("{:<width$} {:#04x}{:02x}", "CALL*", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xde => {println!("{:<width$} #{:#04x}", "SBI", bytes[pc+1]); opcode_offset=2;},
            0xdf => {println!("RST 3");},
    
            // 0xex
            0xe0 => {println!("RPO");},
            0xe1 => {println!("POP H");},
            0xe2 => {println!("{:<width$} {:#04x}{:02x}", "JPO", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xe3 => {println!("XTHL");},
            0xe4 => {println!("{:<width$} {:#04x}{:02x}", "CPO", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xe5 => {println!("PUSH H");},
            0xe6 => {println!("{:<width$} #{:#04x}", "ANI", bytes[pc+1]); opcode_offset=2;},
            0xe7 => {println!("RST 4");},
            0xe8 => {println!("RPE");},
            0xe9 => {println!("PCHL");},
            0xea => {println!("{:<width$} {:#04x}{:02x}", "JPE", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xeb => {println!("XCHG");},
            0xec => {println!("{:<width$} {:#04x}{:02x}", "CPE", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xed => {println!("{:<width$} {:#04x}{:02x}", "CALL*", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xee => {println!("{:<width$} #{:#04x}", "XRI", bytes[pc+1]); opcode_offset=2;},
            0xef => {println!("RST 5");},
    
            // 0xfx
            0xf0 => {println!("RP");},
            0xf1 => {println!("POP PSW");},
            0xf2 => {println!("{:<width$} {:#04x}{:02x}", "JP", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xf3 => {println!("DI");},
            0xf4 => {println!("{:<width$} {:#04x}{:02x}", "CP", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xf5 => {println!("PUSH PSW");},
            0xf6 => {println!("{:<width$} #{:#04x}", "ORI", bytes[pc+1]); opcode_offset=2;},
            0xf7 => {println!("RST 6");},
            0xf8 => {println!("RM");},
            0xf9 => {println!("SPHL");},
            0xfa => {println!("{:<width$} {:#04x}{:02x}", "JM", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xfb => {println!("EI");},
            0xfc => {println!("{:<width$} {:#04x}{:02x}", "CM", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xfd => {println!("{:<width$} {:#04x}{:02x}", "CALL*", bytes[pc+2], bytes[pc+1]); opcode_offset=3;},
            0xfe => {println!("{:<width$} #{:#04x}", "CPI", bytes[pc+1]); opcode_offset=2;},
            0xff => {println!("RST 7");},
            */
            _ => {/* Bork */},
        };
    }

    pub fn emulate(&mut self) {
        while !self.halted {
            self.exec_opcode()
        }
    }

    pub fn test(&mut self) {
        self.registers.set_reg("A", 0b10101010);
        println!("FLAGS: {:#?}\n", self.registers.f);
        println!("A: {:08b}\n", self.registers.get_reg("A"));
        
        let val = self.registers.get_reg("A");

        // Copy the LSB to the carry flag 
        self.registers.f.carry = (val & 0x1) == 1;

        // Rotate reg right by one and use OR to move the LSB as MSB
        let shifted_val: u8 = (val >> 1) | ((self.registers.f.carry as u8) << 7);
        self.registers.set_reg("A", shifted_val);

        println!("FLAGS: {:#?}\n", self.registers.f);
        println!("A: {:08b}\n", self.registers.get_reg("A"));
    }
}
