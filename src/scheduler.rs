use std::mem::discriminant;
use crate::{InterruptSource, MainBus, PSXEmu, R3000};
use crate::cdrom::cdpacket_event;
use crate::controller::controller_delay_event;
use crate::ScheduleTarget::{CDPacket, GPUhblank, TimerOverflow, TimerTarget};

#[derive(PartialEq, Copy, Clone)]
pub enum ScheduleTarget {
    GPUhblank,
    ControllerIRQ,
    TimerTarget(u32),
    TimerOverflow(u32),
    CDPacket(u32),
    CDIrq,
}

pub struct CpuCycles(pub u32);
pub struct GpuCycles(pub u32);
pub struct SysCycles(pub u32);
pub struct HBlankCycles(pub u32);

impl From<SysCycles> for CpuCycles {
    fn from(sys_cycles: SysCycles) -> Self {
        CpuCycles(sys_cycles.0 / 2)
    }
}

// GPU and HBlank cycles are a bit wrong because we don't have access to the real gpu timing values

impl From<GpuCycles> for CpuCycles {
    fn from(gpu_cycles: GpuCycles) -> Self {
        CpuCycles(gpu_cycles.0 * 7 / 11)
    }
}

impl From<HBlankCycles> for CpuCycles {
    fn from(h_cycles: HBlankCycles) -> Self {
        GpuCycles(h_cycles.0 * 3413).into()
    }
}

#[derive(Copy, Clone)]
struct PendingEvent {
    target: ScheduleTarget,
    cycles: u32,
}

pub struct Scheduler {
    pending_events: Vec<PendingEvent>
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            pending_events: Vec::new()
        }
    }

    pub fn schedule_event(&mut self, target: ScheduleTarget, cycles: CpuCycles) {
        self.pending_events.push(PendingEvent {target: target, cycles: cycles.0});
    }

    pub fn run_cycle(&mut self, emu: &mut R3000, main_bus: &mut MainBus) {
        let events_to_process = self.pending_events.to_vec();
        for event in &events_to_process {
            if event.cycles <= 0 {
               self.execute(&event.target, emu, main_bus)
            }
        }


        self.pending_events.retain_mut(|event| {
            if event.cycles > 0 {
                event.cycles -= 1;
                true
            } else {
                false
            }
        });
    }

    pub fn invalidate_all_events_of_target(&mut self, target: ScheduleTarget) {
        self.pending_events.retain(|event| {
            discriminant(&event.target) != discriminant(&target)
        });
    }

    fn execute(&mut self, target: &ScheduleTarget, cpu: &mut R3000, main_bus: &mut MainBus) {
        match target {
            GPUhblank => {
                main_bus.gpu.hblank_event(cpu, self);
            },
            TimerOverflow(timer_num) => {
                main_bus.timers.timer_overflow_event(cpu, self, *timer_num);

            },
            TimerTarget(timer_num) => {
                main_bus.timers.timer_target_event(cpu, self, *timer_num);
            },
            CDPacket(id) => {
                cdpacket_event(cpu, main_bus, self, *id);
            },
            ScheduleTarget::CDIrq => {
                cpu.fire_external_interrupt(InterruptSource::CDROM);
            },
            ScheduleTarget::ControllerIRQ => {
                controller_delay_event(cpu, &mut main_bus.controllers);
            }
            _ => {}
        }
    }
}
