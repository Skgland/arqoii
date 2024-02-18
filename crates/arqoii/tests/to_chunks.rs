use arqoii::Pixel;
use arqoii_types::{QoiChannels, QoiColorSpace, QoiHeader};

#[test]
fn dice() {
    transcode("qoi/dice", None);
}

#[test]
fn edgecase() {
    transcode(
        "qoi/edgecase",
        Some(QoiHeader::new(
            256,
            64,
            QoiChannels::Rgba,
            QoiColorSpace::SRgbWithLinearAlpha,
        )),
    );
}

#[test]
fn kodim10() {
    transcode("qoi/kodim10", None);
}

#[test]
fn kodim23() {
    transcode("qoi/kodim23", None);
}

#[test]
fn qoi_logo() {
    transcode("qoi/qoi_logo", None);
}

#[test]
fn testcard_rgba() {
    transcode("qoi/testcard_rgba", None);
}

#[test]
fn testcard() {
    transcode("qoi/testcard", None);
}

#[test]
fn wikipedia_008() {
    transcode("qoi/wikipedia_008", None);
}

fn transcode(name: &str, _alt_header: Option<QoiHeader>) {
    let reference_qoi = std::fs::read(format!("tests/test-images/{name}.qoi")).unwrap();
    let png_bytes = std::fs::read(format!("tests/test-images/{name}.png")).unwrap();

    let (_info, reference_px) = load_png(&png_bytes);

    let encoder = arqoii::QoiChunkEncoder::new(reference_px.into_iter());
    let decoder = arqoii::QoiChunkDecoder::new(reference_qoi[14..].iter().cloned());

    assert!(Iterator::eq(encoder, decoder));
}
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
