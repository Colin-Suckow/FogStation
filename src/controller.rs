use bit_field::BitField;

use crate::cpu::{InterruptSource, R3000};

pub(super) const JOY_DATA: u32 = 0x1F801040;
pub(super) const JOY_STAT: u32 = 0x1F801044;
pub(super) const JOY_MODE: u32 = 0x1F801048;
pub(super) const JOY_CTRL: u32 = 0x1F80104A;
pub(super) const JOY_BAUD: u32 = 0x1F80104E;

const DEFAULT_JOY_BAUD: u16 = 0x88;

pub(super) struct Controllers {
    joy_ctrl: u16,
    joy_baud: u16,
    joy_mode: u16,
}

impl Controllers {
    pub(super) fn new() -> Self {
        Self {
            joy_ctrl: 0,
            joy_mode: 0,
            joy_baud: DEFAULT_JOY_BAUD,
        }
    }

    pub(super) fn write_half_word(&mut self, addr: u32, val: u16) {
        match addr {
            JOY_CTRL => self.write_joy_ctrl(val),
            JOY_BAUD => self.write_joy_baud(val),
            JOY_MODE => self.write_joy_mode(val),
            _ => println!("CONTROLLER: Unknown half word write! Addr {:#X} val: {:#X}", addr, val)
        };
    }

    fn write_joy_mode(&mut self, val: u16) {
        self.joy_mode = val;
        println!("JOY_MODE {:#X}", self.joy_mode);
    }

    fn write_joy_baud(&mut self, val: u16) {
        self.joy_baud = val;
    }

    fn write_joy_ctrl(&mut self, val: u16) {
        
        if val.get_bit(4) {
            self.acknowledge();
        }

        if val.get_bit(6) {
            self.reset();
        }

        self.joy_ctrl = val & !0x50; // Ignore the reset and ack bits
        println!("JOY_CTRL {:#X}", self.joy_ctrl);
    }

    fn reset(&mut self) {
        self.joy_ctrl = 0;
    }

    fn acknowledge(&mut self) {
        todo!()
    }
}
