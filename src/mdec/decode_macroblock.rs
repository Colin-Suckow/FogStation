use std::os::raw;

use bit_field::BitField;

use super::MdecCommand;

const END_CODE: u16 = 0xFE00;

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



    fn execute(&self, ctx: &mut super::MDEC) {
        let mut parameters = vec!();
        println!("buf {:?}", ctx.parameter_buffer);
        for w in &ctx.parameter_buffer {
            parameters.push((w & 0xFFFF) as u16);
            parameters.push((w >> 16) as u16);
        }
        
        let mut current_block = Vec::<u16>::new();
        for parameter in parameters {
            if parameter != END_CODE {
                //println!("pushed {:#X}", parameter);
                current_block.push(parameter);
            } else {
                //println!("Executing block");
                let decoded_block = decode_block(ctx, &current_block, &self.depth);
                ctx.result_buffer.extend(decoded_block);
                current_block.clear();
            }
        }
        println!("Done");
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



fn decode_block(ctx: & super::MDEC, raw_block: &Vec<u16>, color_depth: &ColorDepth) -> Vec<u32> {

    // Algorithm copied from https://raw.githubusercontent.com/m35/jpsxdec/readme/jpsxdec/PlayStation1_STR_format.txt
    
    // Translate the DC and AC run length codes into a 64 value list

    // let mut coefficient_list: Vec<u16> = vec![0; 64];
    
    // let dc_coefficient = raw_block[0] & 0x3ff;
    // let quantization_scale = raw_block[0] >> 10;

    // coefficient_list[0] = dc_coefficient;
    // let mut i = 0;

    // for rlc in raw_block {
    //     i += 1 + (rlc >> 10);
    //     println!("i {} rlc {:#X}", i, rlc);
    //     coefficient_list[i as usize] = rlc & 0x3ff;
    // }

    // // Un-zig-zag the list into a matrix

    // let mut coefficient_matrix: Vec<i16> = vec![0; 64];

    // for i in 0..64 {
    //     coefficient_matrix[i] = coefficient_list[ZIG_ZAG_MATRIX[i]] as i16;
    // }

    // // Dequantization of the matrix
    // let mut dequantized_matrix: Vec<i16> = vec![0; 64];

    // for i in 0..64 {
    //     if i == 0 {
    //         dequantized_matrix[i] = coefficient_matrix[i] * ctx.scale_table[i];
    //     } else {
    //         dequantized_matrix[i] = 2 * coefficient_matrix[i] * ctx.scale_table[i] * quantization_scale as i16 / 16;
    //     }
    // }

    // println!("{:?}", dequantized_matrix);
    
    
    // TODO do the real decoding
    match color_depth {
        ColorDepth::B4 => todo!(),
        ColorDepth::B8 => todo!(),
        ColorDepth::B24 => vec!(0xFFFFFF; 16*16),
        ColorDepth::B15 => vec!(0x1F001F; 16*16/2),
    }
}

const ZIG_ZAG_MATRIX: [usize; 64] = [
    0,  1,  5,  6, 14, 15, 27, 28,
    2,  4,  7, 13, 16, 26, 29, 42,
    3,  8, 12, 17, 25, 30, 41, 43,
    9, 11, 18, 24, 31, 40, 44, 53,
    10, 19, 23, 32, 39, 45, 52, 54,
    20, 22, 33, 38, 46, 51, 55, 60,
    21, 34, 37, 47, 50, 56, 59, 61,
    35, 36, 48, 49, 57, 58, 62, 63
];