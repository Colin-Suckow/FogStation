use bit_field::BitField;

use super::MdecCommand;

#[derive(Clone, Copy)]
pub(crate) struct SetQuantTableCommand {
    color: bool,
}

impl SetQuantTableCommand {
    pub(crate) fn new(command_word: u32) -> Self {
        if command_word >> 29 != 2 {
            panic!(
                "Not a set_quant_table command! Command number = {}",
                command_word >> 29
            );
        };

        Self {
            color: command_word.get_bit(0) as bool,
        }
    }
}

impl MdecCommand for SetQuantTableCommand {
    fn parameter_words(&self) -> usize {
        if self.color {
            32
        } else {
            16
        }
    }

    fn execute(&self, ctx: &mut super::MDEC) {
        ctx.luminance_quant_table.clear();
        for i in 0..16 {
            let bytes: [u8; 4] = ctx.parameter_buffer[i].to_le_bytes();
            ctx.luminance_quant_table.extend_from_slice(&bytes);
        }

        if self.color {
            ctx.color_quant_table.clear();
            for i in 16..32 {
                let bytes: [u8; 4] = ctx.parameter_buffer[i].to_le_bytes();
                ctx.color_quant_table.extend_from_slice(&bytes);
            }
        }
    }

    fn box_clone(&self) -> Box<dyn MdecCommand> {
        Box::new((*self).clone())
    }

    fn name(&self) -> &str {
        "SetQuantTable"
    }

    fn set_status(&self, _status: &mut u32) {}
}
