use bit_field::BitField;

use super::MdecCommand;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ColorDepth {
    B4 = 0,
    B8 = 1,
    B24 = 2,
    B15 = 3,
}
#[derive(Clone, Copy)]
pub(crate) struct DecodeMacroblockCommand {
    depth: ColorDepth,
    signed: bool,
    set_b15: bool,
    size: usize,
}

impl DecodeMacroblockCommand {
    pub(crate) fn new(command_word: u32) -> Self {
        if command_word >> 29 != 1 {
            panic!(
                "Not a decode_macroblock command! Command number = {}",
                command_word >> 29
            );
        }

        let depth = match (command_word >> 27) & 3 {
            0 => ColorDepth::B4,
            1 => ColorDepth::B8,
            2 => ColorDepth::B24,
            3 => ColorDepth::B15,
            _ => unreachable!(),
        };

        let signed = command_word.get_bit(26);
        let set_b15 = command_word.get_bit(25);
        let size = (command_word & 0xFFFF) as usize;
        println!("Color depth {}", (command_word >> 27) & 3);
        println!("MACROBLOCK size: {}", size);

        Self {
            depth,
            signed,
            set_b15,
            size
        }
    }
}

impl MdecCommand for DecodeMacroblockCommand {
    fn parameter_words(&self) -> usize {
        self.size
    }

    // 0xFE00 is MDEC end code

    fn execute(&self, ctx: &mut super::MDEC) {
        let mut parameters = vec!();
        for w in &ctx.parameter_buffer {
            parameters.push((w & 0xFFFF) as u16);
            parameters.push((w >> 16) as u16);
        }
        let block_count = parameters.into_iter().reduce(|total, item| if item == 0xFE00 {total + 1} else { total}).unwrap();
        println!("{:?}", self.depth);
        for _ in 0..block_count {
            ctx.result_buffer.extend(vec!(0x1f; 256));
        }
        // let blocks = vec!();
        // let current_block = vec!();
        // for parameter in parameters {
        //     if parameters != 0FE00 
        // }
    }

    fn box_clone(&self) -> Box<dyn MdecCommand> {
        Box::new((*self).clone())
    }

    fn name(&self) -> &str {
        "DecodeMacroblock"
    }

    fn set_status(&self, status: &mut u32) {
        *status |= match self.depth {
            ColorDepth::B4 => 0,
            ColorDepth::B8 => 1,
            ColorDepth::B24 => 2,
            ColorDepth::B15 => 3,
        } << 25;

        status.set_bit(24, self.signed);
        status.set_bit(23, self.set_b15);
       
    }
}



fn decode_block(raw_block: &[u16], tmp_count: u16, color_depth: ColorDepth) -> Vec<u32> {
    // TODO do the real decoding
    match color_depth {
        ColorDepth::B4 => todo!(),
        ColorDepth::B8 => todo!(),
        ColorDepth::B24 => vec!(0xFFFFFF; 16*16),
        ColorDepth::B15 => todo!(),
    }
}