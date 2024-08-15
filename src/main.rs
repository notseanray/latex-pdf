use anyhow::Result;
use cairo::Context;
use cairo::Format;
use cairo::ImageSurface;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::RgbImage;
use pdf::file::FileOptions;
use poppler::PopplerDocument;
use poppler::PopplerPage;
use rusty_pdf::lopdf::Document;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::io::{BufWriter, Cursor};
use imagesize::{blob_size, ImageSize};
use rusty_pdf::{PDFSigningDocument, Rectangle};

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

    ctx.save().unwrap();
    page.render(&ctx);
    ctx.restore().unwrap();
    ctx.show_page().unwrap();

    let mut f: File = File::create("out.png").unwrap();
    surface.write_to_png(&mut f).expect("Unable to write PNG");
    Ok(())
}

fn main() -> Result<()> {
    {
        let mut file = FileOptions::cached().open("input.pdf")?;
        //let mut answer_boxes: Option<_> = None;
        /*
        let page0 = file.get_page(0).unwrap();
        let annots = page0.media_box.clone().unwrap();
        */
        for page in 0..file.num_pages() {

        }
        let root = file.get_page(0).unwrap();
        println!("{:#?}", root);
    }


    // TODO return pdf dims
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

    let doc_mem = fs::read("input.pdf").unwrap();

    let doc = Document::load_mem(&doc_mem).unwrap_or_default();

    let image_mem = fs::read("out.png").unwrap();

    let dimensions = blob_size(&image_mem).unwrap_or(ImageSize {
        width: 0,
        height: 0,
    });

    let scaled_vec = Rectangle::scale_image_on_width(
        150.0,
        200.0,
        500.0,
        (dimensions.width as f64, dimensions.height as f64),
    );

    let file = Cursor::new(image_mem);
    let mut test_doc = PDFSigningDocument::new(doc);
    let object_id = test_doc.add_object_from_scaled_vec(scaled_vec);
    let page_id = *test_doc
        .get_document_ref()
        .get_pages()
        .get(&1)
        .unwrap_or(&(0, 0));

    test_doc
        .add_signature_to_form(file.clone(), "signature_1", page_id, object_id)
        .unwrap();

    test_doc.finished().save("output.pdf").unwrap();
    Ok(())
}
