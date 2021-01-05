mod instruction;
mod cop0;

use std::{cell::RefCell, rc::Rc};
use crate::bus::MainBus;
use instruction::{Instruction,NumberHelpers};
use cop0::Cop0;
use bit_field::BitField;


enum Exception {
    IBE = 6, //Bus error
    DBE = 7, //Bus error Data
    AdEL = 4, //Address Error Load
    AdES = 5, //Address Error Store
    Ovf = 12, //Overflow
    Sys = 8, //System Call
    Bp = 9, //Breakpoint
    RI = 10, //Reserved Instruction
    CpU = 11, //Co-processor Unusable
    TLBL = 2, //TLB Miss Load
    TLBS = 3, //TLB Miss Store
    Mod = 1, // TLB modified
    Int = 0, //Interrupt
}

pub struct R3000 {
    pub gen_registers: [u32; 32],
    pub pc: u32,
    old_pc: u32,
    hi: u32,
    lo: u32,
    main_bus: MainBus,
    delay_slot: u32,
    cop0: Cop0,
}

impl R3000 {
    pub fn new(bus: MainBus) -> R3000 {
        R3000 {
            gen_registers: [0; 32],
            pc: 0,
            old_pc: 0,
            hi: 0,
            lo: 0,
            main_bus: bus,
            delay_slot: 0,
            cop0: Cop0::new(),
        }
    }
    /// Resets cpu registers to zero and sets program counter to reset vector (0xBFC00000)
    pub fn reset(&mut self) {
        //Clear registers
        for reg in self.gen_registers.iter_mut() {
            *reg = 0;
        }
        self.hi = 0;
        self.lo = 0;
        self.pc = 0xBFC00000; // Points to the bios entry point
        self.cop0.write_reg(12, self.cop0.read_reg(12).set_bit(23, true).clone());
    }

    /// Runs the next instruction based on the PC location. Only useful for testing because it is not at all accurate to
    /// how the cpu actually works.
    pub fn step_instruction(&mut self) {
        let instruction = self.main_bus.read_word(self.pc);
        self.old_pc = self.pc;
        self.pc += 4;

        //println!("Executing {:#X} (FUNCT {:#X}) at {:#X} (FULL {:#X})", instruction.opcode(), instruction.funct(), self.old_pc, instruction);

        self.execute_instruction(instruction);

        //Execute branch delay operation
        if self.delay_slot != 0 {
            let delay_instruction = self.main_bus.read_word(self.delay_slot);
            //println!("DS executing {:#X} (FUNCT {:#X}) at {:#X}",delay_instruction.opcode(), delay_instruction.funct(), self.old_pc + 4);
            self.execute_instruction(delay_instruction);
            self.delay_slot = 0;
        }
        
    }

    pub fn execute_instruction(&mut self, instruction: u32) {
        if self.pc % 4 != 0 || self.delay_slot % 4 != 0 {
            panic!("Address is not aligned!");
        }
        let pc = self.old_pc;
        match instruction.opcode() {
            0x0 => {
                //SPECIAL INSTRUCTIONS
                match instruction.funct() {
                    0x0 => {
                        //SLL
                        self.write_reg(
                            instruction.rd(),
                            self.read_reg(instruction.rt())
                                << instruction.shamt(),
                        );
                    }

                    0x2 => {
                        //SRL
                        self.write_reg(
                            instruction.rd(),
                            self.read_reg(instruction.rt())
                                >> instruction.shamt(),
                        );
                    }

                    0x3 => {
                        //SRA
                        self.write_reg(
                            instruction.rd(),
                            (self.read_reg(instruction.rt()) as i32
                                >> instruction.shamt()) as u32,
                        );
                    }

                    0x4 => {
                        //SLLV
                        self.write_reg(
                            instruction.rd(),
                            self.read_reg(instruction.rt())
                                << (self.read_reg(instruction.rs()) & 0x1F),
                        );
                    }

                    0x7 => {
                        //SRAV
                        self.write_reg(instruction.rd(), self.read_reg(instruction.rt()) / (2 ^ self.read_reg(instruction.rs())));
                    }

                    0x8 => {
                        //JR
                        self.delay_slot = self.pc;
                        self.pc = self.read_reg(instruction.rs());
                    }

                    0x9 => {
                        //JALR
                        self.delay_slot = self.pc;
                        self.pc = self.read_reg(instruction.rs());
                        self.write_reg(instruction.rd(), self.delay_slot + 4);
                    }

                    0xC => {
                        //SYSCALL
                        self.fire_exception(Exception::Sys);
                    }

                    0x10 => {
                        //MFHI
                        self.write_reg(instruction.rd(), self.hi);
                    }

                    0x11 => {
                        //MTHI
                        self.hi = self.read_reg(instruction.rs());
                    }

                    0x12 => {
                        //MFLO
                        self.write_reg(instruction.rd(), self.lo);
                    }

                    0x13 => {
                        //MTLO
                        self.lo = self.read_reg(instruction.rs());
                    }

                    0x1A => {
                        //DIV
                        let rs = self.read_reg(instruction.rs()) as i32;
                        let rt = self.read_reg(instruction.rt()) as i32;
                        self.hi = (rs / rt) as u32;
                        self.lo = (rs % rt) as u32;
                    }

                    0x1B => {
                        //DIVU
                        let rs = self.read_reg(instruction.rs());
                        let rt = self.read_reg(instruction.rt());
                        self.hi = rs / rt;
                        self.lo = rs % rt;
                    }

                    0x20 => {
                        //ADD
                        self.write_reg(instruction.rd(), match (self.read_reg(instruction.rs()) as i32).checked_add(self.read_reg(instruction.rt()) as i32) {
                            Some(val) => val as u32,
                            None => panic!("ADD overflowed")
                        })
                    }

                    0x2B => {
                        //SLTU
                        self.write_reg(instruction.rd(), (self.read_reg(instruction.rs()) < self.read_reg(instruction.rt())) as u32);
                    }

                    0x23 => {
                        //SUBU
                        self.write_reg(instruction.rd(), (self.read_reg(instruction.rs())).wrapping_sub(self.read_reg(instruction.rt())));
                    }

                    0x24 => {
                        //AND
                        self.write_reg(instruction.rd(), self.read_reg(instruction.rs()) & self.read_reg(instruction.rt()));
                    }

                    0x25 => {
                        //OR
                        self.write_reg(
                            instruction.rd(),
                            self.read_reg(instruction.rs())
                                | self.read_reg(instruction.rt()),
                        );
                    }

                    0x27 => {
                        //NOR
                        self.write_reg(instruction.rd(), !(self.read_reg(instruction.rt()) | self.read_reg(instruction.rs())));
                    }

                    0x21 => {
                        //ADDU
                        self.write_reg(instruction.rd(), (self.read_reg(instruction.rt())).wrapping_add(self.read_reg(instruction.rs())));
                    }

                    0x2A => {
                        //SLT
                        self.write_reg(instruction.rd(), ((self.read_reg(instruction.rs()) as i32) < (self.read_reg(instruction.rt())as i32)) as u32);
                    }

                    _ => panic!(
                        "Unknown SPECIAL instruction. FUNCT is {0} ({0:#08b}, {0:#X})",
                        instruction.funct()
                    ),
                }
            }

            0x1 => {
                //"PC-relative" test and branch instructions
                match instruction.rt() {
                    0x0 => {
                        //BLTZ
                        self.delay_slot = self.pc;
                        if (self.read_reg(instruction.rs()) as i32) < 0 {
                            self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                        }
                    }
                    0x1 => {
                        //BGEZ
                        self.delay_slot = self.pc;
                        if self.read_reg(instruction.rs()) as i32 > 0 {
                            self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                        }
                    }
                    _ => panic!("Unknown test and branch instruction {} ({0:#X})", instruction.rt())
                }
            }

            0x2 => {
                //J
                self.delay_slot = self.pc;
                self.pc = ((instruction.address() << 2) | (self.delay_slot & 0xF0000000));
                
            }

            0x3 => {
                //JAL
                self.delay_slot = self.pc;
                self.write_reg(31, self.pc + 4);
                self.pc = ((instruction.address() << 2) | (self.delay_slot & 0xF0000000));
            }

            0x4 => {
                //BEQ
                self.delay_slot = self.pc;
                if self.read_reg(instruction.rs()) == self.read_reg(instruction.rt()) {
                    self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                }
            }

            0x5 => {
                //BNE
                self.delay_slot = self.pc;
                if self.read_reg(instruction.rs()) != self.read_reg(instruction.rt()) {
                    self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                }
            }

            0x6 => {
                //BLEZ
                self.delay_slot = self.pc;
                if (self.read_reg(instruction.rs()) as i32) <= 0 {
                    self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                }
            }

            0x7 => {
                //BGTZ
                self.delay_slot = self.pc;
                if (self.read_reg(instruction.rs()) as i32) > 0 {
                    self.pc = ((instruction.immediate().sign_extended() << 2).wrapping_add(self.delay_slot) );
                }
            }

            0x8 => {
                //ADDI
                self.write_reg(instruction.rt(), match (self.read_reg(instruction.rs()) as i32).checked_add(instruction.immediate().sign_extended() as i32) {
                    Some(val) => val as u32,
                    None => panic!("ADDI overflowed")
                })

            }

            0x9 => {
                //ADDIU
                self.write_reg(
                    instruction.rt(),
                    self.read_reg(instruction.rs()).wrapping_add(instruction.immediate().sign_extended())
                );
            }

            0xA => {
                //SLTI
                self.write_reg(instruction.rt(), (((self.read_reg(instruction.rs())) as i32) < (instruction.immediate().sign_extended() as i32)) as u32);
            }

            0xB => {
                //SLTIU
                self.write_reg(instruction.rt(), (self.read_reg(instruction.rs()) < instruction.immediate().sign_extended()) as u32);
            }

            0xC => {
                //ANDI
                self.write_reg(instruction.rt(), instruction.immediate().zero_extended() & self.read_reg(instruction.rs()));
            }

            0xD => {
                //ORI
                self.write_reg(
                    instruction.rt(),
                    self.read_reg(instruction.rs()) | instruction.immediate().zero_extended(),
                )
            }
            0xF => {
                //LUI
                self.write_reg(instruction.rt(), (instruction.immediate() as u32) << 16);
            }

            0x10 => {
                match instruction.rs() {
                    0b00100 => {
                        //MTCz
                        self.cop0.write_reg(instruction.rd(), self.read_reg(instruction.rt()));
                    }
                    0x0 => {
                        //MFCz
                        self.write_reg(instruction.rt(), self.cop0.read_reg(instruction.rd()));
                    }
                    _ => ()
                }
            }

            0x20 => {
                //LB
                let addr = (instruction.immediate().sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
                let val = self.main_bus.read_byte(addr).sign_extended();
                self.write_reg(instruction.rt(), val);
            }

            0x21 => {
                //LH
                let addr = (instruction.immediate().sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
                let val = self.main_bus.read_half_word(addr).sign_extended();
                self.write_reg(instruction.rt(), val);
            }

            0x23 => {
                //LW
                let addr = (instruction.immediate().sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
                let val = self.main_bus.read_word(addr);
                self.write_reg(instruction.rt(), val);
            }

            0x24 => {
                //LBU
                let addr = (instruction.immediate().sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
                let val = self.main_bus.read_byte(addr).zero_extended();
                self.write_reg(instruction.rt(), val);
            }

            0x25 => {
                //LHU
                let addr = (instruction.immediate().sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
                let val = self.main_bus.read_half_word(addr).zero_extended();
                self.write_reg(instruction.rt(), val);
            }

            0x28 => {
                //SB
                let addr = instruction.immediate().sign_extended().wrapping_add(self.read_reg(instruction.rs()));
                let val = (self.read_reg(instruction.rt()) & 0xFF) as u8;
                self.write_bus_byte(addr, val);
            }

            0x29 => {
                //SH
                let addr = instruction.immediate().sign_extended().wrapping_add(self.read_reg(instruction.rs()));
                let val = (self.read_reg(instruction.rt()) & 0xFFFF) as u16;
                self.write_bus_half_word(addr, val);
            }

            0x2B => {
                //SW
                let addr =
                    self.read_reg(instruction.rs()).wrapping_add(instruction.immediate().sign_extended());
                self.write_bus_word(addr, self.read_reg(instruction.rt()));
            }
            _ => panic!(
                "Unknown opcode {0} ({0:#08b}, {0:#X})",
                instruction.opcode()
            ),
        }
    }

    fn fire_exception(&mut self, exception: Exception) {
        if self.delay_slot != 0 {
            panic!("Branch delay exception rollback is not implemented!");
        }
        self.cop0.set_cause_execode(exception);
        self.cop0.write_reg(14, self.pc);

        self.pc = if self.cop0.read_reg(12).get_bit(23) {
            0xBFC0_0180
        } else {
            0x8000_0080
        };
    }

    fn write_bus_word(&mut self, addr: u32, val: u32) {
        let sr = self.cop0.read_reg(12);
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        self.main_bus.write_word(addr, val);
    }

    fn write_bus_half_word(&mut self, addr: u32, val: u16) {
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        self.main_bus.write_half_word(addr, val);
    }

    fn write_bus_byte(&mut self, addr: u32, val: u8) {
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        self.main_bus.write_byte(addr, val);
    }

    /// Returns the value stored within the given register. Will panic if register_number > 31
    fn read_reg(&self, register_number: u8) -> u32 {
        self.gen_registers[register_number as usize]
    }

    /// Sets register to given value. Prevents setting R0, which should always be zero. Will panic if register_number > 31
    fn write_reg(&mut self, register_number: u8, value: u32) {
        match register_number {
            0 => (), //Prevent writing to the zero register
            _ => self.gen_registers[register_number as usize] = value,
        }
    }
}