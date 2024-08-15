use anyhow::Result;
use cairo::Context;
use cairo::Format;
use cairo::ImageSurface;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::RgbImage;
use poppler::PopplerDocument;
use poppler::PopplerPage;
use printpdf::image_crate::codecs::png::PngDecoder;
use printpdf::Image;
use printpdf::ImageTransform;
use printpdf::Mm;
use printpdf::PdfDocument;
use std::fs::File;
use std::io::Write;
use std::io::{BufWriter, Cursor};

pub fn save_pdf_as_image(path: &str) -> Result<()> {
    let doc: PopplerDocument = PopplerDocument::new_from_file(path, Some("upw")).unwrap();
    let num_pages = doc.get_n_pages();
    let title = doc.get_title().unwrap();
    let metadata = doc.get_metadata();
    let version_string = doc.get_pdf_version_string();
    let permissions = doc.get_permissions();
    let page: PopplerPage = doc.get_page(0).unwrap();
    let (w, h) = page.get_size();

    println!(
        "Document {} has {} page(s) and is {}x{}",
        title, num_pages, w, h
    );
    println!(
        "Version: {:?}, Permissions: {:x?}",
        version_string, permissions
    );

    assert!(metadata.is_some());
    assert_eq!(version_string, Some("PDF-1.3".to_string()));
    assert_eq!(permissions, 0xff);

    assert_eq!(title, "This is a test PDF file");

    let surface = ImageSurface::create(Format::A8, w as i32, h as i32).unwrap();
    let ctx = Context::new(&surface).unwrap();

    (|page: &PopplerPage, ctx: &Context| {
        ctx.save().unwrap();
        page.render(ctx);
        ctx.restore().unwrap();
        ctx.show_page().unwrap();
    })(&page, &ctx);

    let mut f: File = File::create("out.png").unwrap();
    surface.write_to_png(&mut f).expect("Unable to write PNG");
    Ok(())
}

fn main() -> Result<()> {
    save_pdf_as_image("test.pdf")?;

    {
        let img_data = std::fs::read("out.png")?;
        let output_file = File::create("out.png")?;
        let img =
            ImageReader::with_format(Cursor::new(&img_data), image::ImageFormat::Png).decode()?;

        let mut writer = BufWriter::new(output_file);
        let (width, height) = (img.width(), img.height());
        let rgb_img = match img {
            DynamicImage::ImageLuma8(rgba_img) => {
                let rgba_img = rgba_img.as_raw();

                let mut rgb_img = vec![0u8; (width * height * 3) as usize];
                for (idx, chunk) in rgba_img.iter().enumerate() {
                    let i = idx * 3;
                    rgb_img[i..i + 3].copy_from_slice(&[255 - *chunk, 255 - *chunk, 255 - *chunk]);
                }
                DynamicImage::ImageRgb8(
                    RgbImage::from_raw(width, height, rgb_img).expect("Corrupt png"),
                )
            }
            _ => img,
        };
        rgb_img.write_to(&mut writer, ImageFormat::Png)?;
        writer.flush()?;
    }
    /*
    let img_data = std::fs::read("out.png")?;
    let img = ImageReader::with_format(Cursor::new(&img_data), image::ImageFormat::Png).decode()?;

    let output_file = File::create("out.jpeg")?;
    let mut writer = BufWriter::new(output_file);
    let (width, height) = (img.width(), img.height());
    let rgb_img = match img {
        DynamicImage::ImageRgba8(rgba_img) => {
            // Convert RGBA to RGB by discarding the alpha channel
            let rgba_img = rgba_img.as_raw();

            let mut rgb_img = vec![0u8; (width * height * 3) as usize];
            for (idx, chunk) in rgba_img.chunks(4).enumerate() {
                let i = idx * 3;
                // the exporter tends to only use alpha channel to set color
                if chunk[0..3] == [0; 3] && chunk[3] != 0 {
                    rgb_img[i..i+3].copy_from_slice(&[255 - chunk[3], 255 - chunk[3], 255 - chunk[3]]);
                } else if chunk[0..3] != [0; 3] {
                    // scale by alpha to determine color strength, TODO might need to subtract from
                    // 255 to get it to actually match the intended colors
                    rgb_img[i..i+3].copy_from_slice(&chunk[0..3].iter().map(|x| (*x as f32 * ((chunk[3] as f32) / 255.0)) as u8).collect::<Vec<u8>>());
                } else {
                    // default white background for this
                    rgb_img[i..i+3].copy_from_slice(&[255, 255, 255]);
                }
            }
            DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, rgb_img).expect("Corrupt png"))
        },
        _ => img,
    };
    rgb_img.write_to(&mut writer, ImageFormat::Jpeg)?;


    let img_data = std::fs::read("out.jpeg")?;
    */
    let (doc, page1, layer1) =
        PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let img_data = std::fs::read("out.png")?;
    let mut reader = Cursor::new(img_data);
    let decoder = PngDecoder::new(&mut reader).unwrap();
    let image = Image::try_from(decoder).unwrap();

    // layer,
    image.add_to_layer(
        current_layer.clone(),
        ImageTransform {
            rotate: None,
            translate_x: Some(Mm(5.0)),
            translate_y: Some(Mm(5.0)),
            ..Default::default()
        },
    );
    doc.save(&mut BufWriter::new(File::create("test_image.pdf").unwrap()))
        .unwrap();
    Ok(())
}
