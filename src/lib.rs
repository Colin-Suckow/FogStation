mod bios;
mod bus;
mod cpu;
mod dma;
mod gpu;
mod memory;

use std::{cell::RefCell, rc::Rc};

use crate::gpu::Gpu;
use crate::memory::Memory;
use bios::Bios;
use bus::MainBus;
use cpu::R3000;

pub struct PSXEmu {
    pub r3000: R3000,
}

impl PSXEmu {
    /// Creates a new instance of the emulator.
    /// WARNING: Call reset() before using, emulator is not initialized in a valid state.
    pub fn new(bios: Vec<u8>) -> PSXEmu {
        let bios = Bios::new(bios);
        let memory = Memory::new();
        let gpu = Gpu::new();
        let bus = MainBus::new(bios, memory, gpu);
        let r3000 = R3000::new(bus);
        PSXEmu { r3000: r3000 }
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

    ///Runs the emulator till one frame has been generated
    pub fn run_frame(&mut self) {
        for _ in 0..564480 {
            self.step_instruction();
        }

    }

    pub fn get_vram(&self) -> &Vec<u16> {
        self.r3000.main_bus.gpu.get_vram()
    }

    pub fn get_bios(&self) -> &Vec<u8> {
        self.r3000.main_bus.bios.get_data()
    }
}
