use bit_field::BitField;

use cop0::Cop0;
use instruction::{Instruction, NumberHelpers};
use log::{trace, warn};

use crate::timer::TimerState;
use crate::{bus::MainBus, cdrom};

use self::gte::GTE;

mod cop0;
mod instruction;
mod gte;

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
    cycle_loaded: u32,
}

pub struct R3000 {
    pub gen_registers: [u32; 32],
    cycle_count: u32,
    pub pc: u32,
    current_pc: u32,
    pub hi: u32,
    pub lo: u32,
    pub main_bus: MainBus,
    delay_slot: u32,
    pub cop0: Cop0,
    load_delays: Vec<LoadDelay>,
    i_mask: u32,
    pub i_status: u32,
    pub log: bool,
    pub load_exe: bool,
    exec_delay: bool,
    last_was_branch: bool,
    gte: GTE,
    pub last_touched_addr: u32,
}

impl R3000 {
    pub fn new(bus: MainBus) -> R3000 {
        R3000 {
            gen_registers: [0; 32],
            cycle_count: 0,
            pc: 0,
            current_pc: 0,
            hi: 0,
            lo: 0,
            main_bus: bus,
            delay_slot: 0,
            cop0: Cop0::new(),
            load_delays: Vec::new(),
            i_mask: 0,
            i_status: 0,
            log: false,
            load_exe: false,
            exec_delay: false,
            last_was_branch: false,
            gte: GTE::new(),
            last_touched_addr: 0,
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
        self.load_delays = Vec::new();
    }

    fn print_string(&mut self, addr: u32) {
        let val = self.main_bus.read_byte(addr);
        if val == 0 {
            //Null, end of string
            return;
        }
        print!("{}", std::str::from_utf8(&[val]).unwrap());
        self.print_string(addr + 1);
    }

    pub fn step_instruction(&mut self, timers: &mut TimerState) {
        //Fast load exe

        if self.load_exe && self.pc == 0xbfc0700c {
            println!("Jumping to exe...");
            self.pc = 0x80010000;
        }
     
        if self.pc == 0xB0 {
            // SYSCALL: Send character to serial port
            // This catches any characters and prints them to stdout instead
            if self.read_reg(9) == 0x3D {
                print!(
                    "{}",
                    std::str::from_utf8(&[self.read_reg(4) as u8]).unwrap()
                );
            } else {
                trace!("SYSCALL B({:#X}) pc: {:#X}", self.read_reg(9), self.current_pc);
            }
        }

        if self.pc == 0xA0 {
            trace!("SYSCALL A({:#X}) pc: {:#X}", self.read_reg(9), self.current_pc);
        }

        if self.pc == 0xC0 {
            trace!("SYSCALL C({:#X}) pc: {:#X}", self.read_reg(9), self.current_pc);
        }


        //Check for vblank
        if self.main_bus.gpu.consume_vblank() {
            self.fire_external_interrupt(InterruptSource::VBLANK);
        };

        // Handle interrupts
        let mut cause = self.cop0.read_reg(13);
        cause.set_bit(10, self.i_status & self.i_mask != 0);
        self.cop0.write_reg(13, cause);


        if self.cop0.interrupts_enabled() && cause & 0x700 != 0 {
            self.fire_exception(Exception::Int);
        }

        let instruction = self.main_bus.read_word(self.pc);
        self.current_pc = self.pc;
        self.pc += 4;

        if self.log {
            println!(
                "Executing {:#X} (FUNCT {:#X}) at {:#X} rs: {} rt: {} rd: {} (FULL {:#X})",
                instruction.opcode(),
                instruction.funct(),
                self.current_pc,
                instruction.rs(),
                instruction.rt(),
                instruction.rd(),
                instruction
            );
        }

        self.exec_delay = false;
        self.last_was_branch = false;
        for i in (0..self.load_delays.len()).rev() {
            if self.load_delays[i].cycle_loaded != self.cycle_count {
                self.write_reg(self.load_delays[i].register, self.load_delays[i].value);
                self.load_delays.remove(i);
            }
        }
        self.execute_instruction(instruction, timers);
        self.cycle_count = self.cycle_count.wrapping_add(1);


        //Execute branch delay operation
        if self.delay_slot != 0 {
            let delay_instruction = self.main_bus.read_word(self.delay_slot);
            if self.log {
                println!(
                    "DS executing {:#X} (FUNCT {:#X}) at {:#X} rs: {} ({:#}) rt: {} rd: {}",
                    delay_instruction.opcode(),
                    delay_instruction.funct(),
                    self.current_pc + 4,
                    instruction.rs(),
                    self.gen_registers[instruction.rs() as usize],
                    instruction.rt(),
                    instruction.rd()
                );
            }
            //self.trace_file.write(format!("{:08x}: {:08x}\n", self.delay_slot, delay_instruction).as_bytes());
            //println!("{:08x}: {:08x}", self.delay_slot, delay_instruction);
            self.exec_delay = true;
            for i in (0..self.load_delays.len()).rev() {
                if self.load_delays[i].cycle_loaded != self.cycle_count {
                    self.write_reg(self.load_delays[i].register, self.load_delays[i].value);
                    self.load_delays.remove(i);
                }
            }
            self.execute_instruction(delay_instruction, timers);
            self.cycle_count = self.cycle_count.wrapping_add(1);
            self.exec_delay = false;
            self.delay_slot = 0;
        }
        
    }

    pub fn execute_instruction(&mut self, instruction: u32, timers: &mut TimerState) {
        if self.pc % 4 != 0 || self.delay_slot % 4 != 0 {
            warn!("Tried to execute out of alignment");
            self.fire_exception(Exception::AdEL);
            return;
        }

        match instruction.opcode() {
            0x0 => {
                //SPECIAL INSTRUCTIONS
                match instruction.funct() {
                    0x0 => {
                        //SLL
                        // if instruction.rt() == 0 {
                        //     //Actually a NOP
                        //     return;
                        // }
                        self.op_sll(instruction);
                        //println!("{:#X} << {:#X} = {:#X}", self.read_reg(instruction.rt()), instruction.shamt(), self.read_reg(instruction.rd()));
                    }

                    0x2 => {
                        //SRL
                        self.op_srl(instruction);
                    }

                    0x3 => {
                        //SRA
                        self.op_sra(instruction);
                    }

                    0x4 => {
                        //SLLV
                        self.op_sllv(instruction);
                    }

                    0x6 => {
                        //SRLV
                        self.op_srlv(instruction);
                    }

                    0x7 => {
                        //SRAV
                        self.op_srav(instruction);
                    }

                    0x8 => {
                        //JR
                        self.op_jr(instruction)
                    }

                    0x9 => {
                        //JALR
                        self.op_jalr(instruction)
                    }

                    0xC => {
                        //SYSCALL
                        //println!("SYSCALL {:#X}", self.read_reg(9));
                        self.op_syscall();
                    }

                    0xD => {
                        //BREAK
                        self.op_break();
                    }

                    0x10 => {
                        //MFHI
                        self.op_mfhi(instruction);
                    }

                    0x11 => {
                        //MTHI
                        self.op_mthi(instruction);
                    }

                    0x12 => {
                        //MFLO
                        self.op_mflo(instruction);
                    }

                    0x13 => {
                        //MTLO
                        self.op_mtlo(instruction);
                    }

                    0x1A => {
                        //DIV
                        self.op_div(instruction);
                    }

                    0x1B => {
                        //DIVU
                        self.op_divu(instruction);
                    }

                    0x20 => {
                        //ADD
                        self.op_add(instruction);
                    }

                    0x22 => {
                        //SUB
                        self.op_sub(instruction);
                    }

                    0x2B => {
                        //SLTU
                        self.op_sltu(instruction);
                    }

                    0x23 => {
                        //SUBU
                        self.op_subu(instruction);
                    }

                    0x24 => {
                        //AND
                        //println!("{} ({:#X}) & {} ({:#X}) = {} ({:#X})", instruction.rs(), self.read_reg(instruction.rs()), instruction.rt(), self.read_reg(instruction.rt()), instruction.rd(), self.read_reg(instruction.rs()) & self.read_reg(instruction.rt()));
                        self.op_and(instruction);
                    }

                    0x25 => {
                        //OR
                        self.op_or(instruction);
                    }

                    0x26 => {
                        //XOR
                        self.op_xor(instruction);
                    }

                    0x27 => {
                        //NOR
                        self.op_nor(instruction);
                    }

                    0x21 => {
                        //ADDU
                        self.op_addu(instruction);
                    }

                    0x18 => {
                        //MULT
                        self.op_mult(instruction);
                    }

                    0x19 => {
                        //MULTU
                        self.op_multu(instruction);
                    }

                    0x2A => {
                        //SLT
                        self.op_slt(instruction);
                    }

                    _ => panic!(
                        "CPU: Unknown SPECIAL instruction. FUNCT is {0} ({0:#08b}, {0:#X}) PC {1:#X} FULL {2:#X}",
                        instruction.funct(),
                        self.pc,
                        instruction
                    ),
                }
            }

            0x1 => {
                //"PC-relative" test and branch instructions
                match instruction.rt() {
                    0x0 => {
                        //BLTZ
                        self.last_was_branch = true;
                        self.op_bltz(instruction)
                    }
                    0x1 => {
                        //BGEZ
                        self.last_was_branch = true;

                        self.op_bgez(instruction)
                    }

                    0x10 => {
                        //BLTZAL
                        self.last_was_branch = true;

                        self.op_bltzal(instruction)
                    }

                    0x11 => {
                        //BGEZAL
                        self.last_was_branch = true;
                        self.op_bgezal(instruction)
                    }
                    _ => (), //psxtest_cpu spams a bunch of invalid instructions, so I'm not printing anything
                }
            }

            0x2 => {
                //J
                self.op_j(instruction);
            }

            0x3 => {
                //JAL
                self.op_jal(instruction);
            }

            0x4 => {
                //BEQ
                self.last_was_branch = true;
                self.op_beq(instruction);
            }

            0x5 => {
                //BNE
                self.last_was_branch = true;
                self.op_bne(instruction);
            }

            0x6 => {
                //BLEZ
                self.last_was_branch = true;
                self.op_blez(instruction);
            }

            0x7 => {
                //BGTZ
                self.last_was_branch = true;
                self.op_bgtz(instruction);
            }

            0x8 => {
                //ADDI
                self.op_addi(instruction);
            }

            0x9 => {
                //ADDIU
                //println!("Value {:#X}", instruction.immediate_sign_extended());
                self.op_addiu(instruction);
            }

            0xA => {
                //SLTI
                self.op_slti(instruction);
            }

            0xB => {
                //SLTIU
                self.op_sltiu(instruction);
            }

            0xC => {
                //ANDI
                self.op_andi(instruction);
            }

            0xD => {
                //ORI
                self.op_ori(instruction);
            }

            0xE => {
                //XORI
                self.op_xori(instruction);
            }
            0xF => {
                //LUI
                self.op_lui(instruction);
            }

            0x10 => {
                //COP0 instructions
                match instruction.rs() {
                    0x4 => {
                        //MTC0
                        self.op_mtc0(instruction);
                    }
                    0x0 => {
                        //MFC0
                        //println!("Reading COP0 reg {}. Val {:#X}", instruction.rd(), self.cop0.read_reg(instruction.rd()));
                        self.op_mfc0(instruction);
                    }

                    0x10 => {
                        //RFE
                        self.op_rfe();
                    }
                    _ => panic!(
                        "CPU: Unknown COP0 MFC instruction {:#X} ({0:#b}, {0})",
                        instruction.rs()
                    ),
                }
            }

            0x12 => {
                //COP2 (GTE) instructions
                if instruction.get_bit(25) {
                    //COP2 imm25
                    // Execute immediate GTE command
                    self.gte.execute_command(instruction & 0x1FFFFFF);
                } else {
                    match instruction.rs() {
                        0x0 => {
                            //MFC2
                            //This one will just return 0 for now
                            self.write_reg(instruction.rt(), self.gte.data_register(instruction.rd() as usize));
                        }
    
                        0x6 => {
                            //CTC2
                            let val = self.read_reg(instruction.rt());
                            self.gte.set_control_register(instruction.rd() as usize, val);
                        }
    
                        0x4 => {
                            //MTC2
                            let val = self.read_reg(instruction.rt());
                            self.gte.set_data_register(instruction.rd() as usize, val);
                        }
    
                        0x2 => {
                            //CFC2
                            self.write_reg(instruction.rt(), self.gte.control_register(instruction.rd() as usize));
                        }
    
                        _ => panic!(
                            "CPU: Unknown COP2 MFC instruction {:#X} ({0:#b}, {0}) {:#b}",
                            instruction.rs(),
                            instruction
                        ),
                    }
                }
            }

            0x20 => {
                //LB
                self.op_lb(instruction);
            }

            0x21 => {
                //LH
                self.op_lh(instruction, timers);
            }

            0x23 => {
                //LW
                self.op_lw(instruction, timers);
            }

            0x24 => {
                //LBU
                self.op_lbu(instruction);
            }

            0x25 => {
                //LHU
                self.op_lhu(instruction, timers);
            }

            0x28 => {
                //SB
                self.op_sb(instruction);
            }

            0x29 => {
                //SH
                self.op_sh(instruction, timers);
            }

            0x22 => {
                //LWL
                self.op_lwl(instruction, timers);
            }

            0x26 => {
                //LWR
                self.op_lwr(instruction, timers);
            }

            0x2A => {
                //SWL
                self.op_swl(instruction, timers);
            }

            0x2E => {
                //SWR
                self.op_swr(instruction, timers);
            }

            0x2B => {
                //SW
                //println!("R{} value {:#X}", instruction.rs(), self.read_reg(instruction.rs()));
                //println!("PC WAS {:#X}", self.pc - 4);

                self.op_sw(instruction, timers);
            }

            0x32 => {
                //LWC2
                let addr = instruction
                    .immediate_sign_extended()
                    .wrapping_add(self.read_reg(instruction.rs()));
                let val = self.read_bus_word(addr, timers);
                self.gte.set_data_register(instruction.rt() as usize, val);

            }

            0x3A => {
                //SWC2
                let addr = instruction
                    .immediate_sign_extended()
                    .wrapping_add(self.read_reg(instruction.rs()));
                let val = if instruction.rt() > 31 {
                    self.gte.control_register(instruction.rt() as usize - 32)
                } else {
                    self.gte.data_register(instruction.rt() as usize)
                };
                self.write_bus_word(addr, val, timers);

            }

            
            _ => panic!(
                "CPU: Unknown opcode {0} ({0:#08b}, {0:#X}) PC {1:#X} FULL {2:#X}",
                instruction.opcode(),
                self.current_pc,
                instruction
            ),
        };
    }

    fn op_sw(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr = instruction
            .immediate_sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));

        if addr % 4 != 0 {
            //unaligned address
            trace!("AdES fired by op_sw");
            self.fire_exception(Exception::AdES);
        } else {
            let val = self.read_reg(instruction.rt());
            self.write_bus_word(addr, val, timers);
        };
    }

    fn op_swr(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr = instruction
            .immediate()
            .sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));
        let word = self.read_bus_word(addr & !3, timers);
        let reg_val = self.read_reg(instruction.rt());
        self.write_bus_word(
            addr & !3,
            match addr & 3 {
                0 => (word & 0x00000000) | (reg_val << 0),
                1 => (word & 0x000000ff) | (reg_val << 8),
                2 => (word & 0x0000ffff) | (reg_val << 16),
                3 => (word & 0x00ffffff) | (reg_val << 24),
                _ => unreachable!(),
            },
            timers,
        );
    }

    fn op_swl(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr = instruction
            .immediate()
            .sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));
        let word = self.read_bus_word(addr & !3, timers);
        let reg_val = self.read_reg(instruction.rt());
        self.write_bus_word(
            addr & !3,
            match addr & 3 {
                0 => (word & 0xffffff00) | (reg_val >> 24),
                1 => (word & 0xffff0000) | (reg_val >> 16),
                2 => (word & 0xff000000) | (reg_val >> 8),
                3 => (word & 0x00000000) | (reg_val >> 0),
                _ => unreachable!(),
            },
            timers,
        );
    }

    fn op_lwr(&mut self, instruction: u32, timers: &mut TimerState) {

        let addr = instruction
            .immediate()
            .sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));

        let word = self.read_bus_word(addr & !3, timers);

        // LWR can ignore the load delay, so check if theres an existing load delay and fetch the rt value
        // from there if it exists
        let mut reg_val = self.read_reg(instruction.rt());

        for ld in &self.load_delays {
            if ld.register == instruction.rt() {
                reg_val = ld.value;
            }
        }

        self.delay_write_reg(
            instruction.rt(),
            match addr & 3 {
                3 => (reg_val & 0xffffff00) | (word >> 24),
                2 => (reg_val & 0xffff0000) | (word >> 16),
                1 => (reg_val & 0xff000000) | (word >> 8),
                0 => (reg_val & 0x00000000) | (word >> 0),
                _ => unreachable!(),
            },
        );
    }

    fn op_lwl(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr = instruction
            .immediate()
            .sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));

        let word = self.read_bus_word(addr & !3, timers);
        
        // LWL can ignore the load delay, so check if theres an existing load delay and fetch the rt value
        // from there if it exists
        let mut reg_val = self.read_reg(instruction.rt());

        for ld in &self.load_delays {
            if ld.register == instruction.rt() {
                reg_val = ld.value;
            }
        }

        self.delay_write_reg(
            instruction.rt(),
            match addr & 3 {
                0 => (reg_val & 0x00ffffff) | (word << 24),
                1 => (reg_val & 0x0000ffff) | (word << 16),
                2 => (reg_val & 0x000000ff) | (word << 8),
                3 => (reg_val & 0x00000000) | (word << 0),
                _ => unreachable!(),
            },
        );
    }

    fn op_sh(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr = instruction
            .immediate_sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));
        if addr % 2 != 0 {
            //unaligned address
            trace!("AdES fired by op_sh");
            self.fire_exception(Exception::AdES);
        } else {
            let val = (self.read_reg(instruction.rt()) & 0xFFFF) as u16;
            if addr == 0xD030028 {println!("PC {:#X}", self.pc);}
            self.write_bus_half_word(addr, val, timers);
        };
    }

    fn op_sb(&mut self, instruction: u32) {
        let addr = instruction
            .immediate_sign_extended()
            .wrapping_add(self.read_reg(instruction.rs()));
        let val = (self.read_reg(instruction.rt()) & 0xFF) as u8;
        self.write_bus_byte(addr, val);
    }

    fn op_lhu(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr =
            (instruction.immediate_sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
        if addr % 2 != 0 {
            trace!("AdEl fired by op_lhu");
            self.fire_exception(Exception::AdEL);
        } else {
            let val = self.read_bus_half_word(addr, timers).zero_extended();
            self.delay_write_reg(instruction.rt(), val);
        };
    }

    fn op_lbu(&mut self, instruction: u32) {
        let addr =
            (instruction.immediate_sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
        let val = self.main_bus.read_byte(addr).zero_extended();
        self.delay_write_reg(instruction.rt(), val);
    }

    fn op_lw(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr =
            (instruction.immediate_sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
        if addr % 4 != 0 {
            trace!("AdEl fired by op_lw");
            self.fire_exception(Exception::AdEL);
        } else {
            let val = self.read_bus_word(addr, timers);
            self.delay_write_reg(instruction.rt(), val);
        };
    }

    fn op_lh(&mut self, instruction: u32, timers: &mut TimerState) {
        let addr =
            (instruction.immediate_sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
        if addr % 2 != 0 {
            trace!("AdEl fired by op_lh");
            self.fire_exception(Exception::AdEL);
        } else {
            let val = self.read_bus_half_word(addr, timers).sign_extended();
            self.delay_write_reg(instruction.rt(), val);
        };
    }

    fn op_lb(&mut self, instruction: u32) {
        let addr =
            (instruction.immediate_sign_extended()).wrapping_add(self.read_reg(instruction.rs()));
        let val = self.main_bus.read_byte(addr).sign_extended();
        self.delay_write_reg(instruction.rt(), val);
    }

    fn op_rfe(&mut self) {
        let mode = self.cop0.read_reg(12) & 0x3f;
        let status = self.cop0.read_reg(12);
        self.cop0.write_reg(12, (status & !0xf) | (mode >> 2));
    }

    fn op_mfc0(&mut self, instruction: u32) {
        self.write_reg(instruction.rt(), self.cop0.read_reg(instruction.rd()));
    }

    fn op_mtc0(&mut self, instruction: u32) {
        self.cop0
            .write_reg(instruction.rd(), self.read_reg(instruction.rt()));
    }

    fn op_lui(&mut self, instruction: u32) {
        self.write_reg(instruction.rt(), (instruction.immediate() as u32) << 16);
    }

    fn op_xori(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            self.read_reg(instruction.rs()) ^ instruction.immediate().zero_extended(),
        );
    }

    fn op_ori(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            self.read_reg(instruction.rs()) | instruction.immediate().zero_extended(),
        );
    }

    fn op_andi(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            (instruction & 0xFFFF) & self.read_reg(instruction.rs()),
        );
    }

    fn op_sltiu(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            (self.read_reg(instruction.rs()) < instruction.immediate_sign_extended()) as u32,
        );
    }

    fn op_slti(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            ((self.read_reg(instruction.rs()) as i32)
                < instruction.immediate_sign_extended() as i32) as u32,
        );
    }

    fn op_addiu(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            ((self.read_reg(instruction.rs()) as i32).wrapping_add((instruction.immediate() as i16) as i32)) as u32,
        );
    }

    fn op_addi(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rt(),
            match (self.read_reg(instruction.rs()) as i32)
                .checked_add(instruction.immediate_sign_extended() as i32)
            {
                Some(val) => val as u32,
                None => {
                    self.fire_exception(Exception::Ovf);
                    return;
                }
            },
        );
    }

    fn op_bgtz(&mut self, instruction: u32) {
        if (self.read_reg(instruction.rs()) as i32) > 0 {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        };
    }

    fn op_blez(&mut self, instruction: u32) {
        if (self.read_reg(instruction.rs()) as i32) <= 0 {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        };
    }

    fn op_bne(&mut self, instruction: u32) {
        if self.read_reg(instruction.rs()) != self.read_reg(instruction.rt()) {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        };
    }

    fn op_beq(&mut self, instruction: u32) {
        if self.read_reg(instruction.rs()) == self.read_reg(instruction.rt()) {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        };
    }

    fn op_jal(&mut self, instruction: u32) {
        self.delay_slot = self.pc;
        self.write_reg(31, self.delay_slot + 4);
        self.pc = (instruction.address() << 2) | (self.delay_slot & 0xF0000000);
    }

    fn op_j(&mut self, instruction: u32) {
        self.delay_slot = self.pc;
        self.pc = (instruction.address() << 2) | ((self.delay_slot) & 0xF0000000);
    }

    fn op_bgezal(&mut self, instruction: u32) {
        let og_pc = self.pc;
        if self.read_reg(instruction.rs()) as i32 >= 0 {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        }
        self.write_reg(31, og_pc + 4);
    }

    fn op_bltzal(&mut self, instruction: u32) {
        let og_pc = self.pc;
        if self.read_reg(instruction.rs()).get_bit(31) {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        }
        self.write_reg(31, og_pc + 4);
    }

    fn op_bgez(&mut self, instruction: u32) {
        if self.read_reg(instruction.rs()) as i32 >= 0 {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        }
    }

    fn op_bltz(&mut self, instruction: u32) {
        if self.read_reg(instruction.rs()).get_bit(31) {
            self.delay_slot = self.pc;
            self.pc = (instruction.immediate_sign_extended() << 2).wrapping_add(self.delay_slot);
        }
    }

    fn op_slt(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            ((self.read_reg(instruction.rs()) as i32) < (self.read_reg(instruction.rt()) as i32))
                as u32,
        );
    }

    fn op_multu(&mut self, instruction: u32) {
        let result =
            (self.read_reg(instruction.rs()) as u64) * (self.read_reg(instruction.rt()) as u64);
        self.lo = (result & 0xFFFF_FFFF) as u32;
        self.hi = ((result >> 32) & 0xFFFF_FFFF) as u32;
    }

    fn op_mult(&mut self, instruction: u32) {
        let result = ((self.read_reg(instruction.rs()) as i32) as i64
            * (self.read_reg(instruction.rt()) as i32) as i64) as u64;
        self.lo = result as u32;
        self.hi = (result >> 32) as u32;
    }

    fn op_addu(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            (self.read_reg(instruction.rt()) as i32).wrapping_add(self.read_reg(instruction.rs()) as i32) as u32,
        );
    }

    fn op_nor(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            !(self.read_reg(instruction.rt()) | self.read_reg(instruction.rs())),
        );
    }

    fn op_xor(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            self.read_reg(instruction.rs()) ^ self.read_reg(instruction.rt()),
        );
    }

    fn op_or(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            self.read_reg(instruction.rs()) | self.read_reg(instruction.rt()),
        );
    }

    fn op_and(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            self.read_reg(instruction.rs()) & self.read_reg(instruction.rt()),
        );
    }

    fn op_subu(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            (self.read_reg(instruction.rs())).wrapping_sub(self.read_reg(instruction.rt())),
        );
    }

    fn op_sltu(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            (self.read_reg(instruction.rs()) < self.read_reg(instruction.rt())) as u32,
        );
    }

    fn op_sub(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            match (self.read_reg(instruction.rs()) as i32)
                .checked_sub(self.read_reg(instruction.rt()) as i32)
            {
                Some(val) => val as u32,
                None => {
                    self.fire_exception(Exception::Ovf);
                    return;
                }
            },
        );
    }

    fn op_add(&mut self, instruction: u32) {
        let val = match (self.read_reg(instruction.rs()) as i32)
            .checked_add(self.read_reg(instruction.rt()) as i32)
        {
            Some(val) => val as u32,
            None => {
                self.fire_exception(Exception::Ovf);
                return;
            }
        };
        self.write_reg(instruction.rd(), val)
    }

    fn op_divu(&mut self, instruction: u32) {
        let rs = self.read_reg(instruction.rs());
        let rt = self.read_reg(instruction.rt());
        self.lo = match rs.checked_div(rt) {
            Some(val) => val,
            None => {
                //println!("CPU: Tried to divide by zero at pc: {:#X}!", self.old_pc);
                self.hi = rs as u32;
                self.lo = 0xFFFFFFFF;
                return;
            }
        };
        self.hi = rs % rt;
    }

    fn op_div(&mut self, instruction: u32) {
        let rs = self.read_reg(instruction.rs()) as i32;
        let rt = self.read_reg(instruction.rt()) as i32;
        self.lo = (match rs.checked_div(rt) {
            Some(val) => val,
            None => {
                if rt == -1 {
                    self.hi = 0;
                    self.lo = 0x80000000 as u32;
                } else if rs < 0 {
                    self.hi = rs as u32;
                    self.lo = 1;
                } else {
                    self.hi = rs as u32;
                    self.lo = 0xffffffff as u32;
                }
                return;
            }
        }) as u32;
        self.hi = (rs % rt) as u32;
    }

    fn op_mtlo(&mut self, instruction: u32) {
        self.lo = self.read_reg(instruction.rs());
    }

    fn op_mflo(&mut self, instruction: u32) {
        self.write_reg(instruction.rd(), self.lo);
    }

    fn op_mthi(&mut self, instruction: u32) {
        self.hi = self.read_reg(instruction.rs());
    }

    fn op_mfhi(&mut self, instruction: u32) {
        self.write_reg(instruction.rd(), self.hi);
    }

    fn op_syscall(&mut self) {
        self.fire_exception(Exception::Sys);
    }

    fn op_jalr(&mut self, instruction: u32) {
        let target = self.read_reg(instruction.rs());
        self.write_reg(instruction.rd(), self.pc + 4);
        if target % 4 != 0 {
            trace!("AdEl fired by op_jalr");
            self.fire_exception(Exception::AdEL);
        } else {
            self.delay_slot = self.pc;
            self.pc = target;
        }
    }

    fn op_jr(&mut self, instruction: u32) {
        let target = self.read_reg(instruction.rs());
        if target % 4 != 0 {
            trace!("AdEl fired by op_jr");
            self.fire_exception(Exception::AdEL);
        } else {
            self.delay_slot = self.pc;
            self.pc = target;
        }
    }

    fn op_srav(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            ((self.read_reg(instruction.rt()) as i32) >> (self.read_reg(instruction.rs()) & 0x1F))
                as u32,
        );
    }

    fn op_srlv(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            ((self.read_reg(instruction.rt())) >> (self.read_reg(instruction.rs()) & 0x1F)) as u32,
        );
    }

    fn op_sllv(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            ((self.read_reg(instruction.rt())) << (self.read_reg(instruction.rs()) & 0x1F)) as u32,
        );
    }

    fn op_sra(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            (self.read_reg(instruction.rt()) as i32 >> instruction.shamt()) as u32,
        );
    }

    fn op_srl(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            self.read_reg(instruction.rt()) >> instruction.shamt(),
        );
    }

    fn op_sll(&mut self, instruction: u32) {
        self.write_reg(
            instruction.rd(),
            self.read_reg(instruction.rt()) << instruction.shamt(),
        );
    }

    fn op_break(&mut self) {
        self.fire_exception(Exception::Bp);
    }

    pub fn fire_exception(&mut self, exception: Exception) {
        trace!("CPU EXCEPTION: Type: {:?} PC: {:#X}", exception, self.current_pc);
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
        let mask_bit = source.clone() as usize;
        self.i_status.set_bit(mask_bit, true);
    }

    pub fn read_bus_word(&mut self, addr: u32, timers: &mut TimerState) -> u32 {
        //self.last_touched_addr = addr & 0x1fffffff;
        match addr & 0x1fffffff {
            0x1F801070 => {
                //println!("Reading ISTATUS");
                self.i_status
            }
            0x1F801074 => self.i_mask,
            0x1F801100..=0x1F801128 => timers.read_word(addr & 0x1fffffff),
            _ => self.main_bus.read_word(addr),
        }
    }

    pub fn write_bus_word(&mut self, addr: u32, val: u32, timers: &mut TimerState) {
        self.last_touched_addr = addr & 0x1fffffff;
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        

        match addr & 0x1fffffff {
            0x1F801070 => {
                self.i_status &= val;
            }
            0x1F801074 => {
                //println!("Writing I_MASK val {:#X}", val);
                self.i_mask = val;
            }
            0x1F801100..=0x1F801128 => timers.write_word(addr & 0x1fffffff, val),
            _ => self.main_bus.write_word(addr, val),
        };
    }

    fn read_bus_half_word(&mut self, addr: u32, timers: &mut TimerState) -> u16 {
        //self.last_touched_addr = addr & 0x1fffffff;
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status as u16,
            0x1F801074 => self.i_mask as u16,
            0x1F801100..=0x1F801128 => timers.read_half_word(addr & 0x1fffffff),
            _ => self.main_bus.read_half_word(addr),
        }
    }
    
    pub fn read_bus_byte(&mut self, addr: u32) -> u8 {
        //self.last_touched_addr = addr & 0x1fffffff;
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status as u8,
            0x1F801072 => (self.i_status >> 8) as u8,
            0x1F801074 => self.i_mask as u8,
            0x1F801076 => (self.i_mask >> 8) as u8,
            _ => self.main_bus.read_byte(addr),
        }
    }
   

    fn write_bus_half_word(&mut self, addr: u32, val: u16, timers: &mut TimerState) {
        self.last_touched_addr = addr & 0x1fffffff;
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status &= val as u32,
            0x1F801074 => self.i_mask = val as u32,
            0x1F801100..=0x1F801128 => timers.write_half_word(addr & 0x1fffffff, val),
            _ => self.main_bus.write_half_word(addr, val),
        };
    }

    pub fn write_bus_byte(&mut self, addr: u32, val: u8) {
        self.last_touched_addr = addr & 0x1fffffff;
        if self.cop0.cache_isolated() {
            //Cache is isolated, so don't write
            return;
        }
        match addr & 0x1fffffff {
            0x1F801070 => self.i_status &= val as u32,
            0x1F801074 => self.i_mask = val as u32,
            _ => self.main_bus.write_byte(addr, val),
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

    fn delay_write_reg(&mut self, register_number: u8, value: u32) {
        if register_number != 0 {
            //Get rid of old writes to the same register
            for i in (0..self.load_delays.len()).rev() {
                if self.load_delays[i].register == register_number {
                    self.load_delays.remove(i);
                }
            }
            self.load_delays.push(LoadDelay {
                register: register_number,
                value: value,
                cycle_loaded: self.cycle_count,
            });
        }
    }
}
