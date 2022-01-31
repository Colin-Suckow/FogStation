use crate::cpu::{InterruptSource, R3000};
use bit_field::BitField;

#[derive(PartialEq, Debug)]
enum Cause {
    Full,
    Target,
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
        
        if self.value == self.target {
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

    pub fn write_mode(&mut self, value: u32) {
        self.mode = value;
        self.mode.set_bit(10, true);
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

    pub fn update_sys_clock(&mut self, cpu: &mut R3000) {
        let mode0 = self.timer_0.mode.get_bits(8..=9);
        let mode1 = self.timer_1.mode.get_bits(8..=9);
        let mode2 = self.timer_2.mode.get_bits(8..=9);

        if mode0 == 0 || mode0 == 2 {
            self.timer_0.increment(cpu);
        }

        if mode1 == 0 || mode1 == 2 {
            self.timer_1.increment(cpu);
        }

        if mode2 == 0 || mode2 == 1 {
            self.timer_2.increment(cpu);
        }
    }

    pub fn update_dot_clock(&mut self, cpu: &mut R3000) {
        let mode0 = self.timer_0.mode.get_bits(8..=9);

        if mode0 == 1 || mode0 == 3 {
            self.timer_0.increment(cpu);
        }
    }

    pub fn update_h_blank(&mut self, cpu: &mut R3000) {
        let mode1 = self.timer_1.mode.get_bits(8..=9);

        if mode1 == 1 || mode1 == 3 {
            self.timer_1.increment(cpu);
        }
    }

    pub fn update_sys_div_8(&mut self, cpu: &mut R3000) {
        let mode2 = self.timer_2.mode.get_bits(8..=9);

        if mode2 == 2 || mode2 == 3 {
            self.timer_2.increment(cpu);
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

    pub fn write_word(&mut self, addr: u32, val: u32) {
        println!("Timer write word addr {:#X} val {:#X}", addr, val);
        match addr {
            0x1F801100 => self.timer_0.value = val,
            0x1F801104 => self.timer_0.write_mode(val),
            0x1F801108 => self.timer_0.target = val,

            0x1F801110 => self.timer_1.value = val,
            0x1F801114 => self.timer_1.write_mode(val),
            0x1F801118 => self.timer_1.target = val,

            0x1F801120 => self.timer_2.value = val,
            0x1F801124 => self.timer_2.write_mode(val),
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

    pub fn write_half_word(&mut self, addr: u32, value: u16) {
        //println!("Tried to write timer half addr {:#X} val {:#X}", addr, value);
        match addr {
            0x1F801100 => self.timer_0.value &= (value as u32) & 0xFFFF0000,
            0x1F801102 => self.timer_0.value &= ((value as u32) << 16) & 0xFFFF,

            0x1F801104 => self.timer_0.write_mode(value as u32),
            //0x1F801106 => self.timer_0.read_mode(value as u32),
            0x1F801108 => self.timer_0.target = value as u32,
            0x1F80110A => self.timer_0.target &= (value as u32) << 16,

            0x1F801110 => self.timer_1.value &= value as u32,
            0x1F801112 => self.timer_1.value &= (value as u32) << 16,

            0x1F801114 => self.timer_1.write_mode(value as u32),
            //0x1F801116 => self.timer_1.read_mode(value as u32),
            0x1F801118 => self.timer_1.target = value as u32,
            //0x1F80111A => (self.timer_1.target >> 16) as u16,
            0x1F801120 => self.timer_2.value = value as u32,
            //0x1F801122 => (self.timer_2.value >> 16) as u16,
            0x1F801124 => self.timer_2.write_mode(value as u32),
            //0x1F801126 => (self.timer_2.read_mode() >> 16) as u16,
            0x1F801128 => self.timer_2.target = value as u32,
            //0x1F80112A => (self.timer_2.target >> 16) as u16,
            _ => {
                println!("Half wrote unknown timer address {:#X}", addr)
            }
        }
    }
}
