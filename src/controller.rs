use std::collections::VecDeque;

use bit_field::BitField;
use log::{error, warn};

use crate::cpu::{InterruptSource, R3000};

pub(super) const JOY_DATA: u32 = 0x1F801040;
pub(super) const JOY_STAT: u32 = 0x1F801044;
pub(super) const JOY_MODE: u32 = 0x1F801048;
pub(super) const JOY_CTRL: u32 = 0x1F80104A;
pub(super) const JOY_BAUD: u32 = 0x1F80104E;

const DEFAULT_JOY_BAUD: u16 = 0x88;

#[allow(dead_code)]
const MEMORY_CARD_SELECT_BYTE: u8 = 0x81;
const CONTROLER_SELECT_BYTE: u8 = 0x1;

pub enum ControllerType {
    DigitalPad,
}

pub struct ButtonState {
    pub controller_type: ControllerType,

    pub button_x: bool,
    pub button_square: bool,
    pub button_triangle: bool,
    pub button_circle: bool,

    pub button_up: bool,
    pub button_down: bool,
    pub button_left: bool,
    pub button_right: bool,

    pub button_l1: bool,
    pub button_l2: bool,
    pub button_l3: bool,

    pub button_r1: bool,
    pub button_r2: bool,
    pub button_r3: bool,

    pub button_select: bool,
    pub button_start: bool,
}

impl ButtonState {
    pub fn new_digital_pad() -> Self {
        Self {
            controller_type: ControllerType::DigitalPad,

            button_x: false,
            button_square: false,
            button_triangle: false,
            button_circle: false,

            button_up: false,
            button_down: false,
            button_left: false,
            button_right: false,

            button_l1: false,
            button_l2: false,
            button_l3: false,

            button_r1: false,
            button_r2: false,
            button_r3: false,

            button_select: false,
            button_start: false,
        }
    }

    fn digital_low_byte(&self) -> u8 {
        let mut result = 0;

        result.set_bit(0, !self.button_select);
        result.set_bit(1, !self.button_l3);
        result.set_bit(2, !self.button_r3);
        result.set_bit(3, !self.button_start);
        result.set_bit(4, !self.button_up);
        result.set_bit(5, !self.button_right);
        result.set_bit(6, !self.button_down);
        result.set_bit(7, !self.button_left);

        result
    }

    fn digital_high_byte(&self) -> u8 {
        let mut result = 0;

        result.set_bit(0, !self.button_l2);
        result.set_bit(1, !self.button_r2);
        result.set_bit(2, !self.button_l1);
        result.set_bit(3, !self.button_r1);
        result.set_bit(4, !self.button_triangle);
        result.set_bit(5, !self.button_circle);
        result.set_bit(6, !self.button_x);
        result.set_bit(7, !self.button_square);

        result
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum Slot {
    MemoryCard,
    Controller,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum TXstate {
    Disabled,
    Ready,
    Transfering { slot: Slot, step: usize },
}

pub(super) struct Controllers {
    joy_ctrl: u16,
    joy_baud: u16,
    joy_mode: u16,
    irq_status: bool,

    tx_state: TXstate,
    rx_buf: VecDeque<u8>,

    pub(super) pending_irq: bool,
    irq_cycle_timer: usize,

    latest_button_state: ButtonState,
}

impl Controllers {
    pub(super) fn new() -> Self {
        Self {
            joy_ctrl: 0,
            joy_mode: 0,
            joy_baud: DEFAULT_JOY_BAUD,
            irq_status: false,

            tx_state: TXstate::Disabled,
            rx_buf: VecDeque::new(),

            pending_irq: false,
            irq_cycle_timer: 0,

            latest_button_state: ButtonState::new_digital_pad(),
        }
    }

    pub(super) fn update_button_state(&mut self, new_state: ButtonState) {
        self.latest_button_state = new_state;
    }

    pub(super) fn write_half_word(&mut self, addr: u32, val: u16) {
        match addr {
            JOY_CTRL => self.write_joy_ctrl(val),
            JOY_BAUD => self.write_joy_baud(val),
            JOY_MODE => self.write_joy_mode(val),
            _ => error!(
                "CONTROLLER: Unknown half word write! Addr {:#X} val: {:#X}",
                addr, val
            ),
        };
    }

    pub(super) fn read_half_word(&mut self, addr: u32) -> u16 {
        match addr {
            JOY_STAT => self.read_joy_stat(),
            JOY_CTRL => self.read_joy_ctrl(),
            _ => {
                error!("CONTROLLER: Unknown half word read! Addr {:#X}", addr);
                0
            }
        }
    }

    pub(super) fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            JOY_DATA => self.read_joy_data() as u8,
            _ => {
                error!("CONTROLLER: Unknown byte read! Addr {:#X}", addr);
                0
            }
        }
    }

    pub(super) fn write_byte(&mut self, addr: u32, val: u8) {
        match addr {
            JOY_DATA => self.write_joy_data(val),
            _ => error!(
                "CONTROLLER: Unknown byte write! Addr {:#X} val: {:#X}",
                addr, val
            ),
        };
    }

    fn write_joy_mode(&mut self, val: u16) {
        self.joy_mode = val;
        //println!("JOY_MODE {:#X}", self.joy_mode);
    }

    fn write_joy_baud(&mut self, val: u16) {
        self.joy_baud = val;
    }

    fn write_joy_ctrl(&mut self, val: u16) {
        //println!("JOY_CTRL {:#X}", val);

        if val.get_bit(0) && self.tx_state == TXstate::Disabled {
            //println!("TX Enabled!");
            self.tx_state = TXstate::Ready;
        }

        if !val.get_bit(0) {
            //println!("TX Disabled!");
            self.tx_state = TXstate::Disabled;
            // self.pending_irq = false;
            // self.irq_cycle_timer = 0;
        }

        if val.get_bit(4) {
            self.acknowledge();
        }

        if val.get_bit(6) {
            self.reset();
        }

        self.joy_ctrl = val;
    }

    fn write_joy_data(&mut self, val: u8) {
        //println!("Joy data written {:#X} state = {:?}", val, self.tx_state);
        let new_state = match self.tx_state.clone() {
            TXstate::Disabled => {
                warn!("CONTROLLER: Tried to write JOY_DATA while TX is disabled!");
                TXstate::Disabled
            }
            TXstate::Ready => {
                let slot = if val == CONTROLER_SELECT_BYTE {
                    Slot::Controller
                } else if val == MEMORY_CARD_SELECT_BYTE {
                    Slot::MemoryCard
                } else {
                    panic!("Unknown SIO slot!");
                };

                if slot == Slot::MemoryCard {
                    self.push_rx_buf(0);
                    return;
                }

                if !self.joy_ctrl.get_bit(13) && !self.joy_ctrl.get_bit(1)
                || self.joy_ctrl.get_bit(13) && self.joy_ctrl.get_bit(1)
                {
                    // Controller 2
                    self.push_rx_buf(0);
                    return;
                }

               


                self.push_rx_buf(0);
                self.queue_interrupt();
                TXstate::Transfering {
                    slot: slot,
                    step: 0,
                }
            }
            TXstate::Transfering { slot, step } => {
                if slot == Slot::Controller {

                  

                    let response = match step {
                        0 => 0x41, // Digital pad idlo
                        1 => 0x5A, // Digital pad idhi
                        2 => self.latest_button_state.digital_low_byte(),
                        3 => self.latest_button_state.digital_high_byte(),
                        _ => 0,
                    };
                    self.push_rx_buf(response);
                    if step < 3 {
                        self.queue_interrupt();
                    }
                    TXstate::Transfering {
                        slot: slot.clone(),
                        step: step + 1,
                    }
                } else {
                    panic!("Tried to read memory card! It's not implemented yet :(");
                }
            }
        };
        self.tx_state = new_state;
    }

    fn read_joy_stat(&mut self) -> u16 {
        let mut val: u16 = 0;

        if self.tx_state != TXstate::Disabled {
            val |= 0x1;
        };

        if !self.rx_buf.is_empty() {
            val |= 0x2;
        }

        if self.tx_state != TXstate::Ready {
            val |= 0x4;
        }

        if self.irq_status {
            val |= 0x200;
        }

        if !self.rx_buf.is_empty() {
            val |= 2;
        }

        // if self.joy_ctrl.get_bit(12) {
        //     val |= 0x1000;
        // }

        //val |= 0x80;
        //println!("Reading JOY_STAT {:#X}", val);

        val
    }

    fn read_joy_ctrl(&mut self) -> u16 {
        //println!("Reading joy_ctrl {:#X}", self.joy_ctrl);
        self.joy_ctrl
    }

    fn read_joy_data(&mut self) -> u8 {
        //println!("joy data read");
        self.pop_rx_buf()
    }

    fn reset(&mut self) {
        //println!("Resetting");
        self.write_joy_ctrl(0);
        self.rx_buf.clear();
        self.pending_irq = false;
        self.irq_status = false;
        self.irq_cycle_timer = 0;
    }

    fn acknowledge(&mut self) {
        self.irq_status = false;
    }

    fn push_rx_buf(&mut self, val: u8) {
        self.rx_buf.push_back(val);
    }

    fn pop_rx_buf(&mut self) -> u8 {
        match self.rx_buf.pop_front() {
            Some(val) => val,
            _ => 0,
        }
    }

    fn queue_interrupt(&mut self) {
        self.pending_irq = true;
        self.irq_status = true;
        self.irq_cycle_timer = 350;
    }
}

pub(super) fn controller_execute_cycle(cpu: &mut R3000) {
    if cpu.main_bus.controllers.irq_cycle_timer > 0 {
        // We are waiting for the dumb ack delay to expire
        cpu.main_bus.controllers.irq_cycle_timer -= 1;
    } else if cpu.main_bus.controllers.pending_irq {
        // The dumb ack delay has expired, so now we can fire an INT7
        cpu.fire_external_interrupt(InterruptSource::Controller);
        cpu.main_bus.controllers.pending_irq = false;
    }
}
