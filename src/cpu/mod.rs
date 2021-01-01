mod instruction;

use std::{cell::RefCell, rc::Rc};

use crate::bus::MainBus;
use instruction::Instruction;

pub struct R3000 {
    gen_registers: [u32; 32],
    pc: u32,
    hi: u32,
    lo: u32,
    main_bus: Rc<RefCell<MainBus>>,
    delay_slot: u32,
}

impl R3000 {
    pub fn new(bus: Rc<RefCell<MainBus>>) -> R3000 {
        R3000 {
            gen_registers: [0; 32],
            pc: 0,
            hi: 0,
            lo: 0,
            main_bus: bus,
            delay_slot: 0,
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
    }

    /// Runs the next instruction based on the PC location. Only useful for testing because it is not at all accurate to
    /// how the cpu actually works.
    pub fn step_instruction(&mut self) {
        let instruction = (*self.main_bus).borrow().read_word(self.pc);
        self.execute_instruction(instruction);

        //Execute branch delay operation
        if self.delay_slot != 0 {
            let delay_instruction = (*self.main_bus).borrow().read_word(self.delay_slot);
            self.execute_instruction(delay_instruction);
            self.delay_slot = 0;
        }
        self.pc += 4;
    }

    pub fn execute_instruction(&mut self, instruction: u32) {
        if self.pc % 4 != 0 {
            panic!("Address is not aligned!");
        }
        match instruction.opcode() {
            0x0 => {
                //SPECIAL INSTRUCTIONS
                match instruction.funct() {
                    0x0 => {
                        //SLL
                        self.write_gen_register(
                            instruction.rd(),
                            self.read_gen_register(instruction.rt())
                                << self.read_gen_register(instruction.shamt()),
                        );
                    }

                    0x25 => {
                        //OR
                        self.write_gen_register(
                            instruction.rd(),
                            self.read_gen_register(instruction.rs())
                                | self.read_gen_register(instruction.rt()),
                        );
                    }
                    _ => panic!(
                        "Unknown SPECIAL instruction. FUNCT is {0:#X} ({0:#08b}, {0:#X})",
                        instruction.funct()
                    ),
                }
            }

            0x2 => {
                //JUMP
                self.delay_slot = self.pc + 4;
                self.pc = ((instruction.address() << 2) | self.delay_slot) - 4;
            }

            0x5 => {
                //BNE
                if self.read_gen_register(instruction.rs()) != self.read_gen_register(instruction.rt()) {
                    self.delay_slot = self.pc + 4;
                    self.pc = (((instruction.immediate() as u32) << 2) + self.delay_slot ) - 4;
                }
            }

            0x8 => {
                //ADDI
                self.write_gen_register(instruction.rt(), self.read_gen_register(instruction.rs()) + instruction.immediate() as u32);
            }

            0x10 => {
                //COPz cofun for COP0
                println!("COP0 opcode called. Ignoring because cop0 is not emulated");
            }

            0x2B => {
                //SW
                let addr =
                    self.read_gen_register(instruction.rs()) + instruction.immediate() as u32;
                self.main_bus
                    .borrow_mut()
                    .write_word(addr, self.read_gen_register(instruction.rt()));
            }

            0xD => {
                //ORI
                self.write_gen_register(
                    instruction.rt(),
                    self.read_gen_register(instruction.rs()) | instruction.immediate() as u32,
                )
            }
            0xF => {
                //LUI
                self.write_gen_register(instruction.rt(), (instruction.immediate() as u32) << 16);
            }

            0x9 => {
                //ADDIU
                self.write_gen_register(
                    instruction.rt(),
                    self.read_gen_register(instruction.rs()) + instruction.immediate() as u32,
                );
            }
            _ => panic!(
                "Unknown opcode {0} ({0:#08b}, {0:#X})",
                instruction.opcode()
            ),
        }
    }

    /// Returns the value stored within the given register. Will panic if register_number > 31
    fn read_gen_register(&self, register_number: u8) -> u32 {
        self.gen_registers[register_number as usize]
    }

    /// Sets register to given value. Prevents setting R0, which should always be zero. Will panic if register_number > 31
    fn write_gen_register(&mut self, register_number: u8, value: u32) {
        match register_number {
            0 => (), //Prevent writing to the zero register
            _ => self.gen_registers[register_number as usize] = value,
        }
    }
}
