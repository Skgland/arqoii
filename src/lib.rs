#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{default::Default, iter::{IntoIterator, FusedIterator}, result::Result::Ok};

#[cfg(feature = "std")]
use std::io::Write;

pub use qoif_types as types;

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
    fn new(pixel: I) -> Self {
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


impl<I> FusedIterator for QoiChunkEncoder<I> where QoiChunkEncoder<I>: Iterator, I: FusedIterator {}

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

impl<I> FusedIterator for QoiEncoder<I> where QoiEncoder<I>: Iterator, QoiChunkEncoder<I>: FusedIterator {}

pub const QOI_FOOTER: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

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

#[cfg(feature = "std")]
pub fn encode<I, W: Write>(
    header: QoiHeader,
    pixels: I,
    writer: &mut W,
) -> Result<(), std::io::Error>
where
    I: IntoIterator<Item = Pixel>,
{
    let size = (header.width as u64) * (header.height as u64);

    let data: Vec<_> =
        QoiEncoder::new(header, pixels.into_iter().take(size as usize).fuse()).collect();

    writer.write_all(&data)?;

    Ok(())
}
