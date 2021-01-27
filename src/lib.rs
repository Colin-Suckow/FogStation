use std::{cell::RefCell, rc::Rc};

use bios::Bios;
use bus::MainBus;
use cpu::R3000;
use timer::TimerState;

use crate::gpu::Gpu;
use crate::memory::Memory;
use crate::dma::{DMAState, execute_dma_cycle};
use crate::cpu::InterruptSource;

mod bios;
mod bus;
pub mod cpu;
mod gpu;
mod memory;
mod dma;
mod timer;
mod spu;

pub struct PSXEmu {
    pub r3000: R3000,
    timers: TimerState,
    cycle_count: u32,
    dma: DMAState,
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
        PSXEmu { r3000: r3000, timers: TimerState::new(), cycle_count: 0, dma: DMAState::new() }
    }

    /// Resets system to startup condition
    pub fn reset(&mut self) {
        self.r3000.reset();
    }

    /// Runs the next cpu instruction.
    /// This function is only here for testing and is not at all accurate to how the cpu actually works
    pub fn step_instruction(&mut self) {
        //Threeish cpu per gpu clock
        for _ in 0..3 {
            self.r3000.step_instruction(&mut self.timers);
            execute_dma_cycle(&mut self.r3000);
            self.cycle_count += 1;
            self.timers.update_sys_clock(&mut self.r3000);
            if self.cycle_count % 8 == 0 {
                self.timers.update_sys_div_8(&mut self.r3000);
            }
        }

        self.r3000.main_bus.gpu.execute_cycle();
        self.timers.update_dot_clock(&mut self.r3000);
        if self.r3000.main_bus.gpu.consume_hblank() {
            self.timers.update_h_blank(&mut self.r3000);
        }

    }

    ///Runs the emulator till one frame has been generated
    pub fn run_frame(&mut self) {
        while !self.r3000.main_bus.gpu.end_of_frame() {
            self.step_instruction();
        }
        //Step the gpu once more to get it off this frame
        self.r3000.main_bus.gpu.execute_cycle();
    }

    pub fn load_executable(&mut self, start_addr: u32, data: &Vec<u8>) {
        for (index, val) in data.iter().enumerate() {
            self.r3000.main_bus.write_byte((index + start_addr as usize) as u32, val.clone());
        }
        self.r3000.pc = start_addr;
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        self.r3000.main_bus.gpu.get_vram()
    }

    pub fn get_bios(&self) -> &Vec<u8> {
        self.r3000.main_bus.bios.get_data()
    }

    pub fn manually_fire_interrupt(&mut self, source: InterruptSource) {
        self.r3000.fire_external_interrupt(source);
    }
}
