use crate::{MainBus, PSXEmu, R3000};
use crate::ScheduleTarget::GPUhblank;

pub enum ScheduleTarget {
    GPUhblank,
    ControllerIRQ
}

pub struct CpuCycles(pub u32);
pub struct GpuCycles(pub u32);
pub struct SysCycles(pub u32);

impl From<SysCycles> for CpuCycles {
    fn from(sys_cycles: SysCycles) -> Self {
        CpuCycles(sys_cycles.0 / 2)
    }
}

impl From<GpuCycles> for CpuCycles {
    fn from(gpu_cycles: GpuCycles) -> Self {
        CpuCycles(gpu_cycles.0 * 7 / 11)
    }
}

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
        let mut new_events = Vec::new();
        for event in &self.pending_events {
            if event.cycles <= 0 {
                if let Some(new_event) = self.execute(&event.target, emu, main_bus) {
                    new_events.push(new_event);
                }
            }
        }

        self.pending_events.extend(new_events);

        self.pending_events.retain_mut(|event| {
            if event.cycles > 0 {
                event.cycles -= 1;
                true
            } else {
                false
            }
        });
    }

    fn execute(&self, target: &ScheduleTarget, cpu: &mut R3000, main_bus: &mut MainBus) -> Option<PendingEvent> {
        match target {
            GPUhblank => {
                if let Some(cycles) = main_bus.gpu.schedule_complete() {
                    main_bus.timers.update_h_blank(cpu);
                    Some(PendingEvent{target: GPUhblank, cycles: cycles.0})
                } else {
                    None
                }
            },
            _ => None
        }
    }
}
