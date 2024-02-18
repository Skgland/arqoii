use arqoii_types::QOI_MAGIC;

use crate::iterator_helper::PeekN;
use crate::types::{
    CoderState, Pixel, QoiChannels, QoiChunk, QoiColorSpace, QoiHeader, QOI_FOOTER,
};

/// A decoder for decoding bytes into qoi chunks
///
/// Expects the data to not include the header
pub struct QoiChunkDecoder<I> {
    bytes: PeekN<7, I, u8>,
}

impl<I> QoiChunkDecoder<I> {
    pub fn new(iter: I) -> QoiChunkDecoder<I>
    where
        I: Iterator<Item = u8>,
    {
        Self {
            bytes: PeekN::new(iter),
        }
    }
}

impl<I: Iterator<Item = u8>> Iterator for QoiChunkDecoder<I> {
    type Item = QoiChunk;
    fn next(&mut self) -> Option<Self::Item> {
        let init = self.bytes.next()?;

        if init == 0b11111111 {
            // rgba
            let r = self.bytes.next()?;
            let g = self.bytes.next()?;
            let b = self.bytes.next()?;
            let a = self.bytes.next()?;
            Some(QoiChunk::new_rgba(r, g, b, a))
        } else if init == 0b11111110 {
            // rgb
            let r = self.bytes.next()?;
            let g = self.bytes.next()?;
            let b = self.bytes.next()?;
            Some(QoiChunk::new_rgb(r, g, b))
        } else {
            let short = init >> 6;
            if short == 0b00 {
                // index
                if init == 0 {
                    if let Some(peek) = self.bytes.peek() {
                        if QOI_FOOTER[1..] == peek.map(|elem| *elem) {
                            // we are done, init is the start of the footer
                            // note: this means that this is not a fused iterator
                            return None;
                        }
                    }
                }

                Some(QoiChunk::new_index(init & 0b00111111))
            } else if short == 0b01 {
                // diff
                Some(QoiChunk::new_diff(
                    ((init >> 4) & 0b00000011) as i8 - 2,
                    ((init >> 2) & 0b00000011) as i8 - 2,
                    (init & 0b00000011) as i8 - 2,
                ))
            } else if short == 0b10 {
                // luma
                let next = self.bytes.next()?;
                Some(QoiChunk::new_luma(
                    (init & 0b00111111) as i8 - 32,
                    ((next >> 4) & 0b00001111) as i8 - 8,
                    (next & 0b00001111) as i8 - 8,
                ))
            } else {
                debug_assert_eq!(short, 0b11);
                // run
                Some(QoiChunk::new_run((init & 0b00111111) + 1))
            }
        }
    }
}

/// A decoder for decoding a qoi from bytes into pixels
///
/// Note: this does not check that decoded pixel count matches the width * height from the header
/// If the data does not represent a valid qoi format file you may get fewer or more pixels than expect
pub struct QoiDecoder<I> {
    state: CoderState,
    chunks: QoiChunkDecoder<I>,
}

impl<I: Iterator<Item = u8>> QoiDecoder<I> {
    #[doc(alias = "load")]
    pub fn new(mut iter: I) -> Option<(QoiHeader, Self)> {
        let magic = [iter.next()?, iter.next()?, iter.next()?, iter.next()?];

        if magic != QOI_MAGIC {
            return None;
        }

        let width = u32::from_be_bytes([iter.next()?, iter.next()?, iter.next()?, iter.next()?]);
        let height = u32::from_be_bytes([iter.next()?, iter.next()?, iter.next()?, iter.next()?]);
        let channels = match iter.next()? {
            3 => QoiChannels::Rgb,
            4 => QoiChannels::Rgba,
            _ => return None,
        };
        let color_space = match iter.next()? {
            0 => QoiColorSpace::SRgbWithLinearAlpha,
            1 => QoiColorSpace::AllChannelsLinear,
            _ => return None,
        };

        Some((
            QoiHeader::new(width, height, channels, color_space),
            Self {
                state: CoderState::default(),
                chunks: QoiChunkDecoder::new(iter),
            },
        ))
    }
}

impl<I> Iterator for QoiDecoder<I>
where
    QoiChunkDecoder<I>: Iterator<Item = QoiChunk>,
{
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        if self.state.run > 0 {
            self.state.run -= 1;
            Some(self.state.previous.clone())
        } else {
            let chunk = self.chunks.next()?;

            match chunk {
                QoiChunk::Rgb { r, g, b, .. } => {
                    let next = Pixel {
                        r,
                        g,
                        b,
                        a: self.state.previous.a,
                    };
                    self.state.previous = next.clone();
                    self.state.index[next.pixel_hash() as usize] = next.clone();
                    Some(next)
                }
                QoiChunk::Rgba { r, g, b, a, .. } => {
                    let next = Pixel { r, g, b, a };
                    self.state.previous = next.clone();
                    self.state.index[next.pixel_hash() as usize] = next.clone();
                    Some(next)
                }
                QoiChunk::Index { idx, .. } => {
                    let next = self.state.index[idx as usize].clone();
                    self.state.previous = next.clone();
                    Some(next)
                }
                QoiChunk::Diff { dr, dg, db, .. } => {
                    let next = Pixel {
                        r: self.state.previous.r.wrapping_add_signed(dr),
                        g: self.state.previous.g.wrapping_add_signed(dg),
                        b: self.state.previous.b.wrapping_add_signed(db),
                        a: self.state.previous.a,
                    };
                    self.state.previous = next.clone();
                    self.state.index[next.pixel_hash() as usize] = next.clone();
                    Some(next)
                }
                QoiChunk::Luma {
                    dg, dr_dg, db_dg, ..
                } => {
                    let next = Pixel {
                        r: self.state.previous.r.wrapping_add_signed(dr_dg + dg),
                        g: self.state.previous.g.wrapping_add_signed(dg),
                        b: self.state.previous.b.wrapping_add_signed(db_dg + dg),
                        a: self.state.previous.a,
                    };
                    self.state.previous = next.clone();
                    self.state.index[next.pixel_hash() as usize] = next.clone();
                    Some(next)
                }
                QoiChunk::Run { run, .. } => {
                    let next = self.state.previous.clone();
                    self.state.run = run - 1;
                    self.state.index[next.pixel_hash() as usize] = next.clone();
                    Some(next)
                }
            }
        }
    }
}
