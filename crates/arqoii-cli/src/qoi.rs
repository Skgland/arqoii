use std::path::Path;

use arqoii::{Pixel, QoiDecoder, QoiEncoder};
use arqoii_types::{QoiChannels, QoiColorSpace, QoiHeader};

pub fn save(channels: QoiChannels, (width, height): (u32, u32), px: &[Pixel], dest: &Path) {
    let header = QoiHeader::new(width, height, channels, QoiColorSpace::SRgbWithLinearAlpha);
    let qoi = QoiEncoder::new(header, px.iter().cloned()).collect::<Vec<_>>();

    std::fs::write(dest, qoi).unwrap();
}

pub fn load(data: &[u8]) -> (QoiHeader, Vec<Pixel>) {
    let (header, pixel) = QoiDecoder::new(data.iter().copied()).unwrap();
    (header, pixel.collect())
}
