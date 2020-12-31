mod bios;
mod bus;
mod cpu;
mod memory;

use std::{cell::RefCell, rc::Rc};

use bios::Bios;
use bus::MainBus;
use cpu::R3000;
use crate::memory::Memory;

pub struct PSXEmu {
    main_bus: Rc<RefCell<MainBus>>,
    r3000: R3000,
}

impl PSXEmu {
    /// Creates a new instance of the emulator.
    /// WARNING: Call reset() before using, emulator is not initialized in a valid state.
    pub fn new(bios: Vec<u8>) -> PSXEmu {
        let bios = Bios::new(bios);
        let memory = Memory::new();
        let bus = Rc::new(RefCell::new(MainBus::new(bios, memory)));
        let r3000 = R3000::new(bus.clone());
        PSXEmu {
            main_bus: bus,
            r3000: r3000,
        }
    }
    
    /// Resets system to startup condition
    pub fn reset(&mut self) {
        self.r3000.reset();
    }

    /// Runs the next cpu instruction.
    /// This function is only here for testing and is not at all accurate to how the cpu actually works
    pub fn step_instruction(&mut self) {
        self.r3000.step_instruction();
    }
}
