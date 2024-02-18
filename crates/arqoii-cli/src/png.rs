use std::{
    io::{BufWriter, Write},
    path::Path,
};

use arqoii::types::{Pixel, QoiChannels};
use png::Transformations;

pub fn load(data: &[u8]) -> (QoiChannels, (u32, u32), Vec<Vec<Pixel>>) {
    let mut channels = QoiChannels::Rgb;

    // The decoder is a build for reader and can be used to set various decoding options
    // via `Transformations`. The default output transformation is `Transformations::IDENTITY`.
    let mut decoder = png::Decoder::new(data);
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);

    let mut reader = decoder.read_info().unwrap();

    let buffer_size = reader.output_buffer_size();

    // Allocate the output buffer.
    let mut buf = vec![0; buffer_size];

    let mut frames = vec![];

    let (width, height) = reader.info().size();
    let pixel_count = width as usize * height as usize;

    // Read the next frame. An APNG might contain multiple frames.
    while let Ok(info) = reader.next_frame(&mut buf) {
        let mut frame = Vec::with_capacity(pixel_count);

        // Grab the bytes of the image.
        let bytes = &buf[..info.buffer_size()];
        match info.color_type {
            png::ColorType::Grayscale => {
                for px in bytes {
                    // TODO grayscale to rgb isn't 1:1:1
                    frame.push(Pixel {
                        r: *px,
                        b: *px,
                        g: *px,
                        a: 255,
                    });
                }
            }
            png::ColorType::Rgb => {
                for px in bytes.chunks(3) {
                    if let [r, g, b] = px {
                        frame.push(Pixel {
                            r: *r,
                            b: *b,
                            g: *g,
                            a: 255,
                        });
                    } else {
                        panic!("image data of an rgb png was not a multiple of 3 bytes")
                    }
                }
            }
            png::ColorType::Indexed => {
                unreachable!("image should have been expanded")
            }
            png::ColorType::GrayscaleAlpha => {
                for px in bytes.chunks(2) {
                    if let [c, a] = px {
                        frame.push(Pixel {
                            r: *c,
                            b: *c,
                            g: *c,
                            a: *a,
                        });
                        if *a != 255 {
                            channels = QoiChannels::Rgba
                        }
                    } else {
                        panic!("image data of an grayscale alpha png was not a multiple of 2 bytes")
                    }
                }
            }
            png::ColorType::Rgba => {
                for px in bytes.chunks(4) {
                    if let [r, g, b, a] = px {
                        frame.push(Pixel {
                            r: *r,
                            b: *b,
                            g: *g,
                            a: *a,
                        });
                        if *a != 255 {
                            channels = QoiChannels::Rgba
                        }
                    } else {
                        panic!("image data of an rgba png was not a multiple of 4 bytes")
                    }
                }
            }
        }
        frames.push(frame);
    }

    (channels, (width, height), frames)
}

pub(crate) fn save(
    channels: QoiChannels,
    (width, height): (u32, u32),
    pixels: &[Pixel],
    dest: &Path,
) {
    let mut file = std::fs::File::create(dest).unwrap();
    let mut buf_writer = BufWriter::new(&mut file);

    let mut encoder = png::Encoder::new(&mut buf_writer, width, height);
    encoder.set_color(match channels {
        QoiChannels::Rgb => png::ColorType::Rgb,
        QoiChannels::Rgba => png::ColorType::Rgba,
    });
    encoder.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let mut writer = encoder.write_header().unwrap();

    let data = pixels
        .iter()
        .flat_map(|px| match channels {
            QoiChannels::Rgb => vec![px.r, px.g, px.b],
            QoiChannels::Rgba => vec![px.r, px.g, px.b, px.a],
        })
        .collect::<Vec<_>>();

    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    buf_writer.flush().unwrap();
    drop(buf_writer);
    file.flush().unwrap();
    file.sync_data().unwrap();
}
