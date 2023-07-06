use bios::Bios;
use bus::MainBus;
use controller::ButtonState;
use cpu::R3000;
use gpu::{DrawCall, Resolution};
use timer::TimerState;

use crate::cdrom::disc::Disc;
use crate::cpu::InterruptSource;
use crate::dma::execute_dma_cycle;
use crate::gpu::Gpu;
use crate::memory::Memory;
use crate::scheduler::{CpuCycles, Scheduler, ScheduleTarget};

mod bios;
mod bus;
pub mod cdrom;
pub mod controller;
pub mod cpu;
mod dma;
pub mod gpu;
mod mdec;
mod memory;
mod spu;
mod timer;
mod scheduler;

static mut LOGGING: bool = false;

pub struct PSXEmu {
    pub r3000: R3000,
    pub main_bus: MainBus,
    pub scheduler: Scheduler,
    cpu_cycles: u32,
    halt_requested: bool,
    sw_breakpoints: Vec<u32>,
    watchpoints: Vec<u32>,
    frame_count: u32,
    exit_requested: bool,
}

impl PSXEmu {
    /// Creates a new instance of the emulator.
    pub fn new(bios: Vec<u8>) -> PSXEmu {
        let bios = Bios::new(bios);
        let memory = Memory::new();
        let gpu = Gpu::new();
        let bus = MainBus::new(bios, memory, gpu);
        let r3000 = R3000::new();

        let mut emu = PSXEmu {
            r3000: r3000,
            main_bus: bus,
            scheduler: Scheduler::new(),
            cpu_cycles: 0,
            halt_requested: false,
            sw_breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            frame_count: 0,
            exit_requested: false,
        };
        emu.reset();

        // Register initial events
        emu.scheduler.schedule_event(ScheduleTarget::GpuHblank, CpuCycles(0).into());
        emu.scheduler.schedule_event(ScheduleTarget::GpuVblank, CpuCycles(413664).into());

        emu
    }

    /// Resets system to startup condition
    pub fn reset(&mut self) {
        self.r3000.reset();
        self.main_bus.gpu.reset();
    }

    pub fn step_cycle(&mut self) {
        if self.main_bus.exit_requested {
            self.exit_requested = true;
            return;
        }

        self.scheduler.run_cycle(&mut self.r3000, &mut self.main_bus);

        // DMA doesn't use any delays, so it is kind of outside of the scheduler right now
        // (plz ignore the fact that scheduler is an argument, that is for later use)
        execute_dma_cycle(&mut self.r3000, &mut self.main_bus, &mut self.scheduler);

        // Cpu run one instruction per 2 cycles, so only execute an instruction every other cycle
        if self.cpu_cycles % 2 == 0 && self.run_cpu_instruction() {
            // A branch delay slot was executed, so run an extra scheduler cycle
            self.scheduler.run_cycle(&mut self.r3000, &mut self.main_bus);
            self.cpu_cycles += 1
        }

        self.cpu_cycles += 1;
    }

    pub fn run_cpu_instruction(&mut self) -> bool {
        if self.sw_breakpoints.contains(&self.r3000.pc) {
            self.halt_requested = true;
            return false;
        }

        if self.watchpoints.contains(&self.r3000.last_touched_addr) {
            self.halt_requested = true;
            return false;
        }

        self.r3000.step_instruction(&mut self.main_bus, &mut self.scheduler)
    }

    ///Runs the emulator till one frame has been generated
    pub fn run_frame(&mut self) {
        while !self.frame_ready() {
            self.step_cycle();
        }
        self.frame_count += 1;
    }

    pub fn load_executable(&mut self, start_addr: u32, entrypoint: u32, _sp: u32, data: &Vec<u8>) {
        for (index, val) in data.iter().enumerate() {
            self
                .main_bus
                .write_byte((index + start_addr as usize) as u32, val.clone(), &mut self.scheduler);
        }
        self.r3000.load_exe = true;
        self.r3000.entrypoint = entrypoint;
        // self.gen_registers[29] = sp;
        // self.gen_registers[30] = sp;
    }

    pub fn load_disc(&mut self, disc: Disc) {
        self.main_bus.cd_drive.load_disc(disc);
    }

    pub fn loaded_disc(&self) -> &Option<Disc> {
        self.main_bus.cd_drive.disc()
    }

    pub fn remove_disc(&mut self) {
        self.main_bus.cd_drive.remove_disc();
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        self.main_bus.gpu.get_vram()
    }

    pub fn is_full_color_depth(&self) -> bool {
        self.main_bus.gpu.is_full_color_depth()
    }

    pub fn get_bios(&self) -> &Vec<u8> {
        self.main_bus.bios.get_data()
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
        println!("Adding breakpoint");
        self.sw_breakpoints.push(addr);
    }

    pub fn remove_sw_breakpoint(&mut self, addr: u32) {
        self.sw_breakpoints.retain(|&x| x != addr);
    }

    pub fn display_resolution(&self) -> Resolution {
        self.main_bus.gpu.resolution()
    }

    pub fn update_controller_state(&mut self, state: ButtonState) {
        self.main_bus.controllers.update_button_state(state);
    }

    pub fn frame_ready(&mut self) -> bool {
        self.main_bus.gpu.take_frame_ready()
    }

    pub fn set_gpu_logging(&mut self, enabled: bool) {
        self.main_bus.gpu.set_call_logging(enabled);
    }

    pub fn take_gpu_call_log(&mut self) -> Vec<DrawCall> {
        self.main_bus.gpu.take_call_log()
    }

    pub fn clear_gpu_call_log(&mut self) {
        self.main_bus.gpu.clear_call_log();
    }

    pub fn add_watchpoint(&mut self, addr: u32) {
        println!(
            "Adding watchpoint for addr {:#X} ({:#X} masked)",
            addr,
            addr & 0x1fffffff
        );
        self.watchpoints.push(addr & 0x1FFFFFFF);
    }

    pub fn remove_watchpoint(&mut self, addr: u32) {
        self.watchpoints.retain(|&x| x != addr & 0x1FFFFFFF);
    }

    pub fn pc(&self) -> u32 {
        self.r3000.pc
    }

    pub fn display_origin(&self) -> (usize, usize) {
        self.main_bus.gpu.display_origin()
    }

    pub fn get_irq_mask(&self) -> u32 {
        self.r3000.i_mask
    }

    pub fn exit_requested(&self) -> bool {
        if self.exit_requested {
            return true;
        } else {
            return false;
        }
    }
}

pub fn toggle_memory_logging(enabled: bool) {
    unsafe {
        LOGGING = enabled;
    }
}
