use std::collections::VecDeque;

use bit_field::BitField;

pub(super) const JOY_DATA: u32 = 0x1F801040;
pub(super) const JOY_STAT: u32 = 0x1F801044;
pub(super) const JOY_MODE: u32 = 0x1F801048;
pub(super) const JOY_CTRL: u32 = 0x1F80104A;
pub(super) const JOY_BAUD: u32 = 0x1F80104E;

const DEFAULT_JOY_BAUD: u16 = 0x88;

const MEMORY_CARD_SELECT_BYTE: u8 = 0x81;
const CONTROLER_SELECT_BYTE: u8 = 0x1;

#[derive(Debug, PartialEq)]
enum Slot {
    MemoryCard,
    Controller,
}

#[derive(Debug, PartialEq)]
enum TXstate {
    Disabled,
    Ready,
    Transfering{slot: Slot, step: usize},
}

pub(super) struct Controllers {
    joy_ctrl: u16,
    joy_baud: u16,
    joy_mode: u16,

    tx_state: TXstate,
    rx_buf: VecDeque<u8>,

    pub(super) pending_irq: bool,
}

impl Controllers {
    pub(super) fn new() -> Self {
        Self {
            joy_ctrl: 0,
            joy_mode: 0,
            joy_baud: DEFAULT_JOY_BAUD,

            tx_state: TXstate::Disabled,
            rx_buf: VecDeque::new(),

            pending_irq: false,
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

    pub(super) fn read_half_word(&mut self, addr: u32) -> u16 {
        match addr {
            JOY_STAT => self.read_joy_stat(),
            JOY_CTRL => self.read_joy_ctrl(),
            _ =>  {
                println!("CONTROLLER: Unknown half word read! Addr {:#X}", addr);
                0
            }
        }
    }

    pub(super) fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            JOY_DATA => self.read_joy_data() as u8,
            _ => {
                println!("CONTROLLER: Unknown byte read! Addr {:#X}", addr);
                0
            }
        }
    }


    pub(super) fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            JOY_DATA => self.write_joy_data(val),
            _ => println!("CONTROLLER: Unknown byte write! Addr {:#X} val: {:#X}", addr, val)
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
        
        if val.get_bit(0) && self.tx_state == TXstate::Disabled {
            println!("TX Enabled!");
            self.tx_state = TXstate::Ready;
        } else {
            println!("TX Disabled!");
            self.tx_state = TXstate::Disabled;
        }

        if val.get_bit(4) {
            self.acknowledge();
        }

        if val.get_bit(6) {
            self.reset();
        }

        self.joy_ctrl = val & !0x50; // Ignore the reset and ack bits
        println!("JOY_CTRL {:#X}", self.joy_ctrl);
    }

    fn write_joy_data(&mut self, val: u8) {
        println!("Joy data written {:#X}", val);
        match &mut self.tx_state {
            TXstate::Disabled => {
                println!("CONTROLLER: Tried to write JOY_DATA while TX is disabled!");
            }
            TXstate::Ready => {
                let slot = if val == CONTROLER_SELECT_BYTE {
                    Slot::Controller
                } else {
                    Slot::MemoryCard
                };
                self.push_rx_buf(0);
                self.tx_state = TXstate::Transfering {
                    slot: slot,
                    step: 0,
                };
                self.pending_irq = true;
            }
            TXstate::Transfering { slot, step } => {
                println!("Transfering");
            }
        }
    }

    fn read_joy_stat(&mut self) -> u16 {
        let mut val: u16 = 0;


        if self.tx_state != TXstate::Disabled {
            val |= 0x1;
        };

        if self.tx_state == TXstate::Ready {
            val |= 0x4;
        }

        if self.pending_irq {
            val |= 0x200;
        }

        if !self.rx_buf.is_empty() {
            val |= 2;
        }

        println!("Reading JOY_STAT {:#X}", val);

        val
    }

    fn read_joy_ctrl(&mut self) -> u16 {
        //println!("Reading joy_ctrl {:#X}", self.joy_ctrl);
        self.joy_ctrl
    }

    fn read_joy_data(&mut self) -> u8 {
       self.pop_rx_buf()
    }

    fn reset(&mut self) {
        self.write_joy_ctrl(0);
        self.rx_buf.clear();
    }

    fn acknowledge(&mut self) {
       self.pending_irq = false;
    }

    fn push_rx_buf(&mut self, val: u8) {
        self.rx_buf.push_back(val);
    }

    fn pop_rx_buf(&mut self) -> u8 {
        match self.rx_buf.pop_front() {
            Some(val) => val,
            _ => 0
        }
    }
}
