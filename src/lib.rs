use bios::Bios;
use bus::MainBus;
use cpu::R3000;
use timer::TimerState;
use std::panic;

use crate::cpu::InterruptSource;
use crate::dma::execute_dma_cycle;
use crate::gpu::Gpu;
use crate::memory::Memory;
use crate::cdrom::disc::Disc;

mod bios;
mod bus;
pub mod cpu;
mod dma;
mod gpu;
mod memory;
mod spu;
pub mod cdrom;
mod timer;
mod controller;

pub struct PSXEmu {
    pub r3000: R3000,
    timers: TimerState,
    cycle_count: u32,
    halt_requested: bool,
    sw_breakpoints: Vec<u32>
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

        PSXEmu {
            r3000: r3000,
            timers: TimerState::new(),
            cycle_count: 0,
            halt_requested: false,
            sw_breakpoints: Vec::new(),
        }
    }

    /// Resets system to startup condition
    pub fn reset(&mut self) {
        self.r3000.reset();
        self.r3000.main_bus.gpu.reset();
    }

    /// Runs the next cpu instruction.
    /// This function is only here for testing and is not at all accurate to how the cpu actually works
    pub fn step_instruction(&mut self) {
        //Twoish cpu per gpu clock
        for _ in 0..2 {
            if self.sw_breakpoints.contains(&self.r3000.pc) {
                self.halt_requested = true;
                return;
            }
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

    pub fn load_executable(&mut self, start_addr: u32, entrypoint: u32, _sp: u32, data: &Vec<u8>) {
        for (index, val) in data.iter().enumerate() {
            self.r3000
                .main_bus
                .write_byte((index + start_addr as usize) as u32, val.clone());
        }
        self.r3000.load_exe = true;
        //self.r3000.pc = entrypoint;
        // self.r3000.gen_registers[29] = sp;
        // self.r3000.gen_registers[30] = sp;
    }

    pub fn load_disc(&mut self, disc: Disc) {
        self.r3000.main_bus.cd_drive.load_disc(disc);
    }

    pub fn loaded_disc(&self) -> &Option<Disc> {
        self.r3000.main_bus.cd_drive.disc()
    }

    pub fn remove_disc(&mut self) {
        self.r3000.main_bus.cd_drive.remove_disc();
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

    pub fn read_gen_reg(&self, reg_num: usize) -> u32 {
        self.r3000.gen_registers[reg_num]
    }

    pub fn set_gen_reg(&mut self, reg_num: usize, value: u32) {
        self.r3000.gen_registers[reg_num] = value;
    }

    pub fn halt_requested(&self) -> bool {
        self.halt_requested
    }

    pub fn clear_halt(&mut self) {
        self.halt_requested = false;
    }

    pub fn add_sw_breakpoint(&mut self, addr: u32) {
        self.sw_breakpoints.push(addr);
    }

    pub fn remove_sw_breakpoint(&mut self, addr: u32) {
        self.sw_breakpoints.retain(|&x| x != addr);
    }
}
