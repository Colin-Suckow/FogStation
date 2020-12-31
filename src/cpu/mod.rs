mod instruction;

use std::rc::Rc;

use crate::bus::MainBus;
use instruction::Instruction;

pub struct R3000 {
    gen_registers: [u32; 32],
    pc: u32,
    hi: u32,
    lo: u32,
    main_bus: Rc<MainBus>,
}

impl R3000 {
    pub fn new(bus: Rc<MainBus>) -> R3000 {
        R3000 {
            gen_registers: [0; 32],
            pc: 0,
            hi: 0,
            lo: 0,
            main_bus: bus,
        }
    }

    pub fn reset(&mut self) {
        //Clear registers
        for reg in self.gen_registers.iter_mut() {
            *reg = 0;
        }
        self.hi = 0;
        self.lo = 0;
        self.pc = 0xBFC00000; // Points to the bios entry point
    }

    pub fn execute_instruction(&mut self, instruction: u32) {
        match instruction.opcode() {
            _ => panic!("Unknown opcode {:#X}", instruction.opcode())
        }
    }

    fn read_gen_register(&self, register_number: u8) -> u32 {
        self.gen_registers[register_number as usize]
    }

    fn write_gen_register(&mut self, register_number: u8, value: u32) {
        match register_number {
            0 => (), //Prevent writing to the zero register
            _ => self.gen_registers[register_number as usize] = value
        }
    }
}