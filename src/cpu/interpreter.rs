use log::trace;

use crate::{cpu::Exception, timer::TimerState};

use super::{R3000, instruction::{InstructionArgs, NumberHelpers}};

fn op_sw(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr = instruction
        .immediate_sign_extended()
        .wrapping_add(cpu.read_reg(instruction.rs()));
    let val = cpu.read_reg(instruction.rt());
        
    cpu.flush_load_delay();

    if addr % 4 != 0 {
        //unaligned address
        trace!("AdES fired by op_sw");
        cpu.fire_exception(Exception::AdES);
    } else {
        cpu.write_bus_word(addr, val, timers);
    };
}

fn op_swr(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr = instruction
        .immediate_sign_extended()
        .wrapping_add(cpu.read_reg(instruction.rs()));
    let word = cpu.read_bus_word(addr & !3, timers);
    let reg_val = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_bus_word(
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

fn op_swl(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr = instruction
        .immediate_sign_extended()
        .wrapping_add(cpu.read_reg(instruction.rs()));
    let word = cpu.read_bus_word(addr & !3, timers);
    let reg_val = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_bus_word(
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

fn op_lwr(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr = instruction
        .immediate_sign_extended()
        .wrapping_add(cpu.read_reg(instruction.rs()));

    let word = cpu.read_bus_word(addr & !3, timers);

    // LWR can ignore the load delay, so check if theres an existing load delay and fetch the rt value
    // from there if it exists
    let mut reg_val = cpu.read_reg(instruction.rt());

    if let Some(delay) = &cpu.load_delay {
        if delay.register == instruction.rt() {
            reg_val = delay.value;
        }
    }

    cpu.delayed_load(
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

fn op_lwl(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr = instruction
    .immediate_sign_extended()
    .wrapping_add(cpu.read_reg(instruction.rs()));
    
    let word = cpu.read_bus_word(addr & !3, timers);
    
    // LWL can ignore the load delay, so check if theres an existing load delay and fetch the rt value
    // from there if it exists
    let mut reg_val = cpu.read_reg(instruction.rt());
    
    if let Some(delay) = &cpu.load_delay {
        if delay.register == instruction.rt() {
            reg_val = delay.value;
        }
    }

    cpu.delayed_load(
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

fn op_sh(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let base = instruction.immediate_sign_extended();
    let offset = cpu.read_reg(instruction.rs());
    let addr = base.wrapping_add(offset);
    let val = (cpu.read_reg(instruction.rt()) & 0xFFFF) as u16;
    cpu.flush_load_delay();
    if addr % 2 != 0 {
        //unaligned address
        trace!("AdES fired by op_sh pc {:#X}  addr {:#X}   s_reg  {}   s_reg_val  {:#X}   offset   {:#X}", cpu.current_pc, addr, instruction.rs(), offset , base);
        cpu.fire_exception(Exception::AdES);
    } else {
        cpu.write_bus_half_word(addr, val, timers);
    };
}

fn op_sb(cpu: &mut R3000, instruction: u32) {
    let addr = instruction
        .immediate_sign_extended()
        .wrapping_add(cpu.read_reg(instruction.rs()));
    let val = (cpu.read_reg(instruction.rt()) & 0xFF) as u8;
    cpu.flush_load_delay();
    cpu.write_bus_byte(addr, val);
}

fn op_lhu(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr =
        (instruction.immediate_sign_extended()).wrapping_add(cpu.read_reg(instruction.rs()));
    if addr % 2 != 0 {
        trace!("AdEl fired by op_lhu");
        cpu.flush_load_delay();
        cpu.fire_exception(Exception::AdEL);
    } else {
        let val = cpu.read_bus_half_word(addr, timers).zero_extended();
        cpu.delayed_load(instruction.rt(), val);
    };
}

fn op_lbu(cpu: &mut R3000, instruction: u32) {
    let addr =
        (instruction.immediate_sign_extended()).wrapping_add(cpu.read_reg(instruction.rs()));
    let val = cpu.main_bus.read_byte(addr).zero_extended();
    cpu.delayed_load(instruction.rt(), val);
}

fn op_lw(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let base = instruction.immediate_sign_extended();
    let offset = cpu.read_reg(instruction.rs());
    let addr = base.wrapping_add(offset);
    if addr % 4 != 0 {
        trace!("AdEl fired by op_lw   addr {:#X}   s_reg  {}   s_reg_val  {:#X}   offset   {:#X}", addr, instruction.rs(), offset , base);
        cpu.fire_exception(Exception::AdEL);
    } else {
        let val = cpu.read_bus_word(addr as u32, timers);
       
        //println!("lw addr {:08x} val {:08x} reg {}", addr, val, instruction.rt());
        
        cpu.delayed_load(instruction.rt(), val);
    };
}

fn op_lh(cpu: &mut R3000, instruction: u32, timers: &mut TimerState) {
    let addr =
        (instruction.immediate_sign_extended()).wrapping_add(cpu.read_reg(instruction.rs()));
    if addr % 2 != 0 {
        trace!("AdEl fired by op_lh");
        cpu.fire_exception(Exception::AdEL);
    } else {
        let val = cpu.read_bus_half_word(addr, timers).sign_extended();
        cpu.delayed_load(instruction.rt(), val as u32);
    };
}

fn op_lb(cpu: &mut R3000, instruction: u32) {
    let addr =
        (instruction.immediate_sign_extended()).wrapping_add(cpu.read_reg(instruction.rs()));
    let val = cpu.main_bus.read_byte(addr).sign_extended();
    cpu.delayed_load(instruction.rt(), val as u32);
}

fn op_rfe(cpu: &mut R3000) {
    cpu.flush_load_delay();
    let mode = cpu.cop0.read_reg(12) & 0x3f;
    let status = cpu.cop0.read_reg(12);
    cpu.cop0.write_reg(12, (status & !0xf) | (mode >> 2));
}

fn op_mfc0(cpu: &mut R3000, instruction: u32) {
    let val = cpu.cop0.read_reg(instruction.rd());
    cpu.flush_load_delay();
    cpu.delayed_load(instruction.rt(), val);
}

fn op_mtc0(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.cop0
        .write_reg(instruction.rd(), val);
}

fn op_lui(cpu: &mut R3000, instruction: u32) {
    cpu.flush_load_delay();
    cpu.write_reg(instruction.rt(), (instruction.immediate().zero_extended() << 16) as u32);
}

fn op_xori(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        val ^ instruction.immediate().zero_extended(),
    );
}

fn op_ori(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        val | instruction.immediate().zero_extended(),
    );
}

fn op_andi(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        instruction.immediate().zero_extended() & val,
    );
}

fn op_sltiu(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        (val < instruction.immediate_sign_extended() as u32) as u32,
    );
}

fn op_slti(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        ((val as i32)
            < instruction.immediate_sign_extended() as i32) as u32,
    );
}

fn op_addiu(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        val.wrapping_add(instruction.immediate_sign_extended()) as u32,
    );
}

fn op_addi(cpu: &mut R3000, instruction: u32) {
    let val = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rt(),
        match (val as i32)
            .checked_add(instruction.immediate_sign_extended() as i32)
        {
            Some(val) => val as u32,
            None => {
                cpu.fire_exception(Exception::Ovf);
                return;
            }
        },
    );
}

fn op_bgtz(cpu: &mut R3000, instruction: u32) {
    if (cpu.read_reg(instruction.rs()) as i32) > 0 {
        cpu.delay_slot = cpu.pc;
        cpu.pc = ((instruction.immediate_sign_extended() as u32) << 2).wrapping_add(cpu.delay_slot);
    };
    cpu.flush_load_delay();
}

fn op_blez(cpu: &mut R3000, instruction: u32) {
    if (cpu.read_reg(instruction.rs()) as i32) <= 0 {
        cpu.delay_slot = cpu.pc;
        cpu.pc = ((instruction.immediate_sign_extended() as u32) << 2).wrapping_add(cpu.delay_slot);
    };
    cpu.flush_load_delay();
}

fn op_bne(cpu: &mut R3000, instruction: u32) {
    if cpu.read_reg(instruction.rs()) != cpu.read_reg(instruction.rt()) {
        cpu.delay_slot = cpu.pc;
        cpu.pc = ((instruction.immediate_sign_extended() as u32) << 2).wrapping_add(cpu.delay_slot);
    };
    cpu.flush_load_delay();
}

fn op_beq(cpu: &mut R3000, instruction: u32) {
    if cpu.read_reg(instruction.rs()) == cpu.read_reg(instruction.rt()) {
        cpu.delay_slot = cpu.pc;
        cpu.pc = ((instruction.immediate_sign_extended() as u32) << 2).wrapping_add(cpu.delay_slot);
    };
    cpu.flush_load_delay();
}

fn op_jal(cpu: &mut R3000, instruction: u32) {
    cpu.delay_slot = cpu.pc;
    cpu.flush_load_delay();
    cpu.write_reg(31, cpu.delay_slot + 4);
    cpu.pc = (instruction.address() << 2) | (cpu.delay_slot & 0xF0000000);
}

fn op_j(cpu: &mut R3000, instruction: u32) {
    cpu.delay_slot = cpu.pc;
    cpu.pc = (instruction.address() << 2) | ((cpu.delay_slot) & 0xF0000000);
    cpu.flush_load_delay();
}

fn op_slt(cpu: &mut R3000, instruction: u32) {
    let t_val = cpu.read_reg(instruction.rt()) as i32;
    let s_val = cpu.read_reg(instruction.rs()) as i32;
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        (s_val < t_val)
            as u32,
    );
}

fn op_multu(cpu: &mut R3000, instruction: u32) {
    let m1 = cpu.read_reg(instruction.rs());
    let m2 = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();

    let result =
        (m1 as u64) * (m2 as u64);
    cpu.lo = result as u32;
    cpu.hi = (result >> 32) as u32;
}

fn op_mult(cpu: &mut R3000, instruction: u32) {
    let m1 = cpu.read_reg(instruction.rs());
    let m2 = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    let result = ((m1 as i32) as i64
        * (m2 as i32) as i64) as u64;
    cpu.lo = result as u32;
    cpu.hi = (result >> 32) as u32;
}

fn op_addu(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rt.wrapping_add(rs) as u32,
    );
}

fn op_nor(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        !(rt | rs),
    );
}

fn op_xor(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rs ^ rt,
    );
}

fn op_or(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rs | rt,
    );
    //println!("or ${}({:08x}) | ${}({:08x}) = ${}({:08x})", instruction.rs(), cpu.read_reg(instruction.rs()), instruction.rt(), cpu.read_reg(instruction.rt()), instruction.rd(), cpu.read_reg(instruction.rd()))
}

fn op_and(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rs & rt,
    );
}

fn op_subu(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rs.wrapping_sub(rt),
    );
}

fn op_sltu(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        (rs < rt) as u32,
    );
}

fn op_sub(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        match (rs as i32)
            .checked_sub(rt as i32)
        {
            Some(val) => val as u32,
            None => {
                cpu.fire_exception(Exception::Ovf);
                return;
            }
        },
    );
}

fn op_add(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    let val = match (rs as i32)
        .checked_add(rt as i32)
    {
        Some(val) => val as u32,
        None => {
            cpu.fire_exception(Exception::Ovf);
            return;
        }
    };
    cpu.write_reg(instruction.rd(), val)
}

fn op_divu(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    match rs.checked_div(rt) {
        Some(lo) => {
            cpu.lo = lo;
            cpu.hi = rs % rt;
        },
        None => {
            //println!("CPU: Tried to divide by zero at pc: {:#X}!", cpu.old_pc);
            cpu.hi = rs as u32;
            cpu.lo = 0xFFFFFFFF;
            return;
        }
    };
}

fn op_div(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs()) as i32;
    let rt = cpu.read_reg(instruction.rt()) as i32;
    cpu.flush_load_delay();
    match rs.checked_div(rt) {
        Some(lo) => {
            cpu.lo = lo as u32;
            cpu.hi = (rs % rt) as u32;
        },
        None => {
            if rt == -1 {
                cpu.hi = 0;
                cpu.lo = 0x80000000 as u32;
            } else if rs < 0 {
                cpu.hi = rs as u32;
                cpu.lo = 1;
            } else {
                cpu.hi = rs as u32;
                cpu.lo = 0xffffffff as u32;
            }
            return;
        }
    };
}

fn op_mtlo(cpu: &mut R3000, instruction: u32) {
    cpu.lo = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
}

fn op_mflo(cpu: &mut R3000, instruction: u32) {
    cpu.flush_load_delay();
    cpu.write_reg(instruction.rd(), cpu.lo);
}

fn op_mthi(cpu: &mut R3000, instruction: u32) {
    cpu.hi = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
}

fn op_mfhi(cpu: &mut R3000, instruction: u32) {
    cpu.flush_load_delay();
    cpu.write_reg(instruction.rd(), cpu.hi);
}

fn op_syscall(cpu: &mut R3000) {
    cpu.flush_load_delay();
    cpu.fire_exception(Exception::Sys);
}

fn op_jalr(cpu: &mut R3000, instruction: u32) {
    let target = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    cpu.write_reg(instruction.rd(), cpu.pc + 4);
    if target % 4 != 0 {
        trace!("AdEl fired by op_jalr");
        cpu.fire_exception(Exception::AdEL);
    } else {
        cpu.delay_slot = cpu.pc;
        cpu.pc = target;
    }
}

fn op_jr(cpu: &mut R3000, instruction: u32) {
    let target = cpu.read_reg(instruction.rs());
    cpu.flush_load_delay();
    if target % 4 != 0 {
        trace!("AdEl fired by op_jr");
        cpu.fire_exception(Exception::AdEL);
    } else {
        cpu.delay_slot = cpu.pc;
        cpu.pc = target;
    }
}

fn op_srav(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        ((rt as i32) >> (rs & 0x1F))
            as u32,
    );
}

fn op_srlv(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        ((rt) >> (rs & 0x1F)) as u32,
    );
}

fn op_sllv(cpu: &mut R3000, instruction: u32) {
    let rs = cpu.read_reg(instruction.rs());
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        ((rt) << (rs & 0x1F)) as u32,
    );
}

fn op_sra(cpu: &mut R3000, instruction: u32) {
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        ((rt as i32) >> instruction.shamt()) as u32,
    );
}

fn op_srl(cpu: &mut R3000, instruction: u32) {
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rt >> instruction.shamt(),
    );
}

fn op_sll(cpu: &mut R3000, instruction: u32) {
    let rt = cpu.read_reg(instruction.rt());
    cpu.flush_load_delay();
    cpu.write_reg(
        instruction.rd(),
        rt << instruction.shamt(),
    );
}

fn op_break(cpu: &mut R3000) {
    cpu.flush_load_delay();
    cpu.fire_exception(Exception::Bp);
}