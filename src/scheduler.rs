use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::mem::discriminant;
use crate::{InterruptSource, MainBus, PSXEmu, R3000};
use crate::cdrom::cdpacket_event;
use crate::controller::controller_delay_event;
use crate::ScheduleTarget::{CDPacket, GpuHblank, TimerOverflow, TimerTarget};

#[derive(PartialEq, Copy, Clone, Debug)]
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
    target_cycle: u32,
}

impl PartialEq for PendingEvent {
    fn eq(&self, other: &Self) -> bool {
        self.target_cycle == other.target_cycle
    }
}

impl Eq for PendingEvent {}

impl Ord for PendingEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.target_cycle.cmp(&other.target_cycle)
    }
}

impl PartialOrd for PendingEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.target_cycle.partial_cmp(&other.target_cycle)
    }
}

pub struct EventHandle(u32);

pub struct Scheduler {
    pending_events: BinaryHeap<Reverse<PendingEvent>>,
    next_id: u32,
    cycle_count: u32
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            pending_events: BinaryHeap::new(),
            next_id: 0,
            cycle_count: 0
        }
    }

    pub fn schedule_event(&mut self, target: ScheduleTarget, cycles: CpuCycles) -> EventHandle {
        let id = self.next_id();
        self.pending_events.push(Reverse(PendingEvent {id, target: target, target_cycle: self.cycle_count + cycles.0}));
        EventHandle(id)
    }

    pub fn run_cycle(&mut self, emu: &mut R3000, main_bus: &mut MainBus) {


        let peek_event = self.pending_events.peek();
        if peek_event.is_some() && peek_event.unwrap().0.target_cycle <= self.cycle_count {
            let event = self.pending_events.pop().unwrap();
            self.execute(&event.0.target.clone(), emu, main_bus);
        }

        self.cycle_count += 1;
       
    }

    pub fn invalidate_all_events_of_target(&mut self, target: ScheduleTarget) {
        self.pending_events.retain(|event| {
            discriminant(&event.0.target) != discriminant(&target)
        });
    }

    pub fn invalidate_exact_events_of_target(&mut self, target: ScheduleTarget) {
        self.pending_events.retain(|event| {
            event.0.target != target
        });
    }

    pub fn cycles_remaining(&self, handle: &EventHandle) -> Option<CpuCycles> {
        for event in &self.pending_events {
            if event.0.id == handle.0 {
                return Some(CpuCycles(self.cycle_count - event.0.target_cycle));
            }
        }
        None
    }

    fn execute(&mut self, target: &ScheduleTarget, cpu: &mut R3000, main_bus: &mut MainBus) {
        match target {
            GpuHblank => {
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
            },
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
