
use qoif_rs::Pixel;
use qoif_types::QuiHeader;

#[cfg(feature = "std")]
#[test]
fn dice() {
    transcode("dice", None);
}

#[cfg(feature = "std")]
#[test]
fn edgecase() {
    transcode("edgecase", Some(QuiHeader::new(256,64, qoif_types::QuiChannels::Rgba, qoif_types::QuiColorSpace::SRgbWithLinearAlpha)));
}

#[cfg(feature = "std")]
#[test]
fn kodim10() {
    transcode("kodim10", None);
}

#[cfg(feature = "std")]
#[test]
fn kodim23() {
    transcode("kodim23", None);
}

#[cfg(feature = "std")]
#[test]
fn qoi_logo() {
    transcode("qoi_logo", None);
}

#[cfg(feature = "std")]
#[test]
fn testcard_rgba() {
    transcode("testcard_rgba", None);
}

#[cfg(feature = "std")]
#[test]
fn testcard() {
    transcode("testcard", None);
}

#[cfg(feature = "std")]
#[test]
fn wikipedia_008() {
    transcode("wikipedia_008", None);
}

#[cfg(feature = "std")]
fn transcode(name: &str, alt_header: Option<QuiHeader>) {
    let png_bytes = std::fs::read(format!("tests/inputs/{name}.png")).unwrap();
    let qui_bytes = std::fs::read(format!("tests/expected-outputs/{name}.qoi")).unwrap();

    let (info, px) = load_png(&png_bytes);
    let mut out: Vec<u8> = vec![];
    qoif_rs::encode(
        alt_header.unwrap_or_else(||
            QuiHeader::new(
                info.width,
                info.height,
                match info.color_type {
                    png::ColorType::Grayscale | png::ColorType::Rgb => qoif_types::QuiChannels::Rgb,
                    png::ColorType::Indexed => todo!(),
                    png::ColorType::GrayscaleAlpha | png::ColorType::Rgba => {
                        qoif_types::QuiChannels::Rgba
                    }
                },
                qoif_types::QuiColorSpace::SRgbWithLinearAlpha,
            )),
        px,
        &mut out,
    )
    .unwrap();

    assert_eq!(out[0..14], qui_bytes[0..14], "Header should be equal!");
    let found = std::iter::zip(out.iter(), qui_bytes.iter())
        .enumerate()
        .find(|(_, (l, r))| l != r);
    assert_eq!(found, None, "Should not differ!");
    assert_eq!(out.len(), qui_bytes.len(), "Should have the same length");
}
use png::OutputInfo;

#[cfg(feature = "std")]
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
