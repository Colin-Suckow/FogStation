use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;
use crate::{CpuCycles, Scheduler};
use crate::scheduler::{EventHandle, GpuCycles, HBlankCycles};
use crate::ScheduleTarget::{TimerOverflow, TimerTarget};

#[derive(PartialEq, Debug)]
enum Cause {
    Full,
    Target,
}

enum Source {
    Sys,
    SysDiv,
    Dot,
    HBlank
}

pub struct Timer {
    timer_number: usize,
    pub value: u32,
    pub target: u32,
    pub mode: u32,
    irq_fired: bool,
    target_cpu_cycles: u32,
    overflow_cpu_cycles: u32,
    overflow_event_handle: Option<EventHandle>
}

impl Timer {
    pub fn new(num: usize) -> Self {
        Self {
            timer_number: num,
            value: 0,
            target: 0,
            mode: 0,
            irq_fired: false,
            target_cpu_cycles: 0,
            overflow_cpu_cycles: 0,
            overflow_event_handle: None
        }
    }

    pub fn read_mode(&mut self) -> u32 {
        let mode = self.mode;
        self.mode.set_bit(11, false);
        self.mode.set_bit(12, false);
        mode
    }

    pub fn write_mode(&mut self, value: u32, scheduler: &mut Scheduler) {
        self.mode = value;
        self.mode.set_bit(10, true);
        self.value = 0;
        self.irq_fired = false;
        self.reschedule_events(scheduler);
    }

    fn read_value(&self, scheduler: &mut Scheduler) -> u16 {
        if let Some(handle) = &self.overflow_event_handle {
            if let Some(cycles_remaining) = scheduler.cycles_remaining(handle) {
                0xFFFF - (((cycles_remaining.0 as f32) / (self.overflow_cpu_cycles as f32)) * 0xFFFF as f32) as u16
            } else {
                0
            }
        } else {
            0
        }
    }

    fn reschedule_events(&mut self, scheduler: &mut Scheduler) {
        // Get rid of old timer events
        scheduler.invalidate_exact_events_of_target(TimerTarget(self.timer_number as u32));
        scheduler.invalidate_exact_events_of_target(TimerOverflow(self.timer_number as u32));

        // Schedule events for timer expiration
        // Event when target reached
        if self.target != 0 {
            let target_cycles = self.calculate_cycles(self.target);
            self.target_cpu_cycles = target_cycles.0;
            scheduler.schedule_event(TimerTarget(self.timer_number as u32), target_cycles);
        }

        // Event when overflow reached
        let overflow_cycles = self.calculate_cycles(0xFFFF - self.value);
        self.overflow_cpu_cycles = overflow_cycles.0;
        self.overflow_event_handle = Some(scheduler.schedule_event(TimerOverflow(self.timer_number as u32), overflow_cycles));
    }

    fn source(&self) -> Source {
        match self.timer_number {
            0 => {
                match self.mode.get_bits(8..=9) {
                    0 | 2 => Source::Sys,
                    _ => Source::Dot
                }
            },
            1 => {
                match self.mode.get_bits(8..=9) {
                    0 | 2 => Source::Sys,
                    _ => Source::HBlank
                }
            },
            2 => {
                match self.mode.get_bits(8..=9) {
                     0 | 1 => Source::Sys,
                    _ => Source::SysDiv
                }
            },
            _ => unreachable!()
        }
    }

    fn irq_source(&self) -> InterruptSource {
        match self.timer_number {
            0 => InterruptSource::TMR0,
            1 => InterruptSource::TMR1,
            2 => InterruptSource::TMR2,
            _ => panic!("Invalid timer source"),
        }
    }

    fn calculate_cycles(&self, cycle_count: u32) -> CpuCycles {
        match self.source() {
            Source::Sys => CpuCycles(cycle_count).into(),
            Source::SysDiv => CpuCycles(cycle_count * 8).into(),
            Source::Dot => GpuCycles(cycle_count).into(),
            Source::HBlank => HBlankCycles(cycle_count).into(),
        }
    }
}

pub struct TimerState {
    pub timer_0: Timer,
    pub timer_1: Timer,
    pub timer_2: Timer,
}

impl TimerState {
    pub fn new() -> Self {
        Self {
            timer_0: Timer::new(0),
            timer_1: Timer::new(1),
            timer_2: Timer::new(2),
        }
    }

    pub fn timer_overflow_event(&mut self, cpu: &mut R3000, scheduler: &mut Scheduler, timer_num: u32) {
        let timer = match timer_num {
            0 => &mut self.timer_0,
            1 => &mut self.timer_1,
            2 => &mut self.timer_2,
            _ => panic!("Unknown timer num!")
        };

        timer.mode.set_bit(12, true);

        if !timer.irq_fired && timer.mode.get_bit(5) {
            // If in one shot mode, disable further IRQs
            if !timer.mode.get_bit(6) {
                timer.irq_fired = true;
            }
            cpu.fire_external_interrupt(timer.irq_source());
        }

        timer.value = 0;

        let overflow_cycles: CpuCycles = timer.calculate_cycles(0xFFFF);
        timer.overflow_cpu_cycles = overflow_cycles.0;
        timer.overflow_event_handle = Some(scheduler.schedule_event(TimerOverflow(timer_num), overflow_cycles));

    }

    pub fn timer_target_event(&mut self, cpu: &mut R3000, scheduler: &mut Scheduler, timer_num: u32) {
        let timer = match timer_num {
            0 => &mut self.timer_0,
            1 => &mut self.timer_1,
            2 => &mut self.timer_2,
            _ => panic!("Unknown timer num!")
        };

        timer.mode.set_bit(11, true);

        if !timer.irq_fired && timer.mode.get_bit(4) {
            // If in one shot mode, disable further IRQs
            if !timer.mode.get_bit(6) {
                timer.irq_fired = true;
            }
            cpu.fire_external_interrupt(timer.irq_source());
        }

        timer.value = timer.target;
        if timer.mode.get_bit(3) {
            timer.value = 0;

            // Reschedule the overflow counter
            let overflow_cycles = timer.calculate_cycles(0xFFFF);
            scheduler.invalidate_exact_events_of_target(TimerOverflow(timer_num));
            timer.overflow_cpu_cycles = overflow_cycles.0;
            timer.overflow_event_handle = Some(scheduler.schedule_event(TimerOverflow(timer_num), overflow_cycles));

        }

        let cycles = if timer.value == timer.target {
            0xFFFF - timer.value + timer.target
        } else {
            timer.target
        };

        let target_cycles: CpuCycles = timer.calculate_cycles(cycles);
        timer.target_cpu_cycles = target_cycles.0;
        scheduler.schedule_event(TimerTarget(timer_num), target_cycles);

    }

    pub fn read_word(&mut self, addr: u32, scheduler: &mut Scheduler) -> u32 {
        let val = match addr {
            0x1F801100 => self.timer_0.read_value(scheduler) as u32,
            0x1F801104 => self.timer_0.read_mode(),
            0x1F801108 => self.timer_0.target,

            0x1F801110 => self.timer_1.read_value(scheduler) as u32,
            0x1F801114 => self.timer_1.read_mode(),
            0x1F801118 => self.timer_1.target,

            0x1F801120 => self.timer_2.read_value(scheduler) as u32,
            0x1F801124 => self.timer_2.read_mode(),
            0x1F801128 => self.timer_2.target,
            _ => {
                println!("Unknown timer address {:#X}. Returning 0", addr);
                0
            }
        };
        //println!("Timer read addr {:#X} val {:#X}", addr, val);
        val
    }

    pub fn write_word(&mut self, addr: u32, val: u32, scheduler: &mut Scheduler) {
        println!("Timer write word addr {:#X} val {:#X}", addr, val);
        match addr {
            0x1F801100 => {
                self.timer_0.value = val;
                self.timer_0.reschedule_events(scheduler);
            },
            0x1F801104 => self.timer_0.write_mode(val, scheduler),
            0x1F801108 => {
                self.timer_0.target = val;
                self.timer_0.reschedule_events(scheduler);
            },

            0x1F801110 => {
                self.timer_1.value = val;
                self.timer_1.reschedule_events(scheduler);
            },
            0x1F801114 => self.timer_1.write_mode(val, scheduler),
            0x1F801118 => {
                self.timer_1.target = val;
                self.timer_1.reschedule_events(scheduler);
            }

            0x1F801120 => {
                self.timer_2.value = val;
                self.timer_2.reschedule_events(scheduler);
            },
            0x1F801124 => self.timer_2.write_mode(val, scheduler),
            0x1F801128 => {
                self.timer_2.target = val;
                self.timer_2.reschedule_events(scheduler);
            }
            _ => println!("Unknown timer address"),
        }
    }

    pub fn read_half_word(&mut self, addr: u32, scheduler: &mut Scheduler) -> u16 {
        //println!("Reading halfword timer addr {:#X}", addr);

        match addr {
            0x1F801100 => self.timer_0.read_value(scheduler) as u16,
            //0x1F801102 => (self.timer_0.read_value(scheduler) >> 16) as u16,

            0x1F801104 => self.timer_0.read_mode() as u16,
            0x1F801106 => (self.timer_0.read_mode() >> 16) as u16,

            0x1F801108 => self.timer_0.target as u16,
            0x1F80110A => (self.timer_0.target >> 16) as u16,

            0x1F801110 => self.timer_1.read_value(scheduler) as u16,
            //0x1F801112 => (self.timer_1.read_value(scheduler) >> 16) as u16,

            0x1F801114 => self.timer_1.read_mode() as u16,
            0x1F801116 => (self.timer_1.read_mode() >> 16) as u16,

            0x1F801118 => self.timer_1.target as u16,
            0x1F80111A => (self.timer_1.target >> 16) as u16,

            0x1F801120 => self.timer_2.read_value(scheduler) as u16,
            //0x1F801122 => (self.timer_2.read_value(scheduler) >> 16) as u16,

            0x1F801124 => self.timer_2.read_mode() as u16,
            0x1F801126 => (self.timer_2.read_mode() >> 16) as u16,

            0x1F801128 => self.timer_2.target as u16,
            0x1F80112A => (self.timer_2.target >> 16) as u16,
            _ => {
                println!("Unknown timer address. Returning 0");
                0
            }
        }
    }

    pub fn write_half_word(&mut self, addr: u32, value: u16, scheduler: &mut Scheduler,) {
        //println!("Tried to write timer half addr {:#X} val {:#X}", addr, value);
        match addr {
            0x1F801100 => self.timer_0.value &= (value as u32) & 0xFFFF0000,
            0x1F801102 => self.timer_0.value &= ((value as u32) << 16) & 0xFFFF,

            0x1F801104 => self.timer_0.write_mode(value as u32, scheduler),
            //0x1F801106 => self.timer_0.read_mode(value as u32),
            0x1F801108 => self.timer_0.target = value as u32,
            0x1F80110A => self.timer_0.target &= (value as u32) << 16,

            0x1F801110 => self.timer_1.value &= value as u32,
            0x1F801112 => self.timer_1.value &= (value as u32) << 16,

            0x1F801114 => self.timer_1.write_mode(value as u32, scheduler),
            //0x1F801116 => self.timer_1.read_mode(value as u32),
            0x1F801118 => self.timer_1.target = value as u32,
            //0x1F80111A => (self.timer_1.target >> 16) as u16,
            0x1F801120 => self.timer_2.value = value as u32,
            //0x1F801122 => (self.timer_2.value >> 16) as u16,
            0x1F801124 => self.timer_2.write_mode(value as u32, scheduler),
            //0x1F801126 => (self.timer_2.read_mode() >> 16) as u16,
            0x1F801128 => self.timer_2.target = value as u32,
            //0x1F80112A => (self.timer_2.target >> 16) as u16,
            _ => {
                println!("Half wrote unknown timer address {:#X}", addr)
            }
        }
    }
}
