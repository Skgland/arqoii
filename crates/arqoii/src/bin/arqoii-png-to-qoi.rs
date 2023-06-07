fn main() {
    for arg in std::env::args().skip(1) {
        let src: &Path = arg.as_ref();
        if src.extension() == Some("png".as_ref()) {
            let dest = src.with_extension("qoi");
            transcode(src, &dest);
        }
    }
}

fn transcode(src: &Path, dest: &Path) {
    let png_bytes = std::fs::read(src).unwrap();
    let (info, png_px) = load_png(&png_bytes);

    let header = QoiHeader::new(
        info.width,
        info.height,
        match info.color_type {
            png::ColorType::Grayscale | png::ColorType::Rgb => QoiChannels::Rgb,
            png::ColorType::Indexed => todo!(),
            png::ColorType::GrayscaleAlpha | png::ColorType::Rgba => QoiChannels::Rgba,
        },
        QoiColorSpace::SRgbWithLinearAlpha,
    );

    let qoi = QoiEncoder::new(header, png_px.into_iter()).collect::<Vec<_>>();
    std::fs::write(dest, qoi).unwrap();
}

use std::path::Path;

use arqoii::{Pixel, QoiEncoder};
use arqoii_types::{QoiChannels, QoiColorSpace, QoiHeader};
use png::OutputInfo;

fn load_png(data: &[u8]) -> (OutputInfo, Vec<Pixel>) {
    let mut result = vec![];

    // The decoder is a build for reader and can be used to set various decoding options
    // via `Transformations`. The default output transformation is `Transformations::IDENTITY`.
    let decoder = png::Decoder::new(data);
    let mut reader = decoder.read_info().unwrap();
    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    let info = reader.next_frame(&mut buf).unwrap();
    // Grab the bytes of the image.
    let bytes = &buf[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Grayscale | png::ColorType::Rgb => {
            for px in bytes.chunks(3) {
                if let [r, g, b] = px {
                    result.push(Pixel {
                        r: *r,
                        b: *b,
                        g: *g,
                        a: 255,
                    });
                } else {
                    panic!()
                }
            }
        }
        png::ColorType::Indexed => todo!(),
        png::ColorType::GrayscaleAlpha | png::ColorType::Rgba => {
            for px in bytes.chunks(4) {
                if let [r, g, b, a] = px {
                    result.push(Pixel {
                        r: *r,
                        b: *b,
                        g: *g,
                        a: *a,
                    });
                } else {
                    panic!()
                }
            }
        }
    }

    (info, result)
}
