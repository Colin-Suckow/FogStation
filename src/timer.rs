use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;
use crate::{CpuCycles, Scheduler};
use crate::scheduler::{GpuCycles, HBlankCycles, SysCycles};
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
}

impl Timer {
    pub fn new(num: usize) -> Self {
        Self {
            timer_number: num,
            value: 0,
            target: 0,
            mode: 0,
        }
    }

    pub fn increment(&mut self, cpu: &mut R3000) {
        // Crash Bandicoot hack to get it running faster
        // if self.timer_number == 2 {
        //    self.value += 2;
        // } else {
        self.value += 1;
        //}

        if self.value >= self.target {
            self.trigger(cpu, Cause::Target);
        }

        if self.value == 0xFFFF || self.value == 0xFFFF + 1 {
            self.trigger(cpu, Cause::Full);
        }

        match self.mode.get_bit(3) {
            true => {
                if self.value as u16 >= self.target as u16 {
                    self.value = 0;
                }
            }
            false => {
                if self.value as u16 >= 0xFFFF {
                    self.value = 0;
                }
            }
        }
    }

    fn trigger(&mut self, cpu: &mut R3000, cause: Cause) {
        match cause {
            Cause::Full => self.mode.set_bit(12, true),
            Cause::Target => self.mode.set_bit(11, true),
        };

        // println!("Timer {} triggered because of {:?}", self.timer_number, cause);
        // println!("Timer {} mode is {:#X}", self.timer_number, self.mode);

        if (self.mode.get_bit(4) && cause == Cause::Target)
            || (self.mode.get_bit(5) && cause == Cause::Full)
        {
            //println!("Firing timer interrupt");
            let source = match self.timer_number {
                0 => InterruptSource::TMR0,
                1 => InterruptSource::TMR1,
                2 => InterruptSource::TMR2,
                _ => panic!("Invalid timer source"),
            };
            cpu.fire_external_interrupt(source);
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

        // Get rid of old timer events
        scheduler.invalidate_all_events_of_target(TimerTarget(0));
        scheduler.invalidate_all_events_of_target(TimerOverflow(0));

        // Schedule events for timer expiration
        // Event when target reached
        if self.target != 0 {
            let target_cycles: CpuCycles = match self.source() {
                Source::Sys => SysCycles(self.target).into(),
                Source::SysDiv => SysCycles(self.target * 8).into(),
                Source::Dot => GpuCycles(self.target).into(),
                Source::HBlank => HBlankCycles(self.target).into(),
            };
            scheduler.schedule_event(TimerTarget(self.timer_number as u32), target_cycles);
        }

        // Event when overflow reached
        let overflow_cycles: CpuCycles = match self.source() {
            Source::Sys => SysCycles(0xFFFF).into(),
            Source::SysDiv => SysCycles(0xFFFF * 8).into(),
            Source::Dot => GpuCycles(0xFFFF).into(),
            Source::HBlank => HBlankCycles(0xFFFF).into(),
        };

        scheduler.schedule_event(TimerOverflow(self.timer_number as u32), overflow_cycles);

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

        if timer.mode.get_bit(5) {
            cpu.fire_external_interrupt(timer.irq_source());
        }
    }

    pub fn timer_target_event(&mut self, cpu: &mut R3000, scheduler: &mut Scheduler, timer_num: u32) {
        let timer = match timer_num {
            0 => &mut self.timer_0,
            1 => &mut self.timer_1,
            2 => &mut self.timer_2,
            _ => panic!("Unknown timer num!")
        };

        if timer.mode.get_bit(4) {
            cpu.fire_external_interrupt(timer.irq_source());
        }
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        let val = match addr {
            0x1F801100 => self.timer_0.value,
            0x1F801104 => self.timer_0.read_mode(),
            0x1F801108 => self.timer_0.target,

            0x1F801110 => self.timer_1.value,
            0x1F801114 => self.timer_1.read_mode(),
            0x1F801118 => self.timer_1.target,

            0x1F801120 => self.timer_2.value,
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
            0x1F801100 => self.timer_0.value = val,
            0x1F801104 => self.timer_0.write_mode(val, scheduler),
            0x1F801108 => self.timer_0.target = val,

            0x1F801110 => self.timer_1.value = val,
            0x1F801114 => self.timer_1.write_mode(val, scheduler),
            0x1F801118 => self.timer_1.target = val,

            0x1F801120 => self.timer_2.value = val,
            0x1F801124 => self.timer_2.write_mode(val, scheduler),
            0x1F801128 => self.timer_2.target = val,
            _ => println!("Unknown timer address"),
        }
    }

    pub fn read_half_word(&mut self, addr: u32) -> u16 {
        //println!("Reading halfword timer addr {:#X}", addr);

        match addr {
            0x1F801100 => self.timer_0.value as u16,
            0x1F801102 => (self.timer_0.value >> 16) as u16,

            0x1F801104 => self.timer_0.read_mode() as u16,
            0x1F801106 => (self.timer_0.read_mode() >> 16) as u16,

            0x1F801108 => self.timer_0.target as u16,
            0x1F80110A => (self.timer_0.target >> 16) as u16,

            0x1F801110 => self.timer_1.value as u16,
            0x1F801112 => (self.timer_1.value >> 16) as u16,

            0x1F801114 => self.timer_1.read_mode() as u16,
            0x1F801116 => (self.timer_1.read_mode() >> 16) as u16,

            0x1F801118 => self.timer_1.target as u16,
            0x1F80111A => (self.timer_1.target >> 16) as u16,

            0x1F801120 => self.timer_2.value as u16,
            0x1F801122 => (self.timer_2.value >> 16) as u16,

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
