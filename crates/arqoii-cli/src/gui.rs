use eframe::{
    egui::{
        self,
        load::{ImageLoader, LoadError},
    },
    epaint::{ahash::HashMap, mutex::RwLock, ColorImage},
};
use std::{path::PathBuf, sync::Arc};

struct ArqoiiViewer {
    image_paths: Vec<PathBuf>,
}

struct QoiLoader {
    cache: RwLock<HashMap<String, Arc<ColorImage>>>,
}
impl QoiLoader {
    fn new() -> Self {
        Self {
            cache: Default::default(),
        }
    }
}

impl ImageLoader for QoiLoader {
    fn id(&self) -> &str {
        concat!(module_path!(), "::QoiLoader")
    }

    fn load(
        &self,
        _ctx: &eframe::egui::Context,
        uri: &str,
        _size_hint: eframe::egui::SizeHint,
    ) -> eframe::egui::load::ImageLoadResult {
        if !uri.ends_with(".qoi") {
            return Err(eframe::egui::load::LoadError::NotSupported);
        }

        let image = self.cache.read().get(uri).cloned();
        if let Some(image) = image {
            println!("Cache Hit for {uri}");
            Ok(eframe::egui::load::ImagePoll::Ready { image })
        } else {
            match self.cache.write().entry(uri.to_string()) {
                std::collections::hash_map::Entry::Occupied(img) => {
                    println!("Cache Race for {uri}");
                    Ok(eframe::egui::load::ImagePoll::Ready {
                        image: img.get().clone(),
                    })
                }
                std::collections::hash_map::Entry::Vacant(placeholder) => {
                    println!("Cache Miss for {uri}");
                    let data =
                        std::fs::read(uri).map_err(|err| LoadError::Loading(err.to_string()))?;

                    let (header, pixel) = super::qoi::load(&data);
                    let size = [header.width as usize, header.height as usize];

                    let image = match header.channels {
                        arqoii_types::QoiChannels::Rgb => ColorImage::from_rgb(
                            size,
                            &pixel
                                .into_iter()
                                .flat_map(|px| [px.r, px.g, px.b])
                                .collect::<Vec<_>>(),
                        ),
                        arqoii_types::QoiChannels::Rgba => ColorImage::from_rgba_unmultiplied(
                            size,
                            &pixel
                                .into_iter()
                                .flat_map(|px| [px.r, px.g, px.b, px.a])
                                .collect::<Vec<_>>(),
                        ),
                    };

                    println!("Loaded {uri}");
                    Ok(eframe::egui::load::ImagePoll::Ready {
                        image: placeholder.insert(Arc::new(image)).clone(),
                    })
                }
            }
        }
    }

    fn forget(&self, uri: &str) {
        self.cache.write().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.write().clear()
    }

    fn byte_size(&self) -> usize {
        self.cache
            .read()
            .iter()
            .map(
                |(key, data)| key.len() + data.as_raw().len(), /* + HashMap overhead */
            )
            .sum()
    }
}

impl ArqoiiViewer {
    fn new(ctx: &eframe::CreationContext, image_paths: Vec<PathBuf>) -> Self {
        ctx.egui_ctx.add_image_loader(Arc::new(QoiLoader::new()));
        Self { image_paths }
    }
}

impl eframe::App for ArqoiiViewer {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::scroll_area::ScrollArea::both()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for path in &self.image_paths {
                            if let Some(path) = path.to_str() {
                                let image = egui::Image::from_uri(path)
                                    .maintain_aspect_ratio(true)
                                    .fit_to_original_size(1.0);

                                if let Some(size) = image.size() {
                                    if ui.available_width() < size.x + 12.0 {
                                        ui.end_row();
                                    }
                                }
                                ui.group(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(path);
                                        ui.add(image);
                                    });
                                });
                            }
                        }
                    })
                })
        });
    }
}

pub(crate) fn open(args: crate::CmdArgs) {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Arqoii Viewer",
        native_options,
        Box::new(|cc| Box::new(ArqoiiViewer::new(cc, args.paths))),
    )
    .unwrap();
}
