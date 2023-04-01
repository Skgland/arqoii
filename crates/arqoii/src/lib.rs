#![no_std]

use core::{default::Default, iter::FusedIterator, mem::MaybeUninit};

pub use arqoii_types as types;
pub use arqoii_types::{QOI_FOOTER, QOI_MAGIC};

use types::{ChunkBuf, QoiChunk, QoiHeader};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub const ZERO: Self = Pixel {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub fn pixel_hash(&self) -> u8 {
        (((self.r as usize) * 3
            + (self.g as usize) * 5
            + (self.b as usize) * 7
            + (self.a as usize) * 11)
            % 64) as u8
    }
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

struct CoderState {
    previous: Pixel,
    index: [Pixel; 64],
    run: u8,
}

impl Default for CoderState {
    fn default() -> Self {
        Self {
            previous: Pixel::default(),
            index: [Pixel::ZERO; 64],
            run: 0,
        }
    }
}

pub struct QoiChunkEncoder<I> {
    state: CoderState,
    pixel: I,
    peek: Option<Pixel>,
}

impl<I> QoiChunkEncoder<I> {
    pub fn new(pixel: I) -> Self {
        Self {
            state: CoderState::default(),
            pixel,
            peek: None,
        }
    }
}

impl<I: Iterator<Item = Pixel>> Iterator for QoiChunkEncoder<I> {
    type Item = QoiChunk;

    fn next(&mut self) -> Option<Self::Item> {
        let pixel = loop {
            let Some(pixel) = self.peek.take().or_else(||self.pixel.next()) else {
                // end of input pixels
                // check if we have an in progress run
                return if self.state.run > 0 {
                    let run = QoiChunk::new_run(self.state.run);
                    self.state.run = 0;
                    Some(run)
                } else {
                    None
                };
            };

            if pixel == self.state.previous {
                self.state.run += 1;
                if self.state.run == 62 {
                    // reached max run write return it and rest run
                    self.state.run = 0;
                    // we don't need to update the index or the previous  pixel as we are on a run
                    // and as such the pixel preceding the run has already set both correctly
                    return Some(QoiChunk::new_run(62));
                }

                if self.state.run == 1 {
                    // if the first image pixel is r: 0, g: 0, b: 0, a:255
                    // we will still need to update the index as the index is 0 initialized

                    let idx = pixel.pixel_hash();
                    self.state.index[idx as usize] = pixel;
                }
                // updated the we don't know if this the end of a run yet,
                // we only know that on the pixel after the run or on the end of the pixels
                continue;
            } else {
                break pixel;
            }
        };

        // end of run
        if self.state.run > 0 {
            // clear out current run
            self.peek = Some(pixel);
            let next = QoiChunk::new_run(self.state.run);
            self.state.run = 0;
            return Some(next);
        }

        let idx = pixel.pixel_hash();

        let chunk = if self.state.index[idx as usize] == pixel {
            // we have a matching index so use that
            QoiChunk::new_index(idx)
        } else if pixel.a == self.state.previous.a {
            // old_{r,g,b} + d{r,g,b} = new_{r,g,b}
            // d{r,g,b} = new_{r,g,b} - old_{r,g,b}

            let dr = pixel.r.wrapping_sub(self.state.previous.r) as i8;
            let dg = pixel.g.wrapping_sub(self.state.previous.g) as i8;
            let db = pixel.b.wrapping_sub(self.state.previous.b) as i8;

            if (-2..=1).contains(&dr) && (-2..=1).contains(&dg) && (-2..=1).contains(&db) {
                // we can encode it as a diff op so use that
                QoiChunk::new_diff(dr, dg, db)
            } else {
                let dr_dg = dr.wrapping_sub(dg);
                let db_dg = db.wrapping_sub(dg);

                if (-32..=31).contains(&dg)
                    && (-8..=7).contains(&dr_dg)
                    && (-8..=7).contains(&db_dg)
                {
                    // luma encoding is possible so use that
                    QoiChunk::new_luma(dg, dr_dg, db_dg)
                } else {
                    // fallback to rgb as we already checked that alpha matches
                    QoiChunk::new_rgb(pixel.r, pixel.g, pixel.b)
                }
            }
        } else {
            // no run, no index match and different alpha, so we need to fallback to rgba
            QoiChunk::new_rgba(pixel.r, pixel.g, pixel.b, pixel.a)
        };

        self.state.index[idx as usize] = pixel.clone();
        self.state.previous = pixel;
        Some(chunk)
    }
}

impl<I> FusedIterator for QoiChunkEncoder<I>
where
    QoiChunkEncoder<I>: Iterator,
    I: FusedIterator,
{
}

pub struct QoiEncoder<I> {
    chunks: QoiChunkEncoder<I>,
    header_bytes: [u8; 14],
    header_offset: usize,
    buf: ChunkBuf,
    footer_offset: usize,
}
impl<I> QoiEncoder<I>
where
    I: Iterator<Item = Pixel>,
{
    /// Create a new streaming Qoi Encoder
    ///
    /// # Note
    /// the encoder will not stop after width * height pixels on its own!
    /// ensure that the iterator results in the right amount of pixel or the resulting image will be malformed!
    pub fn new(header: QoiHeader, pixels: I) -> Self {
        Self {
            chunks: QoiChunkEncoder::new(pixels),
            header_bytes: header.to_bytes(),
            header_offset: 0,
            buf: ChunkBuf::new(),
            footer_offset: 0,
        }
    }
}

impl<I> FusedIterator for QoiEncoder<I>
where
    QoiEncoder<I>: Iterator,
    QoiChunkEncoder<I>: FusedIterator,
{
}


impl<I: Iterator<Item = Pixel>> Iterator for QoiEncoder<I> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.header_offset < self.header_bytes.len() {
            // still have header left so return the next header byte
            let next = self.header_bytes[self.header_offset];
            self.header_offset += 1;
            Some(next)
        } else if let Some(next) = self.buf.pop() {
            // current chunk has bytes left so return those
            Some(next)
        } else if let Some(chunk) = self.chunks.next() {
            // we have a next chunk, so turn it into bytes and return the first byte of those
            chunk.write_to_chunk_buffer(&mut self.buf);
            let next = self.buf.pop();
            debug_assert!(next.is_some());
            next
        } else if self.footer_offset < QOI_FOOTER.len() {
            // we still have footer left so return the next footer byte
            let next = QOI_FOOTER[self.footer_offset];
            self.footer_offset += 1;
            Some(next)
        } else {
            // done
            None
        }
    }
}

struct PeekN<const N: usize, I, Item> {
    iter: I,
    peek: [Option<Item>; N],
}

impl<const N: usize, I, Item> PeekN<N, I, Item> {
    fn new(iter: I) -> Self {
        Self {
            iter,
            peek: [(); N].map(|_| None),
        }
    }

    fn peek(&mut self) -> Option<[&Item; N]>
    where
        I: Iterator<Item = Item>,
    {

        // rotate the first remaining peek value to the front
        let rotate = self.peek.iter().enumerate().find_map(|(idx, elem)| elem.is_some().then_some(idx)).unwrap_or(0);
        self.peek.rotate_left(rotate);

        let mut peek = [();N] .map(|_| MaybeUninit::uninit());
        let mut count = 0;

        for elem in self.peek
            .iter_mut()
            .flat_map(|elem| {
                if elem.is_none() {
                    *elem = self.iter.next();
                }
                elem.as_ref()
            }) {
                peek[count].write(elem);
                count += 1;
        }

        if count == N {
            // Safety count is N and as such all indices 0 to N - 1 have been written to
            Some(unsafe { core::mem::transmute_copy::<[MaybeUninit<&Item>;N], [&Item;N]>(&peek) })
        } else {
            None
        }
    }
}

impl<const N: usize, I: Iterator> Iterator for PeekN<N, I, I::Item> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.peek
            .iter_mut()
            .find_map(|elem| elem.take())
            .or_else(|| self.iter.next())
    }
}

impl<const N: usize, I, Item> FusedIterator for PeekN<N, I, Item>
where
    I: FusedIterator,
    PeekN<N, I, Item>: Iterator,
{
}

pub struct QoiChunkDecoder<I> {
    bytes: PeekN<7, I, u8>,
}
impl<I> QoiChunkDecoder<I> {
    pub fn new(iter: I) -> QoiChunkDecoder<I> where I: Iterator<Item = u8> {
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
                        if QOI_FOOTER[1..] == peek.map(|elem|*elem) {
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

pub struct QoiDecoder<I> {
    state: CoderState,
    chunks: QoiChunkDecoder<I>,
}

impl<I: Iterator<Item = u8>> QoiDecoder<I> {
    pub fn new(mut iter: I) -> Option<(QoiHeader, Self)> {
        let magic = [iter.next()?,iter.next()?,iter.next()?,iter.next()?];

        if magic != QOI_MAGIC {
            return None;
        }

        let width = u32::from_be_bytes([iter.next()?,iter.next()?,iter.next()?,iter.next()?]);
        let height = u32::from_be_bytes([iter.next()?,iter.next()?,iter.next()?,iter.next()?]);
        let channels = match iter.next()? {
            3 => types::QoiChannels::Rgb,
            4 => types::QoiChannels::Rgba,
            _ => return None,
        };
        let color_space = match iter.next()? {
            0 => types::QoiColorSpace::SRgbWithLinearAlpha,
            1 => types::QoiColorSpace::AllChannelsLinear,
            _ => return None,
        };

        Some((QoiHeader::new(width, height, channels, color_space), Self {
            state: CoderState::default(),
            chunks: QoiChunkDecoder::new(iter),
        }))
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
