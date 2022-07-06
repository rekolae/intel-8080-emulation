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
            "BC" => (self.b as u16) << 8 | self.c as u16,
            "DE" => (self.d as u16) << 8 | self.e as u16,
            "HL" => (self.h as u16) << 8 | self.l as u16,
            _ => panic!("Unknown reg pair {}", pair),
        };

        data
    }

    pub fn set_reg_pair(&mut self, reg_pair: &str, val: u16) {
        let (high, low) = match reg_pair {
            "BC" => (&mut self.b, &mut self.c),
            "DE" => (&mut self.d, &mut self.e),
            "HL" => (&mut self.h, &mut self.l),
            _ => panic!("Unknown reg pair {}", reg_pair),
        };

        *high = (val >> 8) as u8;
        *low  = val as u8;
    }

    pub fn get_reg(&self, reg: &str) -> u8 {
        match reg {
            "B" => self.b,
            "C" => self.c,
            "D" => self.d,
            "E" => self.e,
            "H" => self.h,
            "L" => self.l,
            "A" => self.a,
            _ => panic!("Unknown reg {}", reg),
        }
    }

    pub fn set_reg(&mut self, reg_name: &str, val: u8) {
        let reg = match reg_name {
            "B" => &mut self.b,
            "C" => &mut self.c,
            "D" => &mut self.d,
            "E" => &mut self.e,
            "H" => &mut self.h,
            "L" => &mut self.l,
            "A" => &mut self.a,
            _ => panic!("Unknown reg {}", reg_name),
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

        // Or could have used "val.count_ones()" like a normal person
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

    // Return the next 2 bytes in memory
    fn get_word(&self) -> u16 {
        // Take into account that the 8080 is little endian, so the first byte is actually the lower part of the value
        (self.mem[self.registers.pc + 2] as u16) << 8 | self.mem[self.registers.pc + 1] as u16
    }

    // No operation
    fn nop(&mut self) {
        self.advance_pc(1);
    }

    // LXI reg pair - Load to reg pair the immediate value from addr
    fn lxi(&mut self, reg_pair: &str) {
        let val: u16 = self.get_word();
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
        self.registers.set_reg_pair(reg_pair, self.registers.get_reg_pair(reg_pair).wrapping_add(1));
        self.advance_pc(1);
    }

    // INR reg - Increment reg value
    fn inr(&mut self, reg_name: &str) {
        
        let val: u8 = self.registers.get_reg(reg_name);
        let incremented_val: u8 = val.wrapping_add(1);
        self.registers.set_reg(reg_name, incremented_val);
        self.registers.f.set_artihmetic_flags(incremented_val);

        /*
        Check if incremented value is greater than 0xF by ANDing 0xF to the pre-incremented value and adding one e.g.
            01101111    Some value before it was incremented by one
            00001111    0xF
            00001111    AND operation
            00010000    increment
            Greater than 0xF -> True, carry happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (val & 0xF) + 0x01 > 0x0F;
        
        self.advance_pc(1);
    }

    // DCR reg - Decrement reg value
    fn dcr(&mut self, reg_name: &str) {
        let val: u8 = self.registers.get_reg(reg_name).wrapping_sub(1);
        self.registers.set_reg(reg_name, val);
        self.registers.f.set_artihmetic_flags(val);

        /*
        Not quite sure why the flag is not set when the decremented value's lower four bits are ones e.g.
            01101111    Some value that was decremented by one
            00001111    0xF
            00001111    AND operation
            Equal to 0xF -> False, because borrow I guess?
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
        let val: u32 = self.registers.get_reg_pair(reg_pair) as u32 + self.registers.get_reg_pair("HL") as u32;
        self.registers.set_reg_pair("HL", val as u16);

        // Check if adding the two reg pairs overflows over u16 max val
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

    // MOV dst reg, src reg - Move byte from src to dst reg
    fn mov(&mut self, dst: &str, src: &str) {
        self.registers.set_reg(dst, self.registers.get_reg(src));
        self.advance_pc(1);
    }

    // MOV dst reg, byte from mem - Move byte from mem pointed to by reg pair HL to dst reg
    fn mov_m(&mut self, dst: &str) {
        let addr: usize = self.registers.get_reg_pair("HL").into();
        self.registers.set_reg(dst, self.mem[addr]);
        self.advance_pc(1);
    }

    // MOV src reg, byte from mem - Move byte from src reg to mem pointed to by reg pair HL
    fn mov_r(&mut self, src: &str) {
        let addr: usize = self.registers.get_reg_pair("HL").into();
        self.mem[addr] = self.registers.get_reg(src);
        self.advance_pc(1);
    }

    // ADD val - Add val to accumulator
    fn add(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let added_val: u8 = reg_a.wrapping_add(val);

        self.registers.set_reg("A", added_val);
        self.registers.f.set_artihmetic_flags(added_val);

        /*
        Check if added value is greater than 0xF by ANDing 0xF to the pre-added value and the value to be added and
        adding them together e.g.
            01101111    Reg A before a value was added to it
            00001111    0xF
            00001111    AND operation

            00010110    Some value to be added (26 in this case for example)
            00001111    0xF
            00000110    AND operation

            00001111    Reg A AND 0xF
            00000110    Some val AND 0xF
            00010101    addition of the two values above
            Greater than 0xF -> True, carry happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (reg_a & 0xF) + (val & 0xF) > 0x0F;

        self.advance_pc(1);
    }

    // ADC val - Add val to accumulator with carry
    fn adc(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let carry: u8 = self.registers.f.carry as u8;
        let added_val: u8 = reg_a.wrapping_add(val).wrapping_add(carry);

        self.registers.set_reg("A", added_val);
        self.registers.f.set_artihmetic_flags(added_val);

        /*
        Check if added value is greater than 0xF by ANDing 0xF to the pre-added value and the value to be added and
        adding them together + the carry e.g.
            01101111    Reg A before a value was added to it
            00001111    0xF
            00001111    AND operation

            00010110    Some value to be added (26 in this case for example)
            00001111    0xF
            00000110    AND operation

            00001111    Reg A AND 0xF
            00000110    Some val AND 0xF
            00000001    Carry
            00010110    addition of the two values above and addition the value of carry
            Greater than 0xF -> True, carry happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (reg_a & 0xF) + (val & 0xF) + carry > 0x0F;

        self.advance_pc(1);
    }

    // SUB val - Subtract val from accumulator
    fn sub(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let subtracted_val: u8 = reg_a.wrapping_sub(val);

        self.registers.set_reg("A", subtracted_val);
        self.registers.f.set_artihmetic_flags(subtracted_val);
        self.registers.f.carry = reg_a < val;

        /*
        Check if subtracting the given value and reg A that have been casted as integers and ANDed with 0x0F results in
        a positive value or not e.g.
            01101101    Reg A (casted as an integer) before a value was subtracted from it
            00001111    0x0F
            00001101    AND operation

            00011110    Some value (casted as an integer) to be subtracted
            00001111    0x0F
            00001110    AND operation

            00001101    Reg A AND 0x0F
            00001110    Some val AND 0x0F
            10000001    subtract lower from upper value
            Less than 0x0 -> False, borrow happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (reg_a as i8 & 0x0F) - (val as i8 & 0x0F) >= 0x0;

        self.advance_pc(1);
    }

    // SBB val - Subtract val from accumulator with borrow
    fn sbb(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let carry: u8 = self.registers.f.carry as u8;
        let subtracted_val: u8 = reg_a.wrapping_sub(val).wrapping_sub(carry);

        self.registers.set_reg("A", subtracted_val);
        self.registers.f.set_artihmetic_flags(subtracted_val);
        self.registers.f.carry = reg_a < val + carry;

        /*
        Check if subtracting the given value and reg A that have been casted as integers and ANDed with 0x0F + the
        possible carry/borrow results in a positive value or not e.g.
            01101101    Reg A (casted as an integer) before a value was subtracted from it
            00001111    0x0F
            00001101    AND operation

            00011110    Some value (casted as an integer) to be subtracted
            00001111    0x0F
            00001110    AND operation

            00001101    Reg A AND 0x0F
            00001110    Some val AND 0x0F
            00000001    Carry/Borrow
            10000010    subtract lower from upper value and the carry/borrow
            Less than 0x0 -> False, borrow happened from lower 4 bits to the upper ones
        */
        self.registers.f.aux_carry = (reg_a as i8 & 0x0F) - (val as i8 & 0x0F) - (carry as i8) >= 0x0;

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
                // DCR C - Decrement reg C
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
            
            // 0x1x
            0x10 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x11 => {
                // LXI D - Load reg pair DE immediate
                self.lxi("DE");
            },
            0x12 => {
                // STAX D - Store accumulator to mem addr in reg pair DE
                self.stax("DE");
            },
            0x13 => {
                // INX D - Increment reg pair DE
                self.inx("DE");
            },
            0x14 => {
                // INR D - Increment reg D
                self.inr("D");
            },
            0x15 => {
                // DCR D - Decrement reg D
                self.dcr("D");
            },
            0x16 => {
                // MVI D - Move immediate D
                self.mvi("D");
            },
            0x17 => {
                // RAL - Rotate accumulator (reg A) left through carry
                let val = self.registers.get_reg("A");

                // Save current carry flag val before replacing it with the MSB of reg A
                let temp: u8 = self.registers.f.carry as u8;

                // Copy the MSB to the carry flag
                self.registers.f.carry = (val >> 7) == 1;

                // Rotate reg left by one and use OR to move the previous carry bit as LSB
                let shifted_val: u8 = (val << 1) | temp;
                self.registers.set_reg("A", shifted_val);

                self.advance_pc(1);
            },
            0x18 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x19 => {
                // DAD D - Add register pair DE to register pair HL
                self.dad("DE");
            },
            0x1a => {
                // LDAX D - Load accumulator indirect from reg pair DE
                self.ldax("DE");
            },
            0x1b => {
                // DCX D - Decrement reg pair DE
                self.dcx("DE");
            },
            0x1c => {
                // INR E - Increment reg E
                self.inr("E");
            },
            0x1d => {
                // DCR E - Decrement reg E
                self.dcr("E");
            },
            0x1e => {
                // MVI E - Move immediate E
                self.mvi("E");
            },
            0x1f => {
                // RAR - Rotate accumulator (reg A) right through carry
                let val = self.registers.get_reg("A");

                // Save current carry flag val before replacing it with the LSB of reg A
                let temp: u8 = self.registers.f.carry as u8;

                // Copy the LSB to the carry flag
                self.registers.f.carry = (val & 0x1) == 1;

                // Rotate reg right by one and use OR to move the previous carry bit as MSB
                let shifted_val: u8 = (val >> 1) | (temp << 7);
                self.registers.set_reg("A", shifted_val);

                self.advance_pc(1);
            },
    
            // 0x2x
            0x20 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x21 => {
                // LXI H - Load reg pair HL immediate
                self.lxi("HL");
            },
            0x22 => {
                // SHLD - Store reg H and reg L into mem addr given in pc+1 and pc+2
                let h: u8 = self.registers.get_reg("H");
                let l: u8 = self.registers.get_reg("L");

                let addr: u16 = self.get_word();

                self.mem[addr as usize] = l;
                self.mem[(addr + 1) as usize] = h;

                self.advance_pc(3);
            },
            0x23 => {
                // INX H - Increment reg pair HL
                self.inx("HL");
            },
            0x24 => {
                // INR H - Increment reg H
                self.inr("H");
            },
            0x25 => {
                // DCR H - Decrement reg H
                self.dcr("H");
            },
            0x26 => {
                // MVI H - Move immediate H
                self.mvi("H");
            },
            0x27 => {
                // DAA - Decimal adjust accumulator

                // Get the lower 4 bits of the accumulator
                let lower: u8 = self.registers.get_reg("A") & 0xF;

                // If lower 4 bits is greater than 9 or aux carry is set -> 6 is added to the lower 4 bits of the reg A
                if lower > 9 || self.registers.f.aux_carry {

                    // If the lower 4 bits overflow because of the addition, set aux carry flag, otherwise clear it
                    if lower + 6 > 0xF {
                        self.registers.f.aux_carry = true;
                    } else {
                        self.registers.f.aux_carry = false;
                    }

                    // Use wrapping_add to manage possible overflows
                    self.registers.set_reg("A", self.registers.get_reg("A").wrapping_add(0x6));
                }

                // Get upper 4 bits of the accumulator after it might have been incremented
                let upper: u8 = self.registers.get_reg("A") >> 4;

                // If upper 4 bits is greater than 9 or carry is set -> 6 is added to the upper 4 bits of the reg A
                if upper > 9 || self.registers.f.carry {

                    // If the upper 4 bits overflow because of the addition, set carry flag
                    if upper + 6 > 0xF {
                        self.registers.f.carry = true;
                    }

                    // Use wrapping_add to manage possible overflows
                    self.registers.set_reg("A", self.registers.get_reg("A").wrapping_add(0x60));
                }

                // Set sign, zero and parity flags
                self.registers.f.set_artihmetic_flags(self.registers.get_reg("A"));

                self.advance_pc(1);
            },
            0x28 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x29 => {
                // DAD H - Add register pair HL to register pair HL
                self.dad("HL");
            },
            0x2a => {
                // LHLD - Load reg H and reg L from mem addr given in pc+1 and pc+2
                let addr: u16 = self.get_word();

                self.registers.set_reg("L", self.mem[addr as usize]);
                self.registers.set_reg("H", self.mem[(addr + 1) as usize]);

                self.advance_pc(3);
            },
            0x2b => {
                // DCX H - Decrement reg pair HL
                self.dcx("HL");
            },
            0x2c => {
                // INR L - Increment reg L
                self.inr("L");
            },
            0x2d => {
                // DCR L - Decrement reg L
                self.dcr("L");
            },
            0x2e => {
                // MVI E - Move immediate E
                self.mvi("L");
            },
            0x2f => {
                // CMA - Complement accumulator

                // This one is simple, just invert all of the bits
                self.registers.set_reg("A", !self.registers.get_reg("A"));

                self.advance_pc(1);
            },
    
            // 0x3x
            0x30 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x31 => {
                // LXI SP - Load reg Stack Pointer immediate
                let val: u16 = self.get_word();
                self.registers.sp = val;

                self.advance_pc(3);
            },
            0x32 => {
                // STA - Store accumulator direct
                let addr: u16 = self.get_word();
                self.mem[addr as usize] = self.registers.get_reg("A");

                self.advance_pc(3);
            },
            0x33 => {
                // INX SP - Increment stack pointer
                self.registers.sp = self.registers.sp.wrapping_add(1);
                self.advance_pc(1);
            },
            0x34 => {
                // INR M - Increment byte in memory pointed by reg pair HL
                let addr: usize = self.registers.get_reg_pair("HL").into();
                let val: u8 = self.mem[addr];
                let incremented_val: u8 = val.wrapping_add(1);

                self.mem[addr] = incremented_val;
                self.registers.f.set_artihmetic_flags(incremented_val);

                /*
                Check that the lower four bits are all 0 by ANDing 0xF to the pre-incremented value and adding one e.g.
                    01101111    Some value before it was incremented by one
                    00001111    0xF
                    00001111    AND operation
                    00010000    increment
                    Greater than 0xF -> True, carry happened from lower 4 bits to the upper ones
                */
                self.registers.f.aux_carry = (val & 0xf) + 0x01 > 0x0f;

                self.advance_pc(1);
            },
            0x35 => {
                // DCR M - Decrement byte in memory pointed by reg pair HL
                let addr: usize = self.registers.get_reg_pair("HL").into();
                let val: u8 = self.mem[addr].wrapping_sub(1);

                self.mem[addr] = val;
                self.registers.f.set_artihmetic_flags(val);

                /*
                Not quite sure why the flag is not set when the decremented value's lower four bits are ones e.g.
                    01101111    Some value that was decremented by one
                    00001111    0xF
                    00001111    AND operation
                    Equal to 0xF -> False, because borrow I guess?
                */
                self.registers.f.aux_carry = (val & 0xF) != 0xF;

                self.advance_pc(1);
            },
            0x36 => {
                // MVI M - Move immediate value to mem addr pointed by reg pair HL
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.mem[addr] = self.mem[self.registers.pc + 1];
                self.advance_pc(2);
            },
            0x37 => {
                // STC - Set carry
                self.registers.f.carry = true;
                self.advance_pc(1);
            },
            0x38 => {
                // NOP* - No operation (alternate)
                self.nop();
            },
            0x39 => {
                // DAD SP - Add SP to register pair HL
                let val: u32 = self.registers.sp as u32 + self.registers.get_reg_pair("HL") as u32;
                self.registers.sp = val as u16;

                // Check if adding the two reg pairs overflows over u16 max val
                self.registers.f.carry = val > 0xFFFF;

                self.advance_pc(1);
            },
            0x3a => {
                // LDA - Load byte from mem to accumulator
                let addr: u16 = self.get_word();
                self.registers.set_reg("A", self.mem[addr as usize]);

                self.advance_pc(3);
            },
            0x3b => {
                // DCX SP - Decrement stack pointer
                self.registers.sp = self.registers.sp.wrapping_sub(1);
                self.advance_pc(1);
            },
            0x3c => {
                // INR A - Increment reg A
                self.inr("A");
            },
            0x3d => {
                // DCR A - Decrement reg A
                self.dcr("A");
            },
            0x3e => {
                // MVI A - Move immediate A
                self.mvi("A");
            },
            0x3f => {
                // CMC - Complement carry
                self.registers.f.carry = !self.registers.f.carry;
                self.advance_pc(1);
            },
    
            // 0x4x
            0x40 => {
                // MOV B,B - Move reg B to reg B
                self.nop();
            },
            0x41 => {
                // MOV B,C - Move to reg B value from reg C
                self.mov("B", "C");
            },
            0x42 => {
                // MOV B,D - Move to reg B value from reg D
                self.mov("B", "D");
            },
            0x43 => {
                // MOV B,E - Move to reg B value from reg E
                self.mov("B", "E");
            },
            0x44 => {
                // MOV B,H - Move to reg B value from reg H
                self.mov("B", "H");
            },
            0x45 => {
                // MOV B,L - Move to reg B value from reg L
                self.mov("B", "L");
            },
            0x46 => {
                // MOV B,M - Move to reg B value from mem pointed to by reg pair HL
                self.mov_m("B");
            },
            0x47 => {
                // MOV B,A - Move to reg B value from reg A
                self.mov("B", "A");
            },
            0x48 => {
                // MOV C,B - Move to reg C value from reg B
                self.mov("C", "B");
            },
            0x49 => {
                // MOV C,C - Move reg C to reg C
                self.nop();
            },
            0x4a => {
                // MOV C,D - Move to reg C value from reg D
                self.mov("C", "D");
            },
            0x4b => {
                // MOV C,E - Move to reg C value from reg E
                self.mov("C", "E");
            },
            0x4c => {
                // MOV C,H- Move to reg C value from reg H
                self.mov("C", "H");
            },
            0x4d => {
                // MOV C,L - Move to reg C value from reg L
                self.mov("C", "L");
            },
            0x4e => {
                // MOV C,M - Move to reg C value from mem pointed to by reg pair HL
                self.mov_m("C");
            },
            0x4f => {
                // MOV C,A - Move to reg C value from reg A
                self.mov("C", "A");
            },
    
            // 0x5x
            0x50 => {
                // MOV D,B - Move to reg D value from reg B
                self.mov("D", "B");
            },
            0x51 => {
                // MOV D,C - Move to reg D value from reg C
                self.mov("D", "C");
            },
            0x52 => {
                // MOV D,D - Move reg D to reg D
                self.nop();
            },
            0x53 => {
                // MOV D,E - Move to reg D value from reg E
                self.mov("D", "E");
            },
            0x54 => {
                // MOV D,H - Move to reg D value from reg H
                self.mov("D", "H");
            },
            0x55 => {
                // MOV D,L - Move to reg D value from reg L
                self.mov("D", "L");
            },
            0x56 => {
                // MOV D,M - Move to reg D value from mem pointed to by reg pair HL
                self.mov_m("D");
            },
            0x57 => {
                // MOV D,A - Move to reg D value from reg A
                self.mov("D", "A");
            },
            0x58 => {
                // MOV E,B - Move to reg E value from reg B
                self.mov("E", "B");
            },
            0x59 => {
                // MOV E,C - Move to reg E value from reg C
                self.mov("E", "C");
            },
            0x5a => {
                // MOV E,D - Move to reg E value from reg D
                self.mov("E", "D");
            },
            0x5b => {
                // MOV E,E - Move reg E to reg E
                self.nop();
            },
            0x5c => {
                // MOV E,H - Move to reg E value from reg H
                self.mov("E", "H");
            },
            0x5d => {
                // MOV E,L - Move to reg E value from reg L
                self.mov("E", "L");
            },
            0x5e => {
                // MOV E,M - Move to reg E value from mem pointed to by reg pair HL
                self.mov_m("E");
            },
            0x5f => {
                // MOV E,A - Move to reg E value from reg A
                self.mov("E", "A");
            },
    
            // 0x6x
            0x60 => {
                // MOV H,B - Move to reg H value from reg B
                self.mov("H", "B");
            },
            0x61 => {
                // MOV H,C - Move to reg H value from reg C
                self.mov("H", "C");
            },
            0x62 => {
                // MOV H,D - Move to reg H value from reg D
                self.mov("H", "D");
            },
            0x63 => {
                // MOV H,E - Move to reg H value from reg E
                self.mov("H", "E");
            },
            0x64 => {
                // MOV H,H - Move reg H to reg H
                self.nop();
            },
            0x65 => {
                // MOV H,L - Move to reg H value from reg L
                self.mov("H", "L");
            },
            0x66 => {
                // MOV H,M - Move to reg H value from mem pointed to by reg pair HL
                self.mov_m("H");
            },
            0x67 => {
                // MOV H,A - Move to reg H value from reg A
                self.mov("H", "A");
            },
            0x68 => {
                // MOV L,B - Move to reg L value from reg B
                self.mov("L", "B");
            },
            0x69 => {
                // MOV L,C - Move to reg L value from reg C
                self.mov("L", "C");
            },
            0x6a => {
                // MOV L,D - Move to reg L value from reg D
                self.mov("L", "D");
            },
            0x6b => {
                // MOV L,E - Move to reg L value from reg E
                self.mov("L", "E");
            },
            0x6c => {
                // MOV L,H - Move to reg L value from reg H
                self.mov("L", "H");
            },
            0x6d => {
                // MOV L,L - Move reg L to reg L
                self.nop();
            },
            0x6e => {
                // MOV L,M - Move to reg L value from mem pointed to by reg pair HL
                self.mov_m("L");
            },
            0x6f => {
                // MOV L,A - Move to reg L value from reg A
                self.mov("L", "A");
            },
    
            // 0x7x
            0x70 => {
                // MOV M,B - Move to mem pointed to by reg pair HL from reg B value
                self.mov_r("B");
            },
            0x71 => {
                // MOV M,C - Move to mem pointed to by reg pair HL from reg C value
                self.mov_r("C");
            },
            0x72 => {
                // MOV M,D - Move to mem pointed to by reg pair HL from reg D value
                self.mov_r("D");
            },
            0x73 => {
                // MOV M,E - Move to mem pointed to by reg pair HL from reg E value
                self.mov_r("E");
            },
            0x74 => {
                // MOV M,H - Move to mem pointed to by reg pair HL from reg H value
                self.mov_r("H");
            },
            0x75 => {
                // MOV M,L - Move to mem pointed to by reg pair HL from reg L value
                self.mov_r("L");
            },
            0x76 => {
                // HLT - Halt execution
                self.halted = true;
                self.advance_pc(1);
            },
            0x77 => {
                // MOV M,A - Move to mem pointed to by reg pair HL from reg A value
                self.mov_r("A");
            },
            0x78 => {
                // MOV A,B - Move to reg A value from reg B
                self.mov("A", "B");
            },
            0x79 => {
                // MOV A,C - Move to reg A value from reg C
                self.mov("A", "C");
            },
            0x7a => {
                // MOV A,D - Move to reg A value from reg D
                self.mov("A", "D");
            },
            0x7b => {
                // MOV A,E - Move to reg A value from reg E
                self.mov("A", "E");
            },
            0x7c => {
                // MOV A,H - Move to reg A value from reg H
                self.mov("A", "H");
            },
            0x7d => {
                // MOV A,L - Move to reg A value from reg L
                self.mov("A", "L");
            },
            0x7e => {
                // MOV A,M - Move to reg A value from mem pointed to by reg pair HL
                self.mov_m("A");
            },
            0x7f => {
                // MOV A,A - Move reg A to reg A
                self.nop();
            },
    
            // 0x8x
            0x80 => {
                // ADD B - Add reg B to reg A
                self.add(self.registers.get_reg("B"));
            },
            0x81 => {
                // ADD C - Add reg C to reg A
                self.add(self.registers.get_reg("C"));
            },
            0x82 => {
                // ADD D - Add reg D to reg A
                self.add(self.registers.get_reg("D"));
            },
            0x83 => {
                // ADD E - Add reg E to reg A
                self.add(self.registers.get_reg("E"));
            },
            0x84 => {
                // ADD H - Add reg H to reg A
                self.add(self.registers.get_reg("H"));
            },
            0x85 => {
                // ADD L - Add reg L to reg A
                self.add(self.registers.get_reg("L"));
            },
            0x86 => {
                // ADD M - Add byte from mem pointed to by reg pair HL to reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.add(self.mem[addr]);
            },
            0x87 => {
                // ADD A - Add reg A to reg A
                self.add(self.registers.get_reg("A"));
            },
            0x88 => {
                // ADC B - Add reg B to reg A with carry
                self.adc(self.registers.get_reg("B"));
            },
            0x89 => {
                // ADC C - Add reg C to reg A with carry
                self.adc(self.registers.get_reg("C"));
            },
            0x8a => {
                // ADC D - Add reg D to reg A with carry
                self.adc(self.registers.get_reg("D"));
            },
            0x8b => {
                // ADC E - Add reg E to reg A with carry
                self.adc(self.registers.get_reg("E"));
            },
            0x8c => {
                // ADC H - Add reg H to reg A with carry
                self.adc(self.registers.get_reg("H"));
            },
            0x8d => {
                // ADC L - Add reg L to reg A with carry
                self.adc(self.registers.get_reg("L"));
            },
            0x8e => {
                // ADC M - Add byte from mem pointed to by reg pair HL to reg A with carry
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.adc(self.mem[addr]);
            },
            0x8f => {
                // ADC A - Add reg A to reg A with carry
                self.adc(self.registers.get_reg("A"));
            },
    
            // 0x9x
            0x90 => {
                // SUB B - Subtract reg B from from reg A
                self.sub(self.registers.get_reg("B"));
            },
            0x91 => {
                // SUB C - Subtract reg C from from reg A
                self.sub(self.registers.get_reg("C"));
            },
            0x92 => {
                // SUB D - Subtract reg D from from reg A
                self.sub(self.registers.get_reg("D"));
            },
            0x93 => {
                // SUB E - Subtract reg E from from reg A
                self.sub(self.registers.get_reg("E"));
            },
            0x94 => {
                // SUB H - Subtract reg H from from reg A
                self.sub(self.registers.get_reg("H"));
            },
            0x95 => {
                // SUB L - Subtract reg L from from reg A
                self.sub(self.registers.get_reg("L"));
            },
            0x96 => {
                // SUB M - Subtract byte from mem pointed to by reg pair HL from reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.sub(self.mem[addr]);
            },
            0x97 => {
                // SUB A - Subtract reg A from reg A
                self.sub(self.registers.get_reg("A"));
            },
            0x98 => {
                // SBB B - Subtract reg B from reg A with borrow
                self.sbb(self.registers.get_reg("B"));
            },
            0x99 => {
                // SBB C - Subtract reg C from reg A with borrow
                self.sbb(self.registers.get_reg("C"));
            },
            0x9a => {
                // SBB D - Subtract reg D from reg A with borrow
                self.sbb(self.registers.get_reg("D"));
            },
            0x9b => {
                // SBB E - Subtract reg E from reg A with borrow
                self.sbb(self.registers.get_reg("E"));
            },
            0x9c => {
                // SBB H - Subtract reg H from reg A with borrow
                self.sbb(self.registers.get_reg("H"));
            },
            0x9d => {
                // SBB L - Subtract reg L from reg A with borrow
                self.sbb(self.registers.get_reg("L"));
            },
            0x9e => {
                // SBB M - Subtract byte from mem pointed to by reg pair HL from reg A with borrow 
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.sbb(self.mem[addr]);
            },
            0x9f => {
                // SBB A - Subtract reg A from reg A with borrow
                self.sbb(self.registers.get_reg("A"));
            },
    
            /*
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
        self.registers.set_reg("A", 0x4);
        //self.registers.set_reg("D", 0x2);
        //self.registers.set_reg_pair("HL", 0xF00F);
        //self.registers.f.carry = true;
        println!("FLAGS: {:#?}\n", self.registers.f);
        println!("A: {:08b}\n", self.registers.get_reg("A"));

        // Test code goes here

        println!("\nFLAGS: {:#?}\n", self.registers.f);
        println!("A: {:08b}\n", self.registers.get_reg("A"));
    }
}
