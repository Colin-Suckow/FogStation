use std::{
    borrow::Borrow,
    cmp::{max, min, Ordering},
    mem::size_of_val,
};

use bit_field::BitField;
use log::{error, trace};
use nalgebra::Vector2;
use num_traits::clamp;

const CYCLES_PER_SCANLINE: u32 = 2500;
const TOTAL_SCANLINES: u32 = 245;

#[derive(Copy, Clone, Debug)]
enum TextureColorMode {
    FourBit,
    EightBit,
    FifteenBit,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Resolution {
    pub height: u32,
    pub width: u32,
}

#[derive(Copy, Clone, Debug)]
struct Point {
    x: i32,
    y: i32,
    color: u16,
    tex_x: i32,
    tex_y: i32,
}

enum ColorDepth {
    Full,    // 24 bit
    Reduced, // 15 bit
}

impl Point {
    fn from_word(word: u32, color: u16) -> Self {
        let result = Self {
            x: sign_extend(word as i32 & 0x7FF, 11),
            y: sign_extend(((word as i32) >> 16) & 0x7FF, 11),
            color,
            tex_x: 0,
            tex_y: 0,
        };
        result
    }

    fn from_word_with_offset(word: u32, color: u16, offset: &Point) -> Self {
        Self {
            x: sign_extend(word as i32 & 0x7FF, 11) + offset.x,
            y: sign_extend((word as i32 >> 16) & 0x7FF, 11) + offset.y,
            color: 0,
            tex_x: 0,
            tex_y: 0,
        }
    }

    fn from_components(x: i32, y: i32, color: u16) -> Self {
        Self {
            x,
            y,
            color,
            tex_x: 0,
            tex_y: 0,
        }
    }

    fn new_textured_point(word: u32, tex_y: i32, tex_x: i32) -> Self {
        Self {
            x: sign_extend(word as i32 & 0x7FF, 11),
            y: sign_extend(((word as i32) >> 16) & 0x7FF, 11),
            color: 0,
            tex_x,
            tex_y,
        }
    }
}

fn sign_extend(x: i32, nbits: u32) -> i32 {
    let notherbits = size_of_val(&x) as u32 * 8 - nbits;
    x.wrapping_shl(notherbits).wrapping_shr(notherbits)
}

#[allow(dead_code)]

pub struct Gpu {
    vram: Vec<u16>,
    status_reg: u32,
    pixel_count: u32,
    enabled: bool,
    gp0_buffer: Vec<u32>,
    color_depth: ColorDepth,

    texpage_x_base: u16,
    texpage_y_base: u16,
    texmode: TextureColorMode,
    palette_x: u16,
    palette_y: u16,
    blend_enabled: bool,
    blend_color: u16,

    draw_area_tl_point: Point,
    draw_area_br_point: Point,
    draw_offset: Point,

    irq_fired: bool,
    vblank_consumed: bool,
    hblank_consumed: bool,
    show_frame: bool,
    frame_ready: bool,

    display_h_res: u32,
    display_v_res: u32,

    ntsc_y1: u32,
    ntsc_y2: u32,

    blend_mode: BlendMode,
    force_mask: bool,
    check_mask: bool,
}

impl Gpu {
    pub fn new() -> Gpu {
        Gpu {
            vram: vec![0; 1_048_576 / 2],
            status_reg: 0x1C000000,
            pixel_count: 0,
            enabled: false,
            gp0_buffer: Vec::new(),
            color_depth: ColorDepth::Reduced,

            texpage_x_base: 0,
            texpage_y_base: 0,
            texmode: TextureColorMode::FifteenBit,
            palette_x: 0,
            palette_y: 0,
            blend_enabled: false,
            blend_color: 0xFFFF,

            draw_area_tl_point: Point::from_components(0, 0, 0),
            draw_area_br_point: Point::from_components(0, 0, 0),

            draw_offset: Point::from_components(0, 0, 0),
            irq_fired: false,
            vblank_consumed: false,
            hblank_consumed: false,
            show_frame: false,
            frame_ready: false,

            display_h_res: 640,
            display_v_res: 480,

            ntsc_y1: 16,
            ntsc_y2: 256,

            blend_mode: BlendMode::BAF,
            force_mask: false,
            check_mask: false,
        }
    }

    //Only reseting the big stuff. This will probably bite me later
    pub fn reset(&mut self) {
        self.vram = vec![0; 1_048_576 / 2];
        self.status_reg = 0x1C000000;
        self.gp0_buffer = Vec::new();
        self.pixel_count = 0;
    }

    pub fn read_status_register(&mut self) -> u32 {
        //trace!("Reading GPUSTAT");
        let mut stat: u32 = 0;

        stat |= (self.texpage_x_base) as u32;
        stat |= (self.texpage_y_base << 4) as u32;

        stat |= match self.texmode {
            TextureColorMode::FourBit => 0,
            TextureColorMode::EightBit => 1,
            TextureColorMode::FifteenBit => 2,
        } << 7;

        stat |= 0x1C000000;

        stat
    }

    pub fn read_word_gp0(&mut self) -> u32 {
        //trace!("Reading gp0");
        0x0 as u32
    }

    pub fn send_gp0_command(&mut self, value: u32) {
        self.gp0_push(value);

        let command = self.gp0_buffer[0];

        match command.gp0_header() {
            0x0 => {
                //Random junk
                match command >> 24 {
                    0x2 => {
                        //Quick rectangle fill
                        if self.gp0_buffer.len() < 3 {
                            //Not enough commands
                            return;
                        }
                        trace!("Quick rec");

                        let mut p1 = Point::from_components((self.gp0_buffer[1] & 0xFFFF) as i32, ((self.gp0_buffer[1] >> 16) & 0xFFFF) as i32, 0);
                        let mut p2 = Point::from_components((self.gp0_buffer[2] & 0xFFFF) as i32, ((self.gp0_buffer[2] >> 16) & 0xFFFF) as i32, 0);

                        p1.x += self.draw_offset.x;
                        p1.y += self.draw_offset.y;
                        p2.x += self.draw_offset.x;
                        p2.y += self.draw_offset.y;

                        p2.x += p1.x;
                        p2.y += p1.y;

                        //println!("quick fill p1 {:?}  p2 {:?}", p1, p2);

                        self.draw_solid_box(
                            p1.x as u32,
                            p1.y as u32,
                            p2.x as u32,
                            p2.y as u32,
                            b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF) + 1,
                            false,
                        );
                    }
                    _ => {
                        //NOP
                    }
                }
            }

            0x1 => {
                //Render Polygon

                // If the polygon is textured or gouraud shaded, lets just lock up the emulator.
                // I only want to test flat shaded polygons right now

                let is_gouraud = command.get_bit(28);
                let is_textured = command.get_bit(26);
                let is_quad = command.get_bit(27);
                let verts = if is_quad { 4 } else { 3 };

                let packets = 1
                    + (verts * is_textured as usize)
                    + verts
                    + if is_gouraud { verts - 1 } else { 0 };

                if self.gp0_buffer.len() < packets {
                    // Not enough words for the command. Return early
                    return;
                }

                let fill = b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF);
                self.blend_enabled = self.gp0_buffer[0].get_bit(24);
                self.blend_color = fill;
                if is_quad {
                    if is_textured && is_gouraud {
                        //Should be blending in colors. Do that later
                        trace!("Tried to try draw texture blended quad!");

                        let mut points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[4],
                                ((self.gp0_buffer[5] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[5] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[7],
                                ((self.gp0_buffer[8] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[8] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[10],
                                ((self.gp0_buffer[11] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[11] & 0xFF) as i32,
                            ),
                        ];

                        trace!("points {:?}", points);

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        self.blend_color = fill;

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_textured_quad(&points, command.get_bit(25));
                        }
                    } else if is_textured {
                        trace!("GPU: Tex quad");
                        let mut points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[3],
                                ((self.gp0_buffer[4] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[4] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[5],
                                ((self.gp0_buffer[6] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[6] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[7],
                                ((self.gp0_buffer[8] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[8] & 0xFF) as i32,
                            ),
                        ];

                        trace!("points {:?}", points);

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        self.blend_color = fill;

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_textured_quad(&points, command.get_bit(25));
                        }
                    } else if is_gouraud {
                        trace!("GPU: gouraud quad");
                        let mut points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], fill),
                            Point::from_word(
                                self.gp0_buffer[3],
                                b24color_to_b15color(self.gp0_buffer[2]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[5],
                                b24color_to_b15color(self.gp0_buffer[4]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[7],
                                b24color_to_b15color(self.gp0_buffer[6]),
                            ),
                        ];

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        self.draw_shaded_quad(&points, command.get_bit(25));
                    } else {
                        trace!("GPU: Solid quad");
                        let mut points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], 0),
                            Point::from_word(self.gp0_buffer[2], 0),
                            Point::from_word(self.gp0_buffer[3], 0),
                            Point::from_word(self.gp0_buffer[4], 0),
                        ];

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_solid_quad(&points, fill, command.get_bit(25));
                        }

                        //let center = center_of_points(&points);

                        // points.sort_unstable_by(|a, b| sort_clockwise_big_match(*a, *b, center));
                    };
                } else {
                    if is_gouraud && is_textured {
                        trace!(
                            "Tried to try draw texture blended tri! Queue {:?}",
                            self.gp0_buffer
                        );

                        let mut points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[4],
                                ((self.gp0_buffer[5] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[5] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[7],
                                ((self.gp0_buffer[8] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[8] & 0xFF) as i32,
                            ),
                        ];

                        trace!("points {:?}", points);

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        self.blend_color = fill;

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_textured_triangle(&points, command.get_bit(25));
                        }
                    } else if is_textured {
                        trace!("GPU: Tex tri");
                        let mut points: Vec<Point> = vec![
                            Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[3],
                                ((self.gp0_buffer[4] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[4] & 0xFF) as i32,
                            ),
                            Point::new_textured_point(
                                self.gp0_buffer[5],
                                ((self.gp0_buffer[6] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[6] & 0xFF) as i32,
                            ),
                        ];

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        ////trace!("{:?}", points);
                        self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                        self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;
                        //trace!("palx {}", self.palette_x);
                        self.texpage_x_base = ((self.gp0_buffer[4] >> 16) & 0xF) as u16;
                        self.texpage_y_base = ((self.gp0_buffer[4] >> 20) & 0x1) as u16;
                        // self.blend_color = if fill == 0 {
                        //     0xFFFF
                        // } else {
                        //     fill
                        // };
                        self.blend_color = fill;

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_textured_triangle(&points, command.get_bit(25));
                        }
                    } else if is_gouraud {
                        trace!("GPU: gouraud tri");
                        let mut points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], fill),
                            Point::from_word(
                                self.gp0_buffer[3],
                                b24color_to_b15color(self.gp0_buffer[2]),
                            ),
                            Point::from_word(
                                self.gp0_buffer[5],
                                b24color_to_b15color(self.gp0_buffer[4]),
                            ),
                        ];

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_shaded_triangle(&points, command.get_bit(25));
                        }

                        ////trace!("{:?}", points);
                    } else {
                        trace!("GPU: Solid tri");
                        let mut points: Vec<Point> = vec![
                            Point::from_word(self.gp0_buffer[1], 0),
                            Point::from_word(self.gp0_buffer[3], 0),
                            Point::from_word(self.gp0_buffer[2], 0),
                        ];

                        for point in &mut points {
                            point.x += self.draw_offset.x;
                            point.y += self.draw_offset.y;
                        }

                        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
                        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

                        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
                        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

                        if max_x - min_x > 1023 || max_y - min_y > 511 {
                            trace!("Quad too big, dropping");
                        } else {
                            self.draw_solid_triangle(&points, fill, command.get_bit(25));
                        }
                    }
                }
            }

            0x2 => {
                //Render line
                if command.get_bit(27) {
                    ////trace!("{:?}", self.gp0_buffer);
                    trace!("GPU: Polyline");
                    if (self.gp0_buffer[self.gp0_buffer.len() - 1] & 0xF000F000) != 0x50005000 {
                        //Wait until terminating vertex
                        return;
                    }
                    //TODO draw polyline
                } else {
                    if self.gp0_buffer.len() < (3 + if command.get_bit(28) { 2 } else { 0 }) {
                        //Not enough commands
                        return;
                    }

                    trace!("GPU: Line")

                    //TODO draw line
                }
            }

            0x3 => {
                //Render Rectangle

                let size = (command >> 27) & 0x3;

                let length =
                    2 + if size == 0 { 1 } else { 0 } + if command.get_bit(26) { 1 } else { 0 };

                if self.gp0_buffer.len() < length {
                    //Not enough commands
                    return;
                }

                match size {
                    0b01 => {
                        trace!("GPU: Single point");
                        //Draw single pixel
                        let point = Point::from_word(self.gp0_buffer[1], 0);

                        let address = point_to_address(point.x as u32, point.y as u32) as usize;
                        let fill = b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF);
                        self.composite_and_place_pixel(address, fill, false);
                    }

                    0b0 => {
                        //Draw variable size
                        if command.get_bit(26) {
                            trace!("GPU: Tex box");
                            let tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            );

                            let size = Point::from_word(self.gp0_buffer[3], 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            trace!("GPU: solid box");
                            let tl_point = Point::from_word(self.gp0_buffer[1], 0);
                            let br_point =
                                Point::from_word_with_offset(self.gp0_buffer[2], 0, &tl_point);

                            trace!("tl: {:?} br: {:?}", tl_point, br_point);

                            self.draw_solid_box(
                                (tl_point.x + self.draw_offset.x) as u32,
                                (tl_point.y + self.draw_offset.y) as u32,
                                (br_point.x + self.draw_offset.x) as u32,
                                (br_point.y + self.draw_offset.y) as u32,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    0b10 => {
                        //8x8 sprite
                        trace!("GPU: 8x8 sprite");
                        if command.get_bit(26) {
                            let mut tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            );

                            let size = Point::from_components(8, 8, 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            tl_point.x += self.draw_offset.x;
                            tl_point.y += self.draw_offset.y;
                            


                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            let x1 = (self.gp0_buffer[1] & 0xFFFF) as i32 + self.draw_offset.x;
                            let y1 = ((self.gp0_buffer[1] >> 16) & 0xFFFF) as i32 + self.draw_offset.y;
                            self.draw_solid_box(
                                x1 as u32,
                                y1 as u32,
                                x1 as u32 + 8,
                                y1 as u32 + 8,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    0b11 => {
                        //16x16 sprite
                        trace!("GPU: 16x16 sprite");
                        if command.get_bit(26) {
                            let mut tl_point = Point::new_textured_point(
                                self.gp0_buffer[1],
                                ((self.gp0_buffer[2] >> 8) & 0xFF) as i32,
                                (self.gp0_buffer[2] & 0xFF) as i32,
                            );

                            let size = Point::from_components(16, 16, 0);

                            self.palette_x = ((self.gp0_buffer[2] >> 16) & 0x3F) as u16;
                            self.palette_y = ((self.gp0_buffer[2] >> 22) & 0x1FF) as u16;

                            tl_point.x += self.draw_offset.x;
                            tl_point.y += self.draw_offset.y;

                            self.draw_textured_box(&tl_point, size.x, size.y, command.get_bit(25));
                        } else {
                            let x1 = (self.gp0_buffer[1] & 0xFFFF) as i32 + self.draw_offset.x;
                            let y1 = ((self.gp0_buffer[1] >> 16) & 0xFFFF) as i32 + self.draw_offset.y;
                            self.draw_solid_box(
                                x1 as u32,
                                y1 as u32,
                                x1 as u32 + 16,
                                y1 as u32 + 16,
                                b24color_to_b15color(self.gp0_buffer[0] & 0x1FFFFFF),
                                command.get_bit(25),
                            );
                        }
                    }

                    _ => {
                        //Lets do nothing with the others
                        trace!("GPU: Invalid size rect");
                    }
                }
            }

            0x4 => {
                //VRAM to VRAM blit
                if self.gp0_buffer.len() < 4 {
                    //Not enough commands
                    return;
                }
                trace!("GPU: VRAM -> VRAM blit");
                //trace!("Running VRAM to VRAM transfer");
                let x_source = self.gp0_buffer[1] & 0xFFFF;
                let y_source = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                let x_dest = self.gp0_buffer[2] & 0xFFFF;
                let y_dest = (self.gp0_buffer[2] >> 16) & 0xFFFF;
                let mut width = self.gp0_buffer[3] & 0xFFFF;
                let mut height = (self.gp0_buffer[3] >> 16) & 0xFFFF;

                if width == 0 {
                    width = 1024
                };
                if height == 0 {
                    height = 512
                };

                if width == 0 || height == 0 {
                    panic!("0 width or height! w {} h {}", width, height);
                }

                self.copy_rectangle(x_source, y_source, x_dest, y_dest, width, height);
            }
            0x5 => {
                //CPU To VRAM

                if self.gp0_buffer.len() < 3 {
                    //Not enough for the header
                    return;
                }
                trace!("cpu to vram");
                let mut width = (self.gp0_buffer[2] & 0xFFFF) as u32;
                let mut height = ((self.gp0_buffer[2] >> 16) & 0xFFFF) as u32;
                if width == 0 {
                    width = 1024
                };
                if height == 0 {
                    height = 512
                };
                let length = ((width * height) / 2) + if width % 2 != 0 { 1 } else { 0 } + 3;
                if self.gp0_buffer.len() < length as usize {
                    //Not enough commands
                    return;
                }
                trace!(
                    "GPU: CPU to VRAM length: {} ({} x {})",
                    length,
                    width,
                    height
                );

                let base_x = (self.gp0_buffer[1] & 0xFFFF) as i16;
                let base_y = ((self.gp0_buffer[1] >> 16) & 0xFFFF) as i16;

                for index in 3..(length) {
                    let p2 = ((self.gp0_buffer[index as usize] >> 16) & 0xFFFF) as u16;
                    let p1 = (self.gp0_buffer[index as usize] & 0xFFFF) as u16;
                    let x = base_x + (((index - 3) * 2) % (width)) as i16;
                    let y = base_y + (((index - 3) * 2) / (width)) as i16;
                    let addr = point_to_address(x as u32, y as u32);
                    self.vram[addr as usize] = p1;
                    self.vram[(addr + 1) as usize] = p2;
                }
            }

            0x6 => {
                //VRAM to CPU
                if self.gp0_buffer.len() < 3 {
                    return;
                }

                let width = (self.gp0_buffer[2] & 0xFFFF) as u32;
                let height = (((self.gp0_buffer[2] >> 16) & 0xFFFF) as u32) * 2;

                if width == 0 || height == 0 {
                    panic!("0 width or height! w {} h {}", width, height);
                }
                trace!("GPU: VRAM to CPU")
                //Lets ignore this one for now
            }
            0x7 => {
                //Env commands
                match command.command() {
                    0xE1 => {
                        //Draw Mode Setting
                        self.texpage_x_base = (command & 0xF) as u16;
                        self.texpage_y_base = if command.get_bit(4) { 1 } else { 0 };
                        self.texmode = match (command >> 7) & 0x3 {
                            0 => TextureColorMode::FourBit,
                            1 => TextureColorMode::EightBit,
                            2 => TextureColorMode::FifteenBit,
                            3 => TextureColorMode::FifteenBit, // This one is FifteenBit, for some reason
                            _ => panic!("Unknown texture color mode {}", (command >> 7) & 0x3),
                        };
                        self.blend_mode = match command.get_bits(5..=6) {
                            0 => BlendMode::B2F2,
                            1 => BlendMode::BAF,
                            2 => BlendMode::BSF,
                            _ => BlendMode::BF4,
                        };
                    }

                    0xE3 => {
                        //Set Drawing Area Top Left
                        self.draw_area_tl_point = Point::from_components(
                            ((command & 0x3FF) as u16) as i32,
                            (((command >> 10) & 0x3FF) as u16) as i32,
                            0,
                        );
                    }

                    0xE4 => {
                        //Set Drawing Area Bottom Right
                        self.draw_area_br_point = Point::from_components(
                            ((command & 0x3FF) as u16) as i32,
                            (((command >> 10) & 0x3FF) as u16) as i32,
                            0,
                        );
                    }

                    0xE5 => {
                        //Set Drawing Offset
                        let x = sign_extend(command as i32 & 0x7FF, 11);
                        let y = sign_extend((command as i32 >> 11) & 0x7FF, 11);
                        self.draw_offset = Point::from_components(x, y, 0);
                    }

                    _ => error!(
                        "Unknown GPU ENV command {:#X}. Full command queue is {:#X}",
                        command.command(),
                        self.gp0_buffer[0]
                    ),
                }
            }

            0x1F => {
                panic!("GPU IRQ requested!");
            }

            _ => error!("unknown gp0 {:#X}!", command.gp0_header()),
        }
        trace!("Command was {:#X}", command);
        //Made it to the end, so the command must have been executed
        self.gp0_clear();
    }

    pub fn send_gp1_command(&mut self, command: u32) {
        //trace!("GP1 Command {:#X} parameter {:#X}", command.command(), command.parameter());
        match command.command() {
            0x0 => {
                //Reset GPU
                self.enabled = false;
                self.status_reg = 0;
                self.pixel_count = 0;
                self.vram = vec![0; 1_048_576 / 2];
            }

            0x1 => {
                //Reset Command buffer
                self.gp0_buffer.clear();
            }

            // 0x2 => {
            //     self.show_frame = true;
            // }
            0x4 => {
                // gpu dma direction. I don't think this is needed
            }

            0x6 => {
                //Horizontal Display Range
                //Ignore this one for now
            }

            0x7 => {
                //Vertical display range
                self.ntsc_y1 = command.get_bits(0..9);
                self.ntsc_y2 = command.get_bits(10..19);
            }

            0x8 => {
                //Display mode
                self.display_h_res = {
                    if command.get_bit(6) {
                        368
                    } else {
                        match command & 0x3 {
                            0 => 256,
                            1 => 320,
                            2 => 512,
                            3 => 640,
                            _ => unreachable!(),
                        }
                    }
                };

                self.display_v_res = if command.get_bit(2) && command.get_bit(5) {
                    480
                } else {
                    240
                };

                self.color_depth = match command.get_bit(4) {
                    true => ColorDepth::Full,
                    false => ColorDepth::Reduced,
                }
            }

            0x10 => {
                //Get gpu information
                trace!("Getting gpu info {:#X}", command.parameter());
            }
            _ => error!(
                "Unknown gp1 command {:#X} parameter {}!",
                command.command(),
                command.parameter()
            ),
        }
    }

    pub fn execute_cycle(&mut self) {
        self.pixel_count += 1;

        if self.pixel_count % CYCLES_PER_SCANLINE == 0 {
            self.hblank_consumed = false;
        }

        if self.pixel_count > CYCLES_PER_SCANLINE * TOTAL_SCANLINES {
            self.pixel_count = 0;
            self.vblank_consumed = false;
            self.frame_ready = true;
            trace!("VBLANK DONE");
        }
    }

    pub fn is_vblank(&self) -> bool {
        self.pixel_count > CYCLES_PER_SCANLINE * (self.ntsc_y2 - self.ntsc_y1)
    }

    pub fn is_hblank(&self) -> bool {
        self.pixel_count % CYCLES_PER_SCANLINE > self.display_h_res
    }

    pub fn resolution(&self) -> Resolution {
        Resolution {
            width: self.display_h_res,
            height: self.display_v_res,
        }
    }

    pub fn consume_vblank(&mut self) -> bool {
        if !self.vblank_consumed && self.is_vblank() {
            trace!("VBLANK consumed");
            self.vblank_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn consume_hblank(&mut self) -> bool {
        if !self.hblank_consumed && self.is_hblank() {
            self.hblank_consumed = true;
            true
        } else {
            false
        }
    }

    pub fn take_frame_ready(&mut self) -> bool {
        if self.frame_ready {
            self.frame_ready = false;
            true
        } else {
            false
        }
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        &self.vram
    }

    ///Returns irq status. If true, function will return true then clear irq status
    pub fn consume_irq(&mut self) -> bool {
        if self.irq_fired {
            self.irq_fired = false;
            true
        } else {
            false
        }
    }

    fn gp0_push(&mut self, val: u32) {
        self.gp0_buffer.push(val);
    }

    fn gp0_clear(&mut self) {
        self.gp0_buffer.clear();
    }

    fn copy_horizontal_line(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
    ) {
        for x_offset in 0..=width {
            let val = self.vram[(point_to_address(x_source + x_offset, y_source) as usize)];
            let addr = point_to_address(x_dest + x_offset, y_dest) as usize;
            self.vram[addr] = val;
        }
    }

    fn copy_rectangle(
        &mut self,
        x_source: u32,
        y_source: u32,
        x_dest: u32,
        y_dest: u32,
        width: u32,
        height: u32,
    ) {
        for y_offset in 0..height {
            self.copy_horizontal_line(
                x_source,
                y_source + y_offset,
                x_dest,
                y_dest + y_offset,
                width,
            );
        }
    }

    fn draw_horizontal_line(&mut self, x1: u32, x2: u32, y: u32, fill: u16, transparent: bool) {
        for x in x1..x2 {
            if self.out_of_draw_area(&Point::from_components(x as i32, y as i32, 0)) {
                continue;
            }
            let address = point_to_address(x, y) as usize;
            self.composite_and_place_pixel(address, fill, transparent);
        }
    }

    
    fn out_of_draw_area(&self, test_point: &Point) -> bool {
        !(test_point.x > self.draw_area_tl_point.x
            && test_point.x < self.draw_area_br_point.x
            && test_point.y > self.draw_area_tl_point.y
            && test_point.y < self.draw_area_br_point.y)
    }

    fn draw_horizontal_line_textured(
        &mut self,
        x1: i32,
        x2: i32,
        y: i32,
        y1_tex: i32,
        y2_tex: i32,
        x1_tex: i32,
        x2_tex: i32,
        transparent: bool,
    ) {
        let (start, end) = if x1 > x2 { (x2, x1) } else { (x1, x2) };
        ////trace!("x1: {} y1: {} x2: {} y2: {}", x1_tex, y1_tex, x2_tex, y2_tex);
        for x in start..end {
            if self.out_of_draw_area(&Point::from_components(x, y, 0)) {
                continue;
            }

            let address = point_to_address(x as u32, y as u32) as usize;

            let fill = self.get_texel(
                lerp_coords(x1_tex, x2_tex, start, end, x),
                lerp_coords(y1_tex, y2_tex, start, end, x),
            );
           
            self.composite_and_place_pixel(address, fill, transparent);
        }
    }

    fn composite_and_place_pixel(&mut self, addr: usize, fill: u16, transparent: bool) {
        let color = if transparent && fill.get_bit(15) {
            alpha_composite(self.vram[addr], fill, &self.blend_mode)
        } else {
            fill
        };
        if color != 0 {
            self.vram[min(addr, 524287)] = color;
        }
    }

    fn draw_solid_box(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, fill: u16, transparent: bool) {
        for y in y1..y2 {
            self.draw_horizontal_line(x1, x2, y, fill, transparent);
        }
    }

    fn draw_textured_box(&mut self, tl_point: &Point, width: i32, height: i32, transparent: bool) {
        for offset in 0..height {
            self.draw_horizontal_line_textured(
                tl_point.x,
                tl_point.x + width,
                tl_point.y + offset,
                tl_point.tex_y + offset,
                tl_point.tex_y + offset,
                tl_point.tex_x,
                tl_point.tex_x + width,
                transparent,
            )
        }
    }

    fn draw_solid_triangle(&mut self, in_points: &[Point], fill: u16, transparent: bool) {
        fn edge_function(a: &Point, b: &Point, c: &Vector2<i32>) -> bool {
            (c.x as isize - a.x as isize) * (b.y as isize - a.y as isize)
                - (c.y as isize - a.y as isize) * (b.x as isize - a.x as isize)
                <= 0
        }

        let points = sort_points_clockwise(&in_points);

        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;
        

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let point = Vector2::new(x, y);
                let inside = edge_function(&points[0], &points[1], &point)
                    && edge_function(&points[1], &points[2], &point)
                    && edge_function(&points[2], &points[0], &point);
                let addr = ((y as u32) * 1024) + x as u32;
                if !self.out_of_draw_area(&Point::from_components(x, y, 0)) && inside {
                    self.vram[min(addr as usize, 524287)] = fill;
                }
            }
        }
    }

    fn draw_shaded_triangle(&mut self, in_points: &[Point], transparent: bool) {
        // let mut sp = points.to_vec();
        // sp.sort_by_key(|p| p.y);

        // if sp[1].y == sp[2].y {
        //     self.draw_shaded_flat_bottom_triangle(sp[0], sp[1], sp[2], transparent);
        // } else if sp[0].y == sp[1].y {
        //     self.draw_shaded_flat_top_triangle(sp[0], sp[1], sp[2], transparent);
        // } else {
        //     let bound_x = (sp[0].x
        //         + ((sp[1].y - sp[0].y) as f32 / (sp[2].y - sp[0].y) as f32) as i32
        //             * (sp[2].x - sp[0].x)) as i32;
        //     let bound_point = Point::from_components(bound_x, sp[1].y, sp[2].color);
        //     self.draw_shaded_flat_bottom_triangle(sp[0], bound_point, sp[1], transparent);
        //     self.draw_shaded_flat_top_triangle(sp[1], bound_point, sp[2], transparent);
        // }

        fn edge_function(a: &Point, b: &Point, c: &Vector2<i32>) -> isize {
            (c.x as isize - a.x as isize) * (b.y as isize - a.y as isize)
                - (c.y as isize - a.y as isize) * (b.x as isize - a.x as isize)
        }

        let points = sort_points_clockwise(&in_points);

        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

        let area = edge_function(
            &points[0],
            &points[1],
            &Vector2::new(points[2].x, points[2].y),
        );

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let point = Vector2::new(x, y);
                let mut w0 = edge_function(&points[1], &points[2], &point) as f32;
                let mut w1 = edge_function(&points[2], &points[0], &point) as f32;
                let mut w2 = edge_function(&points[0], &points[1], &point) as f32;

                let addr = ((y as u32) * 1024) + x as u32;

                if !self.out_of_draw_area(&Point::from_components(x, y, 0))
                    && w0 <= 0.0
                    && w1 <= 0.0
                    && w2 <= 0.0
                {
                    w0 /= area as f32;
                    w1 /= area as f32;
                    w2 /= area as f32;

                    // Jesus this is bad

                    let c1 = b15_to_rgb(points[0].color);
                    let c2 = b15_to_rgb(points[1].color);
                    let c3 = b15_to_rgb(points[2].color);

                    let red = (w0 * c1.0 as f32) + (w1 * c2.0 as f32) + (w2 * c3.0 as f32);

                    let green = (w0 * c1.1 as f32) + (w1 * c2.1 as f32) + (w2 * c3.1 as f32);

                    let blue = (w0 * c1.2 as f32) + (w1 * c2.2 as f32) + (w2 * c3.2 as f32);

                    let mut fill = (((red as u8 as u16) & 0x1f) << 10)
                        | ((green as u8 as u16) << 5)
                        | (blue as u8 as u16);

                    if points[0].color.get_bit(15)
                        || points[1].color.get_bit(15)
                        || points[2].color.get_bit(15)
                    {
                        fill.set_bit(15, true);
                    }

                    self.composite_and_place_pixel(addr as usize, fill, transparent);
                }
            }
        }
    }

    fn draw_textured_triangle(&mut self, in_points: &[Point], transparent: bool) {
        // let mut sp = points.to_vec();
        // sp.sort_by_key(|p| p.y);

        // if sp[1].y == sp[2].y {
        //     self.draw_textured_flat_bottom_triangle(sp[0], sp[1], sp[2], transparent);
        // } else if sp[0].y == sp[1].y {
        //     self.draw_textured_flat_top_triangle(sp[0], sp[1], sp[2], transparent);
        // } else {
        //     let progress =
        //         sp[0].x + ((sp[1].y - sp[0].y) as f32 / (sp[2].y - sp[0].y) as f32) as i32;
        //     let bound_x = progress * ((sp[2].x - sp[0].x) as i32);
        //     let bound_point = Point {
        //         x: bound_x,
        //         y: sp[1].y,
        //         color: 0,
        //         tex_x: lerp_coords(sp[0].tex_x, sp[1].tex_x, sp[0].y, sp[1].y, progress),
        //         tex_y: lerp_coords(sp[0].tex_y, sp[1].tex_y, sp[0].y, sp[1].y, progress),
        //     };

        //     self.draw_textured_flat_bottom_triangle(sp[0], bound_point, sp[1], transparent);
        //     self.draw_textured_flat_top_triangle(sp[1], bound_point, sp[2], transparent);
        // }

        fn edge_function(a: &Point, b: &Point, c: &Vector2<i32>) -> isize {
            (c.x as isize - a.x as isize) * (b.y as isize - a.y as isize)
                - (c.y as isize - a.y as isize) * (b.x as isize - a.x as isize)
        }

        let points = sort_points_clockwise(&in_points);

        let min_x = points.iter().min_by_key(|v| v.x).unwrap().x;
        let max_x = points.iter().max_by_key(|v| v.x).unwrap().x;

        let min_y = points.iter().min_by_key(|v| v.y).unwrap().y;
        let max_y = points.iter().max_by_key(|v| v.y).unwrap().y;

        let area = edge_function(
            &points[0],
            &points[1],
            &Vector2::new(points[2].x, points[2].y),
        );

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let point = Vector2::new(x, y);
                let mut w0 = edge_function(&points[1], &points[2], &point) as f32;
                let mut w1 = edge_function(&points[2], &points[0], &point) as f32;
                let mut w2 = edge_function(&points[0], &points[1], &point) as f32;

                let addr = ((y as u32) * 1024) + x as u32;

                if !self.out_of_draw_area(&Point::from_components(x, y, 0))
                    && w0 <= 0.0
                    && w1 <= 0.0
                    && w2 <= 0.0
                {
                    w0 /= area as f32;
                    w1 /= area as f32;
                    w2 /= area as f32;

                    //println!("w1 {} w2 {} w3 {}", w0, w1, w2);

                    let tex_x = (w0 * points[0].tex_x as f32)
                        + (w1 * points[1].tex_x as f32)
                        + (w2 * points[2].tex_x as f32);
                    let tex_y = (w0 * points[0].tex_y as f32)
                        + (w1 * points[1].tex_y as f32)
                        + (w2 * points[2].tex_y as f32);

                    //println!("tex_x {} tex_y {}", tex_x, tex_y);

                    let fill = self.get_texel(tex_x as i32, tex_y as i32);

                    self.composite_and_place_pixel(addr as usize, fill, transparent);
                }
            }
        }
    }

    fn draw_solid_quad(&mut self, points: &[Point], fill: u16, transparent: bool) {
        self.draw_solid_triangle(&[points[0], points[2], points[1]], fill, transparent);
        self.draw_solid_triangle(&[points[1], points[2], points[3]], fill, transparent);
    }

    fn draw_shaded_quad(&mut self, points: &[Point], transparent: bool) {
        self.draw_shaded_triangle(&[points[0], points[2], points[1]], transparent);
        self.draw_shaded_triangle(&[points[1], points[2], points[3]], transparent);
    }

    fn draw_textured_quad(&mut self, points: &[Point], transparent: bool) {
        self.draw_textured_triangle(&[points[0], points[2], points[1]], transparent);
        self.draw_textured_triangle(&[points[1], points[2], points[3]], transparent);
    }

    fn get_texel(&self, x: i32, y: i32) -> u16 {
        //TODO inline variables. Just did this because I'm lazy
        let page_x = self.texpage_x_base as u32;
        let page_y = self.texpage_y_base as u32;
        let clut_x = self.palette_x as u32;
        let clut_y = self.palette_y as u32;
        let size = self.texmode;

        let pixel_val = match size {
            TextureColorMode::FifteenBit => {
                self.vram[min(
                    (point_to_address(
                        ((page_x * 64) as u32 + x as u32) as u32,
                        ((page_y * 256) as u32 + y as u32) as u32,
                    ) as usize),
                    524287,
                )]
            }
            TextureColorMode::EightBit => {
                let value = self.vram[min(
                    point_to_address(
                        (page_x * 64) as u32 + (x / 2) as u32,
                        (page_y * 256) as u32 + y as u32,
                    ) as usize,
                    524287,
                )];
                let clut_index = (value >> (x % 2) * 8) & 0xF;
                self.vram[min(
                    point_to_address((clut_x * 16 + clut_index as u32) as u32, clut_y as u32)
                        as usize,
                    524287,
                )]
            }
            TextureColorMode::FourBit => {
                let value = self.vram[min(
                    (point_to_address(
                        (page_x * 64) as u32 + (x / 4) as u32,
                        (page_y * 256) as u32 + y as u32,
                    ) as usize),
                    524287,
                )];
                let (clut_index, _) = value.overflowing_shr(((x % 4) * 4) as u32);
                self.vram[min(
                    point_to_address(
                        (clut_x * 16 + (clut_index & 0xF) as u32) as u32,
                        clut_y as u32,
                    ),
                    524287,
                ) as usize]
            }
        };
        if self.blend_enabled {
            pixel_val & self.blend_color
        } else {
            pixel_val
        }
        //pixel_val
    }
}

fn point_to_address(x: u32, y: u32) -> u32 {
    ((1024) as u32 * y).wrapping_add(x)
}

fn b24color_to_b15color(color: u32) -> u16 {
    let r = ((color >> 16) & 0xFF) / 8;
    let g = ((color >> 8) & 0xFF) / 8;
    let b = (color & 0xFF) / 8;
    ((r << 10) | (g << 5) | b) as u16
}

fn b15_to_rgb(color: u16) -> (u8, u8, u8) {
    (
        ((color >> 10) & 0x1F) as u8,
        ((color >> 5) & 0x1F) as u8,
        (color & 0x1F) as u8,
    )
}

fn rgb_to_b15(r: u8, g: u8, b: u8) -> u16 {
    ((clamp(r, 0, 0xFF) as u16) << 10)
        | ((clamp(g, 0, 0xFF) as u16) << 5)
        | (clamp(b, 0, 0xFF) as u16)
}

fn lerp_color(y0: u16, y1: u16, x0: i32, x1: i32, x: i32) -> u16 {
    let (sr, sg, sb) = b15_to_rgb(y0);
    let (er, eg, eb) = b15_to_rgb(y1);

    let ir = (sr as f32 + ((er as i32 - sr as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;
    let ig = (sg as f32 + ((eg as i32 - sg as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;
    let ib = (sb as f32 + ((eb as i32 - sb as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32)))
        as u16;

    rgb_to_b15(ir as u8, ig as u8, ib as u8)

    //(y0 as f32 + ((y1 - y0) as f32 * ((x - x0) as f32 / (x1 - x0) as f32))) as u16
}

fn lerp_coords(y0: i32, y1: i32, x0: i32, x1: i32, x: i32) -> i32 {
    (y0 as f32 + ((y1 as i32 - y0 as i32) as f32 * ((x - x0) as f32 / (x1 - x0) as f32))) as i32
}

enum BlendMode {
    B2F2, // B/2+F/2
    BAF,  // B+F
    BSF,  // B-F
    BF4,  // B+F/4
}

fn alpha_composite(background_color: u16, alpha_color: u16, mode: &BlendMode) -> u16 {
    let (b_r, b_g, b_b) = b15_to_rgb(background_color);
    let (a_r, a_g, a_b) = b15_to_rgb(alpha_color);

    match mode {
        BlendMode::B2F2 => rgb_to_b15(
            (a_r / 2) + (b_r / 2),
            (a_g / 2) + (b_g / 2),
            (a_b / 2) + (b_b / 2),
        ),
        BlendMode::BAF => rgb_to_b15(a_r + b_r, a_g + b_g, a_b + b_b),
        BlendMode::BSF => rgb_to_b15(a_r - b_r, a_g - b_g, a_b - b_b),
        BlendMode::BF4 => rgb_to_b15(a_r + (b_r / 4), a_g + (b_g / 4), a_b + (b_b / 4)),
    }
}

fn sort_points_clockwise(points: &[Point]) -> Vec<Point> {
    let center_x: i32 = points.iter().map(|p| p.x).sum::<i32>() / points.len() as i32;
    let center_y: i32 = points.iter().map(|p| p.y).sum::<i32>() / points.len() as i32;

    let center_point = Point::from_components(center_x, center_y, 0);

    let mut sorted_points = points.to_vec();
    sorted_points.sort_by(|a, b| sort_clockwise_big_match(a, b, &center_point));
    sorted_points
}

// Stolen from https://wapl.es/rust/2020/07/25/optimising-with-cmp-and-ordering.html
fn sort_clockwise_big_match(a: &Point, b: &Point, center: &Point) -> Ordering {
    let d_ax = a.x - center.x;
    let d_bx = b.x - center.x;

    let cmp_ax = d_ax.cmp(&0);
    let cmp_bx = d_bx.cmp(&0);

    match (cmp_ax, cmp_bx) {
        // d_ax >= 0 && d_bx < 0
        (Ordering::Greater, Ordering::Less) | (Ordering::Equal, Ordering::Less) => {
            Ordering::Greater
        }
        // d_ax < 0 && d_bx >= 0
        (Ordering::Less, Ordering::Greater) | (Ordering::Less, Ordering::Equal) => Ordering::Less,
        // d_ax == 0 && d_bx == 0
        (Ordering::Equal, Ordering::Equal) if a.y - center.y >= 0 || b.y - center.y >= 0 => {
            a.y.cmp(&b.y)
        }
        (Ordering::Equal, Ordering::Equal) => b.y.cmp(&a.y),
        _ => {
            // Compute the cross product of vectors (center -> a) x (center -> b)
            let det = (d_ax) * (b.y - center.y) - (d_bx) * (a.y - center.y);

            match det.cmp(&0) {
                Ordering::Less => Ordering::Greater,
                Ordering::Greater => Ordering::Less,
                Ordering::Equal => {
                    // Points a and b are on the same line from the center. Check which point is closer to
                    // the center.
                    let d1 = (d_ax) * (d_ax) + (a.y - center.y) * (a.y - center.y);
                    let d2 = (d_bx) * (d_bx) + (b.y - center.y) * (b.y - center.y);

                    d1.cmp(&d2)
                }
            }
        }
    }
}

//Helper trait + impl
trait Command {
    fn gp0_header(&self) -> u8;
    fn command(&self) -> u8;
    fn parameter(&self) -> u32;
}

impl Command for u32 {
    fn gp0_header(&self) -> u8 {
        ((self.clone() >> 29) & 0x7) as u8
    }

    fn command(&self) -> u8 {
        ((self.clone() >> 24) & 0xFF) as u8
    }

    fn parameter(&self) -> u32 {
        self.clone() & 0x7FFFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lerp_color() {
        assert_eq!(15, lerp_color(10, 20, 100, 200, 150));
    }

    #[test]
    fn test_lerp_color_negative() {
        assert_eq!(15, lerp_color(20, 10, 100, 200, 150));
    }
}
