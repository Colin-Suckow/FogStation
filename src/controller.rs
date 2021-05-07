use bit_field::BitField;

pub(super) const JOY_DATA: u32 = 0x1F801040;
pub(super) const JOY_STAT: u32 = 0x1F801044;
pub(super) const JOY_MODE: u32 = 0x1F801048;
pub(super) const JOY_CTRL: u32 = 0x1F80104A;
pub(super) const JOY_BAUD: u32 = 0x1F80104E;

pub(super) struct Controllers {
    joy_ctrl: u16,
}

impl Controllers {
    pub(super) fn new() -> Self {
        Self {
            joy_ctrl: 0,
        }
    }

    pub(super) fn write_half_word(&mut self, addr: u32, val: u16) {
        match addr {
            JOY_CTRL => self.write_joy_ctrl(val),
            JOY_BAUD => println!("CONTROLLER: Wrote JOY_BAUD with value {:#X}. Ignoring...", val),
            _ => println!("CONTROLLER: Unknown half word write! Addr {:#X} val: {:#X}", addr, val)
        };
    }

    fn write_joy_ctrl(&mut self, val: u16) {
        
        if val.get_bit(4) {
            self.acknowledge();
        }

        if val.get_bit(6) {
            self.reset();
        }

        self.joy_ctrl = val & !0x50; // Ignore the write only bits
        println!("Wrote {:#X} to JOY_CTRL!", val);
    }

    fn reset(&mut self) {
        self.joy_ctrl = 0;
    }

    fn acknowledge(&mut self) {
        todo!()
    }
}