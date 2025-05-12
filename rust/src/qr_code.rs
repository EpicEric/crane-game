use godot::classes::image::Format;
use godot::classes::Image;
use godot::prelude::*;
use qrcode::render::{Canvas, Pixel};
use qrcode::QrCode;

#[derive(Copy, Clone)]
struct GodotPixel(Color);

impl Pixel for GodotPixel {
    type Image = Gd<Image>;
    type Canvas = GodotCanvas;

    fn default_color(color: qrcode::Color) -> Self {
        color.select(
            Self(Color::from_rgb(0.0, 0.0, 0.0)),
            Self(Color::from_rgb(1.0, 1.0, 1.0)),
        )
    }
}

struct GodotCanvas {
    canvas: Vec<Color>,
    width: u32,
    height: u32,
    dark_pixel: Color,
}

impl Canvas for GodotCanvas {
    type Pixel = GodotPixel;
    type Image = Gd<Image>;

    fn new(width: u32, height: u32, dark_pixel: Self::Pixel, light_pixel: Self::Pixel) -> Self {
        let canvas = vec![light_pixel.0; (width * height) as usize];
        Self {
            width,
            height,
            canvas,
            dark_pixel: dark_pixel.0,
        }
    }

    fn draw_dark_pixel(&mut self, x: u32, y: u32) {
        self.canvas[(x + y * self.width) as usize] = self.dark_pixel;
    }

    fn into_image(self) -> Self::Image {
        let mut image =
            Image::create_empty(self.width as i32, self.height as i32, false, Format::RGBF)
                .unwrap();
        self.canvas
            .chunks_exact(self.width as usize)
            .enumerate()
            .for_each(|(row, row_values)| {
                row_values.iter().enumerate().for_each(|(col, color)| {
                    image.set_pixel(col as i32, row as i32, *color);
                });
            });
        image
    }
}

#[derive(GodotClass)]
#[class(init, base=Object)]
pub struct QrCodeSingleton {
    base: Base<Object>,
}

#[godot_api]
impl QrCodeSingleton {
    #[func]
    fn create(&mut self, text: GString) -> Gd<Image> {
        QrCode::new(text.to_string())
            .unwrap()
            .render::<GodotPixel>()
            .quiet_zone(true)
            .build()
    }
}
