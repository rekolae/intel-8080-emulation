use std::path::PathBuf;
use std::fs::read;

use crate::errors::EmulatorError;


pub struct Intel8080 {
    registers: Registers,
    mem: Vec<u8>,

    // Flag for when HLT (halt) instruction is executed
    halted: bool,

    // Interrupt system state
    int: bool
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

    pub fn get_psw(&mut self) -> u16 {
        (self.get_reg("A") as u16) << 8 | self.f.get_flags() as u16
    }

    pub fn set_psw(&mut self, val: u16) {
        self.set_reg("A", (val >> 8) as u8);

        let flag_vals: u8 = val as u8;
        self.f.set_flags(flag_vals);
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

    pub fn set_flags(&mut self, val: u8) {
        self.sign =      (val >> 7) & 0x1 == 1;     // Bit 7
        self.zero =      (val >> 6) & 0x1 == 1;     // Bit 6
                                                    // Bit 5 not used
        self.aux_carry = (val >> 4) & 0x1 == 1;     // Bit 4
                                                    // Bit 3 not used
        self.parity =    (val >> 2) & 0x1 == 1;     // Bit 2
                                                    // Bit 1 not used
        self.carry =      val       & 0x1 == 1;     // Bit 0
    }

    pub fn get_flags(&mut self) -> u8 {
        (self.sign as u8)       << 7 |    // Bit 7
        (self.zero as u8)       << 6 |    // Bit 6
        (0x0 as u8)             << 5 |    // Bit 5 always 0
        (self.aux_carry as u8)  << 4 |    // Bit 4
        (0x0 as u8)             << 3 |    // Bit 3 always 0
        (self.parity as u8)     << 2 |    // Bit 2
        (0x1 as u8)             << 1 |    // Bit 1 always 1
        (self.carry as u8)                // Bit 0
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
            },
    
            // 2^16 = 64KB of memory
            mem: Vec::<u8>::with_capacity(0x10000),

            halted: false,
            int: false
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

    // Return 2 bytes from memory pointed to by either PC or SP
    fn get_word(&self, pc: bool) -> u16 {
        // Take into account that the 8080 is little endian, so the first byte is actually the lower part of the value
        if pc {
            (self.mem[self.registers.pc + 2] as u16) << 8 | self.mem[self.registers.pc + 1] as u16
        } else {
            (self.mem[self.registers.sp as usize + 1] as u16) << 8 | self.mem[self.registers.sp as usize] as u16
        }
    }

    // Return 2 bytes from memory pointed to by SP
    fn pop_stack(&mut self) -> u16 {
        let val: u16 = self.get_word(false);
        self.registers.sp = self.registers.sp.wrapping_add(2);
        val
    }

    // Store 2 bytes into memory pointed to by SP
    fn push_stack(&mut self, val: u16) {
        self.registers.sp = self.registers.sp.wrapping_sub(2);
        self.mem[(self.registers.sp + 1) as usize] = (val >> 8) as u8;
        self.mem[self.registers.sp as usize] = val as u8;
    }

    // No operation
    fn nop(&mut self) {
        self.advance_pc(1);
    }

    // LXI reg pair - Load to reg pair the immediate value from addr
    fn lxi(&mut self, reg_pair: &str) {
        let val: u16 = self.get_word(true);
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
        self.registers.f.carry = reg_a as u16 + val as u16 > 0xff;

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
        self.registers.f.carry = reg_a as u16 + val as u16 + carry as u16 > 0xff;

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

    // ANA val - Logical AND value with accumulator
    fn ana(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let result: u8 = reg_a & val;

        // Carry is always set to zero
        self.registers.f.carry = false;
        self.registers.f.set_artihmetic_flags(result);
        self.registers.set_reg("A", result);

        /*
        From the "8080/8085 Assembly Language Programming Manual":

            There is some difference in the handling of the auxiliary carry flag by the logical AND instructions in the
            8080 processor and the 8085 processor. The 8085 logical AND instructions always set the auxiliary flag ON.
            The 8080 logical AND instructions set the flag to reflect the logical OR of bit 3 of the values involved in
            the AND operation.
        */
        self.registers.f.aux_carry = ((reg_a | val) & 0x08) == 1;

        self.advance_pc(1);
    }

    // XRA val - Logical XOR value with accumulator
    fn xra(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let result: u8 = reg_a ^ val;

        // Carry and aux carry are always set to zero
        self.registers.f.carry = false;
        self.registers.f.aux_carry = false;
        self.registers.f.set_artihmetic_flags(result);
        self.registers.set_reg("A", result);

        self.advance_pc(1);
    }

    // ORA val - Logical OR value with accumulator
    fn ora(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");
        let result: u8 = reg_a | val;

        // Carry and aux carry are always set to zero
        self.registers.f.carry = false;
        self.registers.f.aux_carry = false;
        self.registers.f.set_artihmetic_flags(result);
        self.registers.set_reg("A", result);

        self.advance_pc(1);
    }

    // CMP val - Compare value with accumulator
    fn cmp(&mut self, val: u8) {
        let reg_a: u8 = self.registers.get_reg("A");

        /*
        From the "8080/8085 Assembly Language Programming Manual":
            Comparisons are performed by subtracting the specified byte from the contents of the accumulator, which is
            why the zero and carry flags indicate the result.

        So we can use the the SUB instruction, but set the reg A value back to it's original value
        */
        self.sub(val);

        self.registers.set_reg("A", reg_a);

        self.advance_pc(1);
    }

    // RET IF condition - Return from subroutine by popping stack if condition is true
    fn ret(&mut self, condition: bool) {
        if condition {
            self.registers.pc = self.pop_stack() as usize;
        } else {
            self.advance_pc(1);
        }
    }

    // POP reg pair - Pop addr from stack and copy word from memory to reg pair
    fn pop(&mut self, reg_pair: &str) {
        let val: u16 = self.pop_stack();

        // Handle PSW (Program Status Word i.e. reg A + Flag reg) separately
        if reg_pair == "PSW" {
            self.registers.set_psw(val);
        } else {
            self.registers.set_reg_pair(reg_pair, val);
        }

        self.advance_pc(1);
    }

    // PUSH reg pair - Push reg pair to memory pointed to by SP
    fn push(&mut self, reg_pair: &str) {

        let mut val: u16 = 0;

        // Handle PSW (Program Status Word i.e. reg A + Flag reg) separately
        if reg_pair == "PSW" {
            val = self.registers.get_psw();
        } else {
            val = self.registers.get_reg_pair(reg_pair);
        }

        self.push_stack(val);
        self.advance_pc(1);
    }

    // JMP IF condition - Jump to address specified in the next two bytes
    fn jmp(&mut self, condition: bool) {
        if condition {
            self.registers.pc = self.get_word(true) as usize;
        } else {
            self.advance_pc(3);
        }
    }

    // CALL IF condition - Jump to address specified in the next two bytes
    fn call(&mut self, condition: bool) {
        if condition {
            self.push_stack(self.registers.pc as u16);
            self.registers.pc = self.get_word(true).into();
        } else {
            self.advance_pc(3);
        }
    }

    // RST num - Restart from a predefined address based on restart num
    fn rst(&mut self, val: u8) {
        self.push_stack(self.registers.pc as u16);
        self.registers.pc = self.mem[(0x08 * val) as usize].into();
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

                let addr: u16 = self.get_word(true);

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
                let addr: u16 = self.get_word(true);

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
                let val: u16 = self.get_word(true);
                self.registers.sp = val;

                self.advance_pc(3);
            },
            0x32 => {
                // STA - Store accumulator direct
                let addr: u16 = self.get_word(true);
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
                let addr: u16 = self.get_word(true);
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
    
            // 0xax
            0xa0 => {
                // ANA B - Logical AND reg B with reg A
                self.ana(self.registers.get_reg("B"));
            },
            0xa1 => {
                // ANA C - Logical AND reg C with reg A
                self.ana(self.registers.get_reg("C"));
            },
            0xa2 => {
                // ANA D - Logical AND reg D with reg A
                self.ana(self.registers.get_reg("D"));
            },
            0xa3 => {
                // ANA E - Logical AND reg E with reg A
                self.ana(self.registers.get_reg("E"));
            },
            0xa4 => {
                // ANA H - Logical AND reg H with reg A
                self.ana(self.registers.get_reg("F"));
            },
            0xa5 => {
                // ANA L - Logical AND reg L with reg A
                self.ana(self.registers.get_reg("L"));
            },
            0xa6 => {
                // ANA M - Logical AND byte from mem pointed to by reg pair HL with reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.ana(self.mem[addr]);
            },
            0xa7 => {
                // ANA A - Logical AND reg A with reg A
                self.ana(self.registers.get_reg("A"));
            },
            0xa8 => {
                // XRA B - Logical XOR reg B with reg A
                self.xra(self.registers.get_reg("B"));
            },
            0xa9 => {
                // XRA C - Logical XOR reg C with reg A
                self.xra(self.registers.get_reg("C"));
            },
            0xaa => {
                // XRA D - Logical XOR reg D with reg A
                self.xra(self.registers.get_reg("D"));
            },
            0xab => {
                // XRA E - Logical XOR reg E with reg A
                self.xra(self.registers.get_reg("E"));
            },
            0xac => {
                // XRA H - Logical XOR reg H with reg A
                self.xra(self.registers.get_reg("H"));
            },
            0xad => {
                // XRA L - Logical XOR reg L with reg A
                self.xra(self.registers.get_reg("L"));
            },
            0xae => {
                // XRA M - Logical XOR byte from mem pointed to by reg pair HL with reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.xra(self.mem[addr]);
            },
            0xaf => {
                // XRA A - Logical XOR reg A with reg A
                self.xra(self.registers.get_reg("A"));
            },
    
            // 0xbx
            0xb0 => {
                // ORA B - Logical OR reg  with reg A
                self.ora(self.registers.get_reg("B"))
            },
            0xb1 => {
                // ORA C - Logical OR reg C with reg A
                self.ora(self.registers.get_reg("C"))
            },
            0xb2 => {
                // ORA D - Logical OR reg D with reg A
                self.ora(self.registers.get_reg("D"))
            },
            0xb3 => {
                // ORA E - Logical OR reg E with reg A
                self.ora(self.registers.get_reg("E"))
            },
            0xb4 => {
                // ORA H - Logical OR reg H with reg A
                self.ora(self.registers.get_reg("H"))
            },
            0xb5 => {
                // ORA L - Logical OR reg L with reg A
                self.ora(self.registers.get_reg("L"))
            },
            0xb6 => {
                // ORA M - Logical OR byte from mem pointed to by reg pair HL with reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.ora(self.mem[addr]);
            },
            0xb7 => {
                // ORA A - Logical OR reg A with reg A
                self.ora(self.registers.get_reg("A"))
            },
            0xb8 => {
                // CMP B - Compare reg B with reg A
                self.cmp(self.registers.get_reg("B"))
            },
            0xb9 => {
                // CMP C - Compare reg C with reg A
                self.cmp(self.registers.get_reg("C"))
            },
            0xba => {
                // CMP D - Compare reg D with reg A
                self.cmp(self.registers.get_reg("D"))
            },
            0xbb => {
                // CMP E - Compare reg E with reg A
                self.cmp(self.registers.get_reg("E"))
            },
            0xbc => {
                // CMP H - Compare reg H with reg A
                self.cmp(self.registers.get_reg("H"))
            },
            0xbd => {
                // CMP L - Compare reg L with reg A
                self.cmp(self.registers.get_reg("L"))
            },
            0xbe => {
                // CMP M - Compare byte from mem pointed to by reg pair HL with reg A
                let addr: usize = self.registers.get_reg_pair("HL").into();
                self.cmp(self.mem[addr]);
            },
            0xbf => {
                // CMP A - Compare reg A with reg A
                self.cmp(self.registers.get_reg("A"))
            },
    
            // 0xcx
            0xc0 => {
                // RNZ - Return if zero flag not set
                self.ret(!self.registers.f.zero);
            },
            0xc1 => {
                // POP B - Pop addr from stack and copy byte from memory to reg pair BC
                self.pop("BC");
            },
            0xc2 => {
                // JNZ - Jump if zero flag not set
                self.jmp(!self.registers.f.zero);
            },
            0xc3 => {
                // JMP - Jump uncoditionally
                self.jmp(true);
            },
            0xc4 => {
                // CNZ - Call if zero flag not set
                self.call(!self.registers.f.zero);
            },
            0xc5 => {
                // PUSH B - Push reg pair BC to memory pointed to by SP
                self.push("BC");
            },
            0xc6 => {
                // ADI - Add immediate value to accumulator
                self.add(self.mem[self.registers.pc + 1]);
                
                // Advance by one because the ADD instructions already advances by one
                self.advance_pc(1);
            },
            0xc7 => {
                // RST 0 - Restart from addr
                self.rst(0);
            },
            0xc8 => {
                // RZ - Return if zero flag is set
                self.ret(self.registers.f.zero);
            },
            0xc9 => {
                // RET - Return uncoditionally
                self.ret(true);
            },
            0xca => {
                // JZ - Jump if zero flag is set
                self.jmp(self.registers.f.zero);
            },
            0xcb => {
                // JMP* - Jump uncoditionally (alternate)
                self.jmp(true);
            },
            0xcc => {
                // CZ - Call if zero flag is set
                self.call(self.registers.f.zero);
            },
            0xcd => {
                // CALL - Call uncoditionally
                self.call(true);
            },
            0xce => {
                // ACI - Add immediate value to accumulator with carry
                self.adc(self.mem[self.registers.pc + 1]);
                
                // Advance by one because the ADC instructions already advances by one
                self.advance_pc(1);
            },
            0xcf => {
                // RST 1 - Restart from addr
                self.rst(1);
            },
    
            // 0xdx
            0xd0 => {
                // RNC - Return if carry flag not set
                self.ret(!self.registers.f.carry);
            },
            0xd1 => {
                // POP D - Pop addr from stack and copy byte from memory to reg pair DE
                self.pop("DE");
            },
            0xd2 => {
                // JNC - Jump if carry flag not set
                self.jmp(!self.registers.f.carry);
            },
            0xd3 => {
                // OUT - Output accumulator to port specified in the next byte

                /*
                    ## Will be implemented later, when all other instructions are emulated ##
                */

                self.advance_pc(2);
            },
            0xd4 => {
                // CNC - Call if carry flag not set
                self.call(!self.registers.f.carry);
            },
            0xd5 => {
                // PUSH D - Push reg pair DE to memory pointed to by SP
                self.push("DE");
            },
            0xd6 => {
                // SUI - Subtract immediate value from accumulator
                self.sub(self.mem[self.registers.pc + 1]);
                
                // Advance by one because the SUB instructions already advances by one
                self.advance_pc(1);
            },
            0xd7 => {
                // RST 2 - Restart from addr
                self.rst(2);
            },
            0xd8 => {
                // RC - Return if carry flag is set
                self.ret(self.registers.f.carry);
            },
            0xd9 => {
                // RET* - Return uncoditionally (alternate)
                self.ret(true);
            },
            0xda => {
                // JC - Jump if carry flag is set
                self.jmp(self.registers.f.carry);
            },
            0xdb => {
                // IN - Write byte to accumulator from port specified in the next byte

                /*
                    ## Will be implemented later, when all other instructions are emulated ##
                */

                self.advance_pc(2);
            },
            0xdc => {
                // CC - Call if carry flag is set
                self.call(self.registers.f.carry);
            },
            0xdd => {
                // CALL* - Call uncoditionally (alternate)
                self.call(true);
            },
            0xde => {
                // SBI - Subtract immediate value from accumulator with carry
                self.sbb(self.mem[self.registers.pc + 1]);
                
                // Advance by one because the SBI instructions already advances by one
                self.advance_pc(1);
            },
            0xdf => {
                // RST 3 - Restart from addr
                self.rst(3);
            },
    
            // 0xex
            0xe0 => {
                // RPO - Return if parity flag not set (odd)
                self.ret(!self.registers.f.parity);
            },
            0xe1 => {
                // POP H - Pop addr from stack and copy byte from memory to reg pair HL
                self.pop("HL");
            },
            0xe2 => {
                // JPO - Jump if parity flag not set (odd)
                self.jmp(!self.registers.f.parity);
            },
            0xe3 => {
                // XTHL - Exhange reg pair HL value with word in mem pointed to by SP
                let mem_val: u16 = self.get_word(false);
                let hl: u16 = self.registers.get_reg_pair("HL");

                self.registers.set_reg_pair("HL", mem_val);
                self.mem[self.registers.sp as usize] = hl as u8;
                self.mem[(self.registers.sp + 1) as usize] = (hl >> 8) as u8;

                self.advance_pc(1);
            },
            0xe4 => {
                // CPO - Call if parity flag not set (odd)
                self.call(!self.registers.f.parity);
            },
            0xe5 => {
                // PUSH H - Push reg pair HL to memory pointed to by SP
                self.push("HL");
            },
            0xe6 => {
                // ANI - AND accumulator with immediate value
                self.ana(self.mem[self.registers.pc + 1]);

                // Advance by one because the ANA instructions already advances by one
                self.advance_pc(1);
            },
            0xe7 => {
                // RST 4 - Restart from addr
                self.rst(4);
            },
            0xe8 => {
                // RPE - Return if parity flag set (even)
                self.ret(self.registers.f.parity);
            },
            0xe9 => {
                // PCHL - Move reg pair HL to PC
                self.registers.pc = self.registers.get_reg_pair("HL").into();
                self.advance_pc(1);
            },
            0xea => {
                // JPE - Jump if parity flag set (even)
                self.jmp(self.registers.f.parity);
            },
            0xeb => {
                // XCHG - Exchange reg pair HL with reg pair DE
                let hl: u16 = self.registers.get_reg_pair("HL");
                let de: u16 = self.registers.get_reg_pair("DE");

                self.registers.set_reg_pair("HL", de);
                self.registers.set_reg_pair("DE", hl);

                self.advance_pc(1);
            },
            0xec => {
                // CPE - Call if parity flag set (even)
                self.call(self.registers.f.parity);
            },
            0xed => {
                // CALL* - Call uncoditionally (alternate)
                self.call(true);
            },
            0xee => {
                // XRI - XOR accumulator with immediate value
                self.xra(self.mem[self.registers.pc + 1]);

                // Advance by one because the XRA instructions already advances by one
                self.advance_pc(1);
            },
            0xef => {
                // RST 5 - Restart from addr
                self.rst(5);
            },
    
            // 0xfx
            0xf0 => {
                // RP - Return if sign flag not set (positive)
                self.ret(!self.registers.f.sign);
            },
            0xf1 => {
                // POP PSW - Pop addr from stack and copy byte from memory to reg A and Flags
                self.pop("PSW");
            },
            0xf2 => {
                // JP - Jump if sign flag not set (positive)
                self.jmp(!self.registers.f.sign);
            },
            0xf3 => {
                // DI - Disable interrupts
                self.int = false;
                self.advance_pc(1);
            },
            0xf4 => {
                // CP - Call if sign flag not set (positive)
                self.call(!self.registers.f.sign);
            },
            0xf5 => {
                // PUSH PSW - Push reg pair HL to memory pointed to by SP
                self.push("PSW");
            },
            0xf6 => {
                // ORI - OR accumulator with immediate value
                self.ora(self.mem[self.registers.pc + 1]);

                // Advance by one because the ORA instructions already advances by one
                self.advance_pc(1);

            },
            0xf7 => {
                // RST 6 - Restart from addr
                self.rst(6);
            },
            0xf8 => {
                // RM - Return if sign flag set (negative)
                self.ret(self.registers.f.sign);
            },
            0xf9 => {
                // SPHL - Move reg pair HL to SP
                self.registers.sp = self.registers.get_reg_pair("HL");
                self.advance_pc(1);
            },
            0xfa => {
                // JM - Jump if sign flag set (negative)
                self.jmp(self.registers.f.sign);
            },
            0xfb => {
                // EI - Enable interrupts
                self.int = true;
                self.advance_pc(1);
            },
            0xfc => {
                // CM - Call if sign flag set (negative)
                self.call(self.registers.f.sign);
            },
            0xfd => {
                // CALL* - Call uncoditionally (alternate)
                self.call(true);
            },
            0xfe => {
                // CPI - Compare immediate value with reg A
                self.cmp(self.mem[self.registers.pc + 1]);

                // Advance by one because the CMP instructions already advances by one
                self.advance_pc(1);
            },
            0xff => {
                // RST 7 - Restart from addr
                self.rst(7);
            },
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
        //println!("HL: {:04X}\n", self.registers.get_reg_pair("HL"));

        // Test code goes here

        println!("\nFLAGS: {:#?}\n", self.registers.f);
        println!("A: {:08b}\n", self.registers.get_reg("A"));
        //println!("HL: {:04X}\n", self.registers.get_reg_pair("HL"));
    }
}
