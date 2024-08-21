use anyhow::Result;
use cairo::Context;
use cairo::Format;
use cairo::ImageSurface;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::RgbImage;
use imagesize::blob_size;
use poppler::PopplerDocument;
use poppler::PopplerPage;
use rusty_pdf::lopdf::Document;
use rusty_pdf::{PDFSigningDocument, Rectangle};
use std::fs;
use std::fs::File;
use std::io::stdin;
use std::io::Write;
use std::io::{BufWriter, Cursor};

mod canny;
use canny::canny;

pub fn save_pdf_as_image(path: &str, output_path: &str) -> Result<(f64, f64)> {
    let doc: PopplerDocument = PopplerDocument::new_from_file(path, Some("upw")).unwrap();
    let num_pages = doc.get_n_pages();
    let title = doc.get_title().unwrap();
    let version_string = doc.get_pdf_version_string();
    let permissions = doc.get_permissions();
    let mut width = 0.0;
    let mut height = 0.0;
    for page_num in 0..doc.get_n_pages() {
        let page: PopplerPage = doc.get_page(page_num).unwrap();
        let (w, h) = page.get_size();
        if width != 0.0 && width != w {
            panic!("width changes");
        }
        width = w;
        height = h;

        println!(
            "Document {} has {} page(s) and is {}x{}",
            title, num_pages, w, h
        );
        println!(
            "Version: {:?}, Permissions: {:x?}",
            version_string, permissions
        );

        let surface = ImageSurface::create(Format::A8, w as i32, h as i32).unwrap();
        let ctx = Context::new(&surface).unwrap();

        ctx.save().unwrap();
        page.render(&ctx);
        ctx.restore().unwrap();
        ctx.show_page().unwrap();

        if let Some((path, _extension)) = output_path.split_once(".") {
            let mut padded_page_num = format!("{page_num}");
            let page_num_str_len = format!("{page_num}").len();
            if page_num_str_len < 3 {
                for _ in 0..3 - page_num_str_len {
                    padded_page_num = format!("0{padded_page_num}");
                }
            }
            let mut f: File = File::create(format!("{path}_{padded_page_num}.png")).unwrap();
            surface.write_to_png(&mut f).expect("Unable to write PNG");
        }
    }
    Ok((width, height))
}

fn find_answer_boxes(path: &str) -> Result<Option<(usize, usize, usize, usize)>> {
    let source_image = image::open(path)?.to_luma8();
    let detection = canny(
        source_image,
        0.8, // sigma
        0.4, // strong threshold
        0.3, // weak threshold
    );
    // image::save_buffer("edge.png", detection.as_image().as_bytes(), detection.width() as u32, detection.height() as u32, ExtendedColorType::Rgb8)?;
    let mut vertical_lines = vec![0; detection.height()];
    let mut horizontal_lines = vec![0; detection.width()];
    for (r, row) in detection.edges.iter().enumerate() {
        let mut active;
        for (c, col) in row.iter().enumerate() {
            if col.magnitude() > 0.01 {
                vertical_lines[c] += 1;
                active = true;
            } else {
                active = false;
            }
            if active {
                horizontal_lines[r] += 1;
            }
        }
    }
    let largest = *vertical_lines.iter().max().unwrap_or(&0);
    let max_range = largest as f32 * 0.95;
    let mut vertical_box_starts = Vec::with_capacity(4);
    for (idx, col) in vertical_lines.iter().enumerate() {
        if *col > (max_range + 0.5) as i32 {
            vertical_box_starts.push(idx);
        }
    }
    assert!(vertical_box_starts.len() % 4 == 0);
    let mut vertical_box_starts_trimmed = vec![0; vertical_box_starts.len() / 2];
    for i in 1..=vertical_box_starts.len() / 4 {
        vertical_box_starts_trimmed[i..i+1] = vertical_box_starts[i * 1..i * 3];
        println!("vertical lines {:#?}", vertical_box_starts);
    }

    let largest = *horizontal_lines.iter().max().unwrap_or(&0);
    let max_range = largest as f32 * 0.95;
    let mut horizontal_box_starts = Vec::with_capacity(4);
    for (idx, row) in horizontal_lines.iter().enumerate() {
        if *row > (max_range + 0.5) as i32 {
            horizontal_box_starts.push(idx);
        }
    }
    assert!(horizontal_box_starts.len() == 4);
    if horizontal_box_starts[0].max(horizontal_box_starts[1])
        - horizontal_box_starts[0].min(horizontal_box_starts[1])
        > 5
    {
        println!("box upper dimensions too variable");
        return Ok(None);
    }
    if horizontal_box_starts[2].max(horizontal_box_starts[3])
        - horizontal_box_starts[2].min(horizontal_box_starts[3])
        > 5
    {
        println!("box bottom dimensions too variable");
        return Ok(None);
    }
    horizontal_box_starts = horizontal_box_starts[1..3].to_vec();
    println!("horizontal lines {:#?}", horizontal_box_starts);

    let (height, width) = (
        vertical_box_starts[1] - vertical_box_starts[0],
        horizontal_box_starts[1] - horizontal_box_starts[0],
    );
    println!("box dimensions: {width} x {height}");
    Ok(Some((
        horizontal_box_starts[1],
        vertical_box_starts[1],
        width,
        height,
    )))
}

// render latex pdf to png
pub fn render_pdf_to_png_and_resize(
    input_path: &str,
    output_path: &str,
    width: usize,
    height: usize,
) -> Result<()> {
    let img_data = std::fs::read(input_path)?;
    let output_file = File::create(output_path)?;
    let img = ImageReader::with_format(Cursor::new(&img_data), image::ImageFormat::Png).decode()?;

    let mut writer = BufWriter::new(output_file);
    let (width, height) = (img.width(), img.height());
    let mut rgb_img = match img {
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
    let rgb_img = rgb_img.crop(0, 0, width, height);
    rgb_img.write_to(&mut writer, ImageFormat::Png)?;
    writer.flush()?;
    Ok(())
}

pub fn place_png_on_pdf(
    input_pdf: &str,
    image_input: &str,
    x: f64,
    y: f64,
    width: usize,
) -> Result<()> {
    let doc_mem = fs::read(input_pdf)?;

    let doc = Document::load_mem(&doc_mem).unwrap_or_default();

    let image_mem = fs::read(image_input)?;

    let dimensions = blob_size(&image_mem)?;
    println!("{} {}", dimensions.width, dimensions.height);

    let scaled_vec = Rectangle::scale_image_on_width(
        width as f64,
        x,
        y,
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

fn main() -> Result<()> {
    if !fs::exists("work")? {
        fs::create_dir("work")?;
    }

    let (original_width, original_height) = save_pdf_as_image("input.pdf", "work/original.png")?;

    let work_dir = fs::read_dir("work")?;
    let mut pages = vec![];
    for entry in work_dir {
        let entry = entry?;
        if let Some(v) = entry.file_name().to_str() {
            if !v.starts_with("original") {
                continue;
            }
            pages.push(v.to_owned());
        }
    }
    pages.sort();

    let mut boxes = Vec::with_capacity(pages.len());
    for (idx, page) in pages.iter().enumerate() {
        let box_pos = find_answer_boxes(&page)?;
        if let Some(v) = box_pos {
            boxes.push(v);
        } else {
            println!("Box was not found for page {idx}, please type Yes to ignore");
            let mut buffer = String::new();
            stdin().read_line(&mut buffer)?;
            if buffer.trim() != "Yes" {
                panic!("mixing box found and user chose to quit");
            }
        }
    }

    let offset_coordinates: Vec<(f64, f64, usize, usize)> = boxes
        .iter_mut()
        .map(|(x, y, w, h)| {
            (
                original_width - *x as f64,
                original_height - *y as f64,
                *w,
                *h,
            )
        })
        .collect();

    unimplemented!();

    // let mut edges = detection.edges.into_iter().flatten().filter(|x| x.magnitude() > 0.0).collect::<Vec<Edge>>();
    // edges.sort_by(|l, r| { if l.magnitude() > r.magnitude() { Ordering::Greater } else { Ordering::Less } });
    // println!("{:#?}", edges);

    // TODO return pdf dims
    // Latex pdf
    save_pdf_as_image("test.pdf", "work/latex.png")?;

    Ok(())
}
