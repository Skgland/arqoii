use core::iter::FusedIterator;

use arqoii_types::QOI_FOOTER;

use crate::types::{CoderState, Pixel, QoiChunk, QoiHeader};

/// An encoder for encoding Pixels into Chunks
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
        // we try to encode using these priorities:
        // - fewest bytes
        // - simplest: previous_pixel > index lookup > calculation
        //
        // this results in this ordering:
        // 1. Run    1-byte  / 1..=62 pixel, copy previous_pixel
        //
        // 2. Index  1-byte  / pixel       , copy from index
        // 3. Diff   1-byte  / pixel       , calculation based on previous_pixel
        //
        // 4. Luma   2-bytes / pixel       , calculation based on previous_pixel
        // 5. Rgb    4-bytes / pixel       , alpha based on previous_pixel
        // 6. Rgba   5-bytes / pixel

        let pixel = loop {
            let Some(pixel) = self.peek.take().or_else(|| self.pixel.next()) else {
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
                    // we don't need to update the index or the previous pixel as we are on a run
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
            // we can't use a run so we won't violate the standard which states:
            // > A valid encoder must not issue 2 or more consecutive QOI_OP_INDEX
            // > chunks to the same index. QOI_OP_RUN should be used instead.

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

/// An encoder used to turn a Qoi Format File Header and Pixels into bytes
pub struct QoiEncoder<I: Iterator<Item = Pixel>> {
    header_bytes: core::array::IntoIter<u8, 14>,
    chunks: core::iter::Flatten<QoiChunkEncoder<I>>,
    footer_bytes: core::array::IntoIter<u8, 8>,
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
    #[doc(alias = "save")]
    pub fn new(header: QoiHeader, pixels: I) -> Self {
        Self {
            chunks: QoiChunkEncoder::new(pixels).flatten(),
            header_bytes: header.to_bytes().into_iter(),
            footer_bytes: QOI_FOOTER.into_iter(),
        }
    }
}

impl<I> FusedIterator for QoiEncoder<I>
where
    I: Iterator<Item = Pixel>,
    QoiEncoder<I>: Iterator,
    QoiChunkEncoder<I>: FusedIterator,
{
}

impl<I: Iterator<Item = Pixel>> Iterator for QoiEncoder<I> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.header_bytes.next()
            .or_else(||self.chunks.next())
            .or_else(||self.footer_bytes.next())
    }
}
