use std::collections::VecDeque;

use bit_field::BitField;

use self::{
    decode_macroblock::DecodeMacroblockCommand, set_quant_table::SetQuantTableCommand,
    set_scale_table::SetScaleTableCommand,
};

mod decode_macroblock;
mod set_quant_table;
mod set_scale_table;

enum InputState {
    Idle,
    AwaitingParameters(Box<dyn MdecCommand>),
}

impl Clone for InputState {
    fn clone(&self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::AwaitingParameters(command) => Self::AwaitingParameters(command.box_clone()),
        }
    }
}

trait MdecCommand {
    fn parameter_words(&self) -> usize;
    fn execute(&self, ctx: &mut MDEC);
    fn box_clone(&self) -> Box<dyn MdecCommand>;
    fn name(&self) -> &str;
    fn set_status(&self, status: &mut u32);
}

fn decode_command(command_word: u32) -> Box<dyn MdecCommand> {
    match command_word >> 29 {
        1 => Box::new(DecodeMacroblockCommand::new(command_word)),
        2 => Box::new(SetQuantTableCommand::new(command_word)),
        3 => Box::new(SetScaleTableCommand::new(command_word)),
        n => panic!(
            "Invalid MDEC command {}! (Full word: {:#X})",
            n, command_word
        ),
    }
}

pub(crate) struct MDEC {
    input_state: InputState,
    parameter_buffer: Vec<u32>,
    luminance_quant_table: Vec<u8>,
    color_quant_table: Vec<u8>,
    scale_table: Vec<i16>,
    result_buffer: VecDeque<u32>,

    dma_out_enabled: bool,
    dma_in_enabled: bool,
}

impl MDEC {
    pub(crate) fn new() -> Self {
        Self {
            input_state: InputState::Idle,
            parameter_buffer: vec![],
            luminance_quant_table: vec![],
            color_quant_table: vec![],
            scale_table: vec![],

            dma_out_enabled: false,
            dma_in_enabled: false,
            result_buffer: VecDeque::new(),
        }
    }

    fn reset(&mut self) {
        self.input_state = InputState::Idle;
        self.parameter_buffer = vec![];
    }

    pub(crate) fn bus_read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x1f801820 => self.read_response(),
            0x1f801824 => self.read_status(),
            _ => panic!("Tried to read unknown MDEC word! {:#X}", addr),
        }
    }

    pub(crate) fn bus_write_word(&mut self, addr: u32, word: u32) {
        match addr {
            0x1f801820 => self.write_command_register(word),
            0x1f801824 => self.write_control(word),
            _ => panic!("Tried to write unknown MDEC word! {:#X}", addr),
        }
    }

    fn write_command_register(&mut self, word: u32) {
        let current_state = self.input_state.clone();
        match current_state {
            InputState::Idle => {
                let command = decode_command(word);
                self.input_state = InputState::AwaitingParameters(command);
            }
            InputState::AwaitingParameters(command) => {
                let expected_words = command.parameter_words();
                self.parameter_buffer.push(word);

                if self.parameter_buffer.len() == expected_words {
                    self.result_buffer.clear();
                    command.execute(self);
                    self.input_state = InputState::Idle;
                    self.parameter_buffer.clear();
                }
            }
        }
    }

    fn read_status(&self) -> u32 {
        let mut result: u32 = 0;

        if let InputState::AwaitingParameters(command) = &self.input_state {
            let remaining_words =
                command.parameter_words() as isize - self.parameter_buffer.len() as isize;
            result.set_bit(29, true);
            command.set_status(&mut result);
            if remaining_words <= 0 {
                result |= 0x4000FFFF;
            } else {
                result |= (remaining_words & 0xFFFF) as u32;
            }
        } else {
            result |= 0xFFFF;
        }

        result.set_bit(27, self.dma_out_enabled);
        result.set_bit(28, self.dma_in_enabled);
        result.set_bit(31, self.result_buffer.is_empty());
        //println!("MDEC status {:#X}", result);
        result
    }

    fn write_control(&mut self, word: u32) {
        self.dma_out_enabled = word.get_bit(29);
        self.dma_in_enabled = word.get_bit(30);

        if word.get_bit(31) {
            self.reset();
        }
    }

    fn read_response(&mut self) -> u32 {
        if let Some(val) = self.result_buffer.pop_front() {
            val
        } else {
            // Buffer is empty, so return zero
            0
        }
    }
}
