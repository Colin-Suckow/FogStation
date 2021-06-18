use std::error::Error;

use gdbstub::{arch, target::{Target, TargetResult, ext::{base::{ResumeAction, singlethread::{SingleThreadOps, StopReason}}, breakpoints::{HwBreakpoint, HwWatchpoint, SwBreakpoint, SwBreakpointOps}}}};
use crate::{EmuMessage, EmuState, emu_loop_step};

impl Target for EmuState {
    type Arch = arch::mips::Mips;

    type Error = &'static str;

    fn base_ops(&mut self) -> gdbstub::target::ext::base::BaseOps<Self::Arch, Self::Error> {
        gdbstub::target::ext::base::BaseOps::SingleThread(self)
    }

    fn sw_breakpoint(&mut self) -> Option<gdbstub::target::ext::breakpoints::SwBreakpointOps<Self>> {
        Some(self)
    }

    fn hw_breakpoint(&mut self) -> Option<gdbstub::target::ext::breakpoints::HwBreakpointOps<Self>> {
        Some(self)
    }

    fn hw_watchpoint(&mut self) -> Option<gdbstub::target::ext::breakpoints::HwWatchpointOps<Self>> {
        Some(self)
    }

    fn monitor_cmd(&mut self) -> Option<gdbstub::target::ext::monitor_cmd::MonitorCmdOps<Self>> {
        None
    }

    fn extended_mode(&mut self) -> Option<gdbstub::target::ext::extended_mode::ExtendedModeOps<Self>> {
        None
    }

    fn section_offsets(&mut self) -> Option<gdbstub::target::ext::section_offsets::SectionOffsetsOps<Self>> {
        None
    }

    fn target_description_xml_override(
        &mut self,
    ) -> Option<gdbstub::target::ext::target_description_xml_override::TargetDescriptionXmlOverrideOps<Self>> {
        None
    }
}

impl SingleThreadOps for EmuState {
    fn resume(
        &mut self,
        action: gdbstub::target::ext::base::ResumeAction,
        check_gdb_interrupt: &mut dyn FnMut() -> bool,
    ) -> Result<StopReason<u32>, Self::Error> {
        match action {
            ResumeAction::Continue => {
                let mut cycles = 0;
                self.emu.clear_halt();
                println!("Continuing!");
                loop {
                    if self.emu.halt_requested() {
                        println!("Halt hit!");
                        return Ok(StopReason::SwBreak);
                    }
                    if let Err(e) = emu_loop_step(self) {
                        println!("EmuThread: Encountered error: {:?}, exiting...", e);
                    };
                    cycles += 1;
                    if cycles % 1024 == 0 && check_gdb_interrupt() {
                        println!("GDB Interrupt hit!");
                        return Ok(StopReason::GdbInterrupt);
                    }
                }
            }
            _ => Err("cannot resume")
            
        }
    }

    fn read_registers(
        &mut self,
        regs: &mut gdbstub::arch::mips::reg::MipsCoreRegs<u32>,
    ) -> gdbstub::target::TargetResult<(), Self> {
       
       
        for i in 0..31 {
            regs.r[i] = self.emu.read_gen_reg(i);
        };

        regs.hi = self.emu.r3000.hi;
        regs.lo = self.emu.r3000.lo;
        regs.pc = self.emu.r3000.pc;

        regs.cp0.status = self.emu.r3000.cop0.read_reg(12);
        regs.cp0.cause = self.emu.r3000.cop0.read_reg(13);
        regs.cp0.badvaddr = self.emu.r3000.cop0.read_reg(14);

        Ok(())
    }

    fn write_registers(&mut self, regs: &gdbstub::arch::mips::reg::MipsCoreRegs<u32>)
        -> gdbstub::target::TargetResult<(), Self> {
        
        for i in 0..31 {
            self.emu.set_gen_reg(i, regs.r[i]);
        };

        self.emu.r3000.hi = regs.hi;
        self.emu.r3000.lo = regs.lo;
        self.emu.r3000.pc = regs.pc;

        self.emu.r3000.cop0.write_reg(12, regs.cp0.status);
        self.emu.r3000.cop0.write_reg(13, regs.cp0.cause);
        self.emu.r3000.cop0.write_reg(14, regs.cp0.badvaddr);

        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: u32,
        data: &mut [u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        for i in 0..data.len() {
            data[i] = self.emu.r3000.read_bus_byte(start_addr + i as u32);
        }
        Ok(())
    }

    fn write_addrs(
        &mut self,
        start_addr: u32,
        data: &[u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        for i in 0..data.len() {
            self.emu.r3000.main_bus.write_byte(start_addr + i as u32, data[i]);
        }

        Ok(())
    }
}

impl SwBreakpoint for EmuState {
    fn add_sw_breakpoint(&mut self, addr: u32) -> gdbstub::target::TargetResult<bool, Self> {
        self.emu.add_sw_breakpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }

    fn remove_sw_breakpoint(
        &mut self,
        addr: u32,
    ) -> gdbstub::target::TargetResult<bool, Self> {
        self.emu.remove_sw_breakpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }
}

impl HwBreakpoint for EmuState {
    fn add_hw_breakpoint(&mut self, addr: u32) -> TargetResult<bool, Self> {
        println!("Set breakpoint");
        self.emu.add_sw_breakpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }

    fn remove_hw_breakpoint(
        &mut self,
        addr: u32,
    ) -> TargetResult<bool, Self> {
        self.emu.remove_sw_breakpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }
}

impl HwWatchpoint for EmuState {
    fn add_hw_watchpoint(
        &mut self,
        addr: u32,
        kind: gdbstub::target::ext::breakpoints::WatchKind,
    ) -> TargetResult<bool, Self> {
        println!("Trying to add watchpoint...");
        self.emu.add_watchpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }

    fn remove_hw_watchpoint(
        &mut self,
        addr: u32,
        kind: gdbstub::target::ext::breakpoints::WatchKind,
    ) -> TargetResult<bool, Self> {
        self.emu.remove_watchpoint(addr);
        TargetResult::<bool, Self>::Ok(true)
    }
}