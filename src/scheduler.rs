use crate::cdrom::cdpacket_event;
use crate::controller::controller_delay_event;
use crate::ScheduleTarget::{CDPacket, GpuHblank, TimerOverflow, TimerTarget};
use crate::{InterruptSource, MainBus, PSXEmu, R3000};
use std::array;
use std::mem::discriminant;

#[derive(PartialEq, Copy, Clone)]
pub enum ScheduleTarget {
    GpuHblank,
    GpuVblank,
    ControllerIRQ,
    TimerTarget(u32),
    TimerOverflow(u32),
    CDPacket(u32),
    CDIrq,
}

pub struct CpuCycles(pub u32);
pub struct GpuCycles(pub u32);
pub struct HBlankCycles(pub u32);

// GPU and HBlank cycles are a bit wrong because we don't have access to the real gpu timing values

impl From<GpuCycles> for CpuCycles {
    fn from(gpu_cycles: GpuCycles) -> Self {
        CpuCycles((gpu_cycles.0 as f32 * 3.2) as u32)
    }
}

impl From<HBlankCycles> for CpuCycles {
    fn from(h_cycles: HBlankCycles) -> Self {
        GpuCycles(h_cycles.0 * 853).into()
    }
}

#[derive(Copy, Clone)]
struct PendingEvent {
    id: u32,
    target: ScheduleTarget,
    cycles: u32,
    complete: bool,
}

pub struct EventHandle(u32);

const EVENT_SLOTS: usize = 11;

pub struct Scheduler {
    pending_events: [PendingEvent; EVENT_SLOTS],
    next_id: u32,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            pending_events: [PendingEvent {
                id: 0,
                target: GpuHblank,
                cycles: 0,
                complete: true,
            }; EVENT_SLOTS],
            next_id: 0,
        }
    }

    pub fn schedule_event(&mut self, target: ScheduleTarget, cycles: CpuCycles) -> EventHandle {
        let id = self.next_id();
        for i in 0..EVENT_SLOTS {
            if self.pending_events[i].complete {
                self.pending_events[i] = PendingEvent {
                    id,
                    target: target,
                    cycles: cycles.0,
                    complete: false,
                };
                return EventHandle(id);
            }
        }
        // If we made it throug the loop, then there are no open event slots
        panic!("Unable to find an open event slot!");
    }

    pub fn run_cycle(&mut self, emu: &mut R3000, main_bus: &mut MainBus) {
        for i in 0..EVENT_SLOTS {
            if !self.pending_events[i].complete {
                if self.pending_events[i].cycles == 0 {
                    self.execute(&self.pending_events[i].target.clone(), emu, main_bus);
                    self.pending_events[i].complete = true;
                } else {
                    self.pending_events[i].cycles -= 1;
                }
            }
        }
    }

    pub fn invalidate_all_events_of_target(&mut self, target: ScheduleTarget) {
        for event in &mut self.pending_events {
            if discriminant(&event.target) == discriminant(&target) {
                event.complete = true;
            }
        }
    }

    pub fn invalidate_exact_events_of_target(&mut self, target: ScheduleTarget) {
        for event in &mut self.pending_events {
            if event.target == target {
                event.complete = true;
            }
        }
    }

    pub fn cycles_remaining(&self, handle: &EventHandle) -> Option<CpuCycles> {
        for event in &self.pending_events {
            if event.id == handle.0 {
                return Some(CpuCycles(event.cycles));
            }
        }
        None
    }

    fn execute(&mut self, target: &ScheduleTarget, cpu: &mut R3000, main_bus: &mut MainBus) {
        match target {
            GpuHblank => {
                main_bus.gpu.hblank_event(cpu, self);
            }
            TimerOverflow(timer_num) => {
                main_bus.timers.timer_overflow_event(cpu, self, *timer_num);
            }
            TimerTarget(timer_num) => {
                main_bus.timers.timer_target_event(cpu, self, *timer_num);
            }
            CDPacket(id) => {
                cdpacket_event(cpu, main_bus, self, *id);
            }
            ScheduleTarget::CDIrq => {
                cpu.fire_external_interrupt(InterruptSource::CDROM);
            }
            ScheduleTarget::ControllerIRQ => {
                controller_delay_event(cpu, &mut main_bus.controllers);
            }
            ScheduleTarget::GpuVblank => {
                main_bus.gpu.vblank_event(cpu, self);
            }
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
