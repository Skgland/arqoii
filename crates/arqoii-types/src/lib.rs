#![no_std]

const QOI_MAGIC: [u8; 4] = *b"qoif";

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum QoiChannels {
    Rgb = 3,
    Rgba = 4,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum QoiColorSpace {
    SRgbWithLinearAlpha = 0,
    AllChannelsLinear = 1,
}

#[derive(Debug)]
pub struct QoiHeader {
    pub width: u32,
    pub height: u32,
    pub channels: QoiChannels,
    pub color_space: QoiColorSpace,
}

impl QoiHeader {
    pub fn new(width: u32, height: u32, channels: QoiChannels, color_space: QoiColorSpace) -> Self {
        Self {
            width,
            height,
            channels,
            color_space,
        }
    }

    pub fn to_bytes(&self) -> [u8; 14] {
        let mut bytes = [0; 14];

        for (i, &b) in QOI_MAGIC.iter().enumerate() {
            bytes[i] = b;
        }

        for (i, b) in self.width.to_be_bytes().into_iter().enumerate() {
            bytes[i + QOI_MAGIC.len()] = b;
        }

        for (i, b) in self.height.to_be_bytes().into_iter().enumerate() {
            bytes[i + QOI_MAGIC.len() + (u32::BITS / 8) as usize] = b;
        }

        bytes[QOI_MAGIC.len() + 2 * (u32::BITS / 8) as usize] = self.channels.clone() as u8;
        bytes[QOI_MAGIC.len() + 2 * (u32::BITS / 8) as usize + 1] = self.color_space.clone() as u8;

        bytes
    }
}

#[derive(Debug, Clone)]
pub enum QoiChunk {
    #[non_exhaustive]
    Rgb { r: u8, g: u8, b: u8 },
    #[non_exhaustive]
    Rgba { r: u8, g: u8, b: u8, a: u8 },
    #[non_exhaustive]
    Index { idx: u8 /* u6 */ },
    #[non_exhaustive]
    Diff {
        dr: u8, /* u2 */
        dg: u8, /* u2 */
        db: u8, /* u2 */
    },
    #[non_exhaustive]
    Luma {
        dg: u8,    /* u6 */
        dr_dg: u8, /* u4 */
        db_dg: u8, /* u4 */
    },
    #[non_exhaustive]
    Run { run: u8, /* u6, except 63 & 64*/ },
}

impl QoiChunk {
    pub fn new_run(run: u8) -> Self {
        debug_assert!(run <= 62);
        Self::Run { run: run - 1 }
    }

    pub fn new_index(idx: u8) -> Self {
        debug_assert!(idx <= 63);
        Self::Index { idx }
    }

    pub fn new_diff(dr: i8, dg: i8, db: i8) -> Self {
        debug_assert!((-2..=1).contains(&dr));
        debug_assert!((-2..=1).contains(&dg));
        debug_assert!((-2..=1).contains(&db));

        Self::Diff {
            dr: (dr + 2) as u8,
            dg: (dg + 2) as u8,
            db: (db + 2) as u8,
        }
    }

    pub fn new_luma(dg: i8, dr_dg: i8, db_dg: i8) -> Self {
        debug_assert!((-32..=31).contains(&dg));
        debug_assert!((-8..=7).contains(&dr_dg));
        debug_assert!((-8..=7).contains(&db_dg));

        Self::Luma {
            dg: (dg + 32) as u8,
            dr_dg: (dr_dg + 8) as u8,
            db_dg: (db_dg + 8) as u8,
        }
    }

    pub fn new_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb { r, g, b }
    }
    pub fn new_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::Rgba { r, g, b, a }
    }

    pub fn write_to_chunk_buffer(&self, buf: &mut ChunkBuf) {
        match self.clone() {
            QoiChunk::Rgb { r, g, b } => {
                // [0b11111110] r g b
                buf.set([0b11111110, r, g, b])
            }
            QoiChunk::Rgba { r, g, b, a } => {
                // [0b11111111] r g b a
                buf.set([0b11111111, r, g, b, a])
            }
            QoiChunk::Index { idx } => {
                // [ 0 0  idx idx idx idx idx idx]
                buf.set([0b00111111 & idx])
            }
            QoiChunk::Diff { dr, dg, db } => {
                // [ 0 1 dr dr dg dg db db]
                buf.set([
                    0b01000000 | (0b00111111 & ((0b11 & dr) << 4 | (0b11 & dg) << 2 | (0b11 & db)))
                ])
            }
            QoiChunk::Luma { dg, dr_dg, db_dg } => {
                // [ 1 0 dg dg dg dg dg dg] [ dr_dg dr_dg dr_dg dr_dg db_dg db_dg db_dg db_dg ]
                buf.set([
                    0b10000000 | (0b00111111 & dg),
                    (0b1111 & dr_dg) << 4 | (0b1111 & db_dg),
                ])
            }
            QoiChunk::Run { run } => {
                // [ 1 1 run run run run run run ]
                // Note: [ 1 1 1 1 1 1 1 1 ] & [ 1 1 1 1 1 1 1 0 ] are invalid here

                debug_assert_ne!((run & 0b00111111), 0b00111111);
                debug_assert_ne!((run & 0b00111111), 0b00111110);

                buf.set([0b11000000 | (0b00111111 & run)]);
            }
        }
    }
}

pub struct ChunkBuf {
    data: [u8; 5],
    len: usize,
    offset: usize,
}

trait ChunkData {}

impl ChunkData for [u8; 1] {}
impl ChunkData for [u8; 2] {}
impl ChunkData for [u8; 3] {}
impl ChunkData for [u8; 4] {}
impl ChunkData for [u8; 5] {}

impl ChunkBuf {
    pub fn new() -> Self {
        ChunkBuf {
            data: [0; 5],
            len: 0,
            offset: 0,
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.offset < self.len {
            let res = self.data[self.offset];
            self.offset += 1;
            Some(res)
        } else {
            None
        }
    }

    fn set<const N: usize>(&mut self, data: [u8; N])
    where
        [u8; N]: ChunkData,
    {
        (0..N).for_each(|i| {
            self.data[i] = data[i];
        });
        self.offset = 0;
        self.len = N;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[0..self.len]
    }
}

impl Default for ChunkBuf {
    fn default() -> Self {
        Self::new()
    }
}
