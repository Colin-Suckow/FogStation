use std::collections::HashMap;
use std::convert::TryFrom;

use bit_field::BitField;

use cop0::Cop0;
use instruction::decode_opcode;
use log::warn;

use crate::bus::MainBus;
use crate::cpu::instruction::RegisterNames;
use crate::Scheduler;

use self::gte::GTE;

mod cop0;
mod gte;
mod instruction;
mod interpreter;
mod jit;

#[derive(Debug, Clone, Copy)]
pub enum InterruptSource {
    VBLANK,
    GPU,
    CDROM,
    DMA,
    TMR0,
    TMR1,
    TMR2,
    Controller,
    SIO,
    SPU,
    Lightpen,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Exception {
    IBE = 6,  //Bus error
    DBE = 7,  //Bus error Data
    AdEL = 4, //Address Error Load
    AdES = 5, //Address Error Store
    Ovf = 12, //Overflow
    Sys = 8,  //System Call
    Bp = 9,   //Breakpoint
    RI = 10,  //Reserved Instruction
    CpU = 11, //Co-processor Unusable
    TLBL = 2, //TLB Miss Load
    TLBS = 3, //TLB Miss Store
    Mod = 1,  // TLB modified
    Int = 0,  //Interrupt
}

#[derive(Debug)]
struct LoadDelay {
    register: u8,
    value: u32,
}

pub struct R3000 {
    pub gen_registers: [u32; 32],
    cycle_count: u32,
    pub pc: u32,
    current_pc: u32,
    pub hi: u32,
    pub lo: u32,
    delay_slot: u32,
    pub cop0: Cop0,
    load_delay: Option<LoadDelay>,
    pub i_mask: u32,
    pub i_status: u32,
    pub log: bool,
    pub load_exe: bool,
    exec_delay: bool,
    last_was_branch: bool,
    gte: GTE,
    pub last_touched_addr: u32,
    pub entrypoint: u32,

    pub inst_map: HashMap<String, u32>
}

impl R3000 {
    pub fn new() -> R3000 {
        R3000 {
            gen_registers: [0; 32],
            cycle_count: 0,
            pc: 0,
            current_pc: 0,
            hi: 0,
            lo: 0,
            delay_slot: 0,
            cop0: Cop0::new(),
            load_delay: None,
            i_mask: 0,
            i_status: 0,
            log: false,
            load_exe: false,
            exec_delay: false,
            last_was_branch: false,
            gte: GTE::new(),
            last_touched_addr: 0,
            entrypoint: 0,
            inst_map: HashMap::new()
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
        self.cop0
            .write_reg(12, self.cop0.read_reg(12).set_bit(23, true).clone());
        self.load_delay = None;
    }

    #[allow(dead_code)]
    fn print_string(&mut self, addr: u32, main_bus: &mut MainBus) {
        let val = main_bus.read_byte(addr);
        if val == 0 {
            //Null, end of string
            return;
        }
        print!("{}", std::str::from_utf8(&[val]).unwrap());
        self.print_string(addr + 1, main_bus);
    }

    fn print_registers(&self) {
        for r in 0..=32 {
            print!(
                "{:#4} : {:#10X}, ",
                RegisterNames::try_from(r as usize).unwrap(),
                self.read_reg(r)
            );
            if r % 8 == 0 && r != 0 {
                println!("");
            }
        }
        println!("");
    }

    pub fn step_instruction(&mut self, main_bus: &mut MainBus, scheduler: &mut Scheduler) -> bool {

        let mut ran_delay_inst = false;

        //Fast load exe
        if self.load_exe && self.pc == 0xbfc0700c {
            println!("Jumping to exe...");
            self.pc = self.entrypoint;
        }

        if self.pc == 0xB0 {
            // SYSCALL: Send character to serial port
            // This catches any characters and prints them to stdout instead
            match self.read_reg(9) {
                0x35 => {
                    if self.read_reg(RegisterNames::a0 as u8) == 1 {
                        //Writing to stdout
                        let len = self.read_reg(RegisterNames::a2 as u8);
                        let base = self.read_reg(RegisterNames::a1 as u8);
                        for i in 0..len {
                            let char = self.read_bus_byte(base + i, main_bus);
                            print!("{}", unsafe { std::str::from_utf8_unchecked(&[char]) });
                        }
                    }
                }

                0x3D => {
                    print!("{}", unsafe {
                        std::str::from_utf8_unchecked(&[self.read_reg(4) as u8])
                    })
                }
                _ => (),
            }
        }

        if self.pc == 0xA0 {
            //println!("SYSCALL A({:#X}) pc: {:#X}", self.read_reg(9), self.current_pc);
            if self.read_reg(9) == 0x40 {
                println!("Unhandled exception hit!");
                println!("PC was {:#X}", self.current_pc);
                println!("Registers were:");
                self.print_registers();
                println!("");
                panic!();
            }
        }

        if self.pc == 0xC0 {
            //trace!("SYSCALL C({:#X}) pc: {:#X}", self.read_reg(9), self.current_pc);
        }

        // Handle SPU irq
        if main_bus.spu.check_and_ack_irq() {
            self.fire_external_interrupt(InterruptSource::SPU);
        }
        
        // Handle interrupts
        let mut cause = self.cop0.read_reg(13);
        cause.set_bit(10, self.i_status & self.i_mask != 0);
        self.cop0.write_reg(13, cause);

        if self.cop0.interrupts_enabled() && cause & 0x700 != 0 {
            //println!("Interrupt hit! i_status: {:#X}", self.i_status);
            self.fire_exception(Exception::Int);
        }

        let instruction = main_bus.read_word(self.pc, scheduler);
        self.current_pc = self.pc;
        self.pc += 4;

        self.exec_delay = false;
        self.last_was_branch = false;

        if self.log {
            self.log_instruction(instruction, main_bus);
        }
        self.cycle_count = self.cycle_count.wrapping_add(1);
        self.run_opcode(instruction, main_bus, scheduler);

        // if main_bus.last_touched_addr == 0x121CA8 {
        //     println!("lta pc {:#X} val {:#X}", self.current_pc, main_bus.read_word(0x121CA8));
        //     self.last_touched_addr = 0;
        // }

        //Execute branch delay operation
        if self.delay_slot != 0 {
            ran_delay_inst = true;
            let delay_instruction = main_bus.read_word(self.delay_slot, scheduler);
            if self.log {
                self.log_instruction(delay_instruction, main_bus);
            }
            //self.trace_file.write(format!("{:08x}: {:08x}\n", self.delay_slot, delay_instruction).as_bytes());
            //println!("{:08x}: {:08x}", self.delay_slot, delay_instruction);
            self.exec_delay = true;
            self.cycle_count = self.cycle_count.wrapping_add(1);
            self.run_opcode(delay_instruction, main_bus, scheduler);
            self.exec_delay = false;
            self.delay_slot = 0;
        };
        ran_delay_inst
    }

    fn flush_load_delay(&mut self) {
        if let Some(delay) = self.load_delay.take() {
            self.write_reg(delay.register, delay.value);
        }
    }

    fn log_instruction(&self, instruction: u32, main_bus: &mut MainBus) {
        let inst = decode_opcode(instruction).unwrap();
        // println!(
        //     "{:#X} : {:?} rs: {:#X} rt: {:#X} rd: {:#X}",
        //     self.current_pc,
        //     inst,
        //     self.read_reg(instruction.rs()),
        //     self.read_reg(instruction.rt()),
        //     self.read_reg(instruction.rd()),
        // );

        println!(
            "{:08x} {:08x}: {:<7}{}",
            self.current_pc,
            instruction,
            inst.mnemonic(),
            inst.arguments(self, main_bus)
        );
    }

    pub fn run_opcode(&mut self, opcode: u32, main_bus: &mut MainBus, scheduler: &mut Scheduler) {
        if self.pc % 4 != 0 || self.delay_slot % 4 != 0 {
            warn!("Tried to execute out of alignment");
            self.fire_exception(Exception::AdEL);
            return;
        }

        if let Some(inst) = decode_opcode(opcode) {
            // let inst_count = self.inst_map.entry(inst.mnemonic().into()).or_insert(0);
            // *inst_count += 1;
            inst.execute(self, main_bus, scheduler);
        } else {
            panic!("Unknown opcode! {:X}", opcode);
        }
    }

    pub fn fire_exception(&mut self, exception: Exception) {
        //println!("CPU EXCEPTION: Type: {:?} PC: {:#X}", exception, self.current_pc);
        self.flush_load_delay();

        self.cop0.set_cause_execode(&exception);

        if self.delay_slot != 0 {
            self.cop0.write_reg(13, self.cop0.read_reg(13) | (1 << 31));
            self.cop0.write_reg(14, self.pc - 8);
        } else {
            self.cop0.write_reg(13, self.cop0.read_reg(13) & !(1 << 31));
            if exception == Exception::Int {
                self.cop0.write_reg(14, self.pc);
            } else {
                self.cop0.write_reg(14, self.pc - 4);
            }
        }

        let old_status = self.cop0.read_reg(12);
        self.cop0.write_reg(
            12,
            (old_status & !0x3F) | (((old_status & 0x3f) << 2) & 0x3f),
        );
        self.pc = if self.cop0.read_reg(12).get_bit(23) {
            0xBFC0_0180
        } else {
            0x8000_0080
        };

        //self.cop0.write_reg(12, self.cop0.read_reg(12) << 4)
    }

    pub fn fire_external_interrupt(&mut self, source: InterruptSource) {
        //println!("Recieved interrupt interrupt request from: {:?}", source);
        let mask_bit = source as usize;
        self.i_status.set_bit(mask_bit, true);
    }

    pub fn read_bus_word(&mut self, addr: u32, main_bus: &mut MainBus, scheduler: &mut Scheduler) -> u32 {
        //self.last_touched_addr = addr & 0x1fffffff;

        match addr & 0x1fffffff {
            0x1F801070 => {
                //println!("Reading ISTATUS");
                self.i_status
            }
            0x1F801074 => self.i_mask,
            _ => main_bus.read_word(addr, scheduler),
        }
    }

    pub fn write_bus_word(&mut self, addr: u32, val: u32, main_bus: &mut MainBus, scheduler: &mut Scheduler) {
        self.last_touched_addr = addr & 0x1fffffff;

        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }

        match addr & 0x1fffffff {
            0x1F801070 => {
                self.i_status &= val & 0x3FF;
            }
            0x1F801074 => {
                //println!("Writing I_MASK val {:#X}", val);
                self.i_mask = val;
            }
            _ => main_bus.write_word(addr, val, scheduler),
        };
    }

    fn read_bus_half_word(&mut self, addr: u32, main_bus: &mut MainBus, scheduler: &mut Scheduler) -> u16 {
        // if addr == 0x1F801C0C {
        //     println!("Read spu thing at pc {:#X}", self.current_pc);
        // }
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status as u16,
            0x1F801074 => self.i_mask as u16,
            _ => main_bus.read_half_word(addr, scheduler),
        }
    }

    pub fn read_bus_byte(&mut self, addr: u32, main_bus: &mut MainBus) -> u8 {
        //self.last_touched_addr = addr & 0x1fffffff;
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status as u8,
            0x1F801072 => (self.i_status >> 8) as u8,
            0x1F801074 => self.i_mask as u8,
            0x1F801076 => (self.i_mask >> 8) as u8,
            _ => main_bus.read_byte(addr),
        }
    }

    fn write_bus_half_word(&mut self, addr: u32, val: u16, main_bus: &mut MainBus, scheduler: &mut Scheduler,) {
        self.last_touched_addr = addr & 0x1fffffff;
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }

        match addr & 0x1fffffff {
            0x1F801070 => self.i_status &= (val & 0x3FF) as u32,
            0x1F801074 => self.i_mask = val as u32,
            _ => main_bus.write_half_word(addr, val, scheduler),
        };
    }

    pub fn write_bus_byte(&mut self, addr: u32, val: u8, main_bus: &mut MainBus, scheduler: &mut Scheduler) {
        self.last_touched_addr = addr & 0x1fffffff;
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status &= (val as u32) & 0x3FF,
            0x1F801074 => self.i_mask = val as u32,
            _ => main_bus.write_byte(addr, val, scheduler),
        };
    }

    /// Returns the value stored within the given register. Will panic if register_number > 31
    pub fn read_reg(&self, register_number: u8) -> u32 {
        if register_number != 0 {
            self.gen_registers[register_number as usize]
        } else {
            0
        }
    }

    /// Sets register to given value. Prevents setting R0, which should always be zero. Will panic if register_number > 31
    fn write_reg(&mut self, register_number: u8, value: u32) {
        match register_number {
            0 => (), //Prevent writing to the zero register
            _ => self.gen_registers[register_number as usize] = value,
        }
    }

    /// Processes the current load delay and replaces it with a new one
    fn delayed_load(&mut self, register_number: u8, value: u32) {
        if let Some(current_delay) = self.load_delay.take() {
            if current_delay.register != register_number {
                self.write_reg(current_delay.register, current_delay.value);
            }
        }
        self.load_delay = Some(LoadDelay {
            register: register_number,
            value: value,
        });
    }
}
