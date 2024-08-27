use anyhow::Result;
use cairo::Context;
use cairo::Format;
use cairo::ImageSurface;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::RgbImage;
use imagesize::blob_size;
use opencv::core::get_platfoms_info;
use opencv::core::have_opencl;
use opencv::core::set_use_opencl;
use opencv::core::use_opencl;
use opencv::core::Device;
use opencv::core::DeviceTraitConst;
use opencv::core::Mat;
use opencv::core::MatTraitConst;
use opencv::core::PlatformInfoTraitConst;
use opencv::core::Size;
use opencv::core::UMat;
use opencv::core::Vector;
use opencv::imgcodecs;
use opencv::imgproc;
use opencv::imgproc::bounding_rect;
use opencv::imgproc::rectangle;
use opencv::imgproc::CHAIN_APPROX_SIMPLE;
use opencv::imgproc::RETR_EXTERNAL;
use opencv::imgproc::RETR_LIST;
use opencv::imgproc::THRESH_BINARY;
use opencv::imgproc::THRESH_OTSU;
use opencv::types::VectorOfVectorOfPoint;
use poppler::PopplerDocument;
use poppler::PopplerPage;
use rusty_pdf::lopdf::Document;
use rusty_pdf::{PDFSigningDocument, Rectangle};
use std::fs;
use std::fs::File;
use std::io::stdin;
use std::io::Write;
use std::io::{BufWriter, Cursor};

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

fn find_answer_boxes(path: &str) -> Result<Vec<(i32, i32, i32, i32)>> {
    let opencl_have = have_opencl()?;
    if opencl_have {
        set_use_opencl(true)?;
        let mut platforms = Vector::new();
        get_platfoms_info(&mut platforms)?;
        for (platf_num, platform) in platforms.into_iter().enumerate() {
            println!("Platform #{}: {}", platf_num, platform.name()?);
            for dev_num in 0..platform.device_number()? {
                let mut dev = Device::default();
                platform.get_device(&mut dev, dev_num)?;
                println!("  OpenCL device #{}: {}", dev_num, dev.name()?);
                println!("    vendor:  {}", dev.vendor_name()?);
                println!("    version: {}", dev.version()?);
            }
        }
    }
    let opencl_use = use_opencl()?;
    println!(
        "OpenCL is {} and {}",
        if opencl_have {
            "available"
        } else {
            "not available"
        },
        if opencl_use { "enabled" } else { "disabled" },
    );
    Ok(if opencl_use {
        let mat = imgcodecs::imread_def(path)?;
        let img = mat.get_umat(
            opencv::core::AccessFlag::ACCESS_READ,
            opencv::core::UMatUsageFlags::USAGE_ALLOCATE_DEVICE_MEMORY,
        )?;
        let mut gray = UMat::new_def();
        imgproc::cvt_color_def(&img, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        let mut blurred = UMat::new_def();
        imgproc::gaussian_blur_def(&gray, &mut blurred, Size::new(7, 7), 1.5)?;
        let mut threshold = UMat::new_def();
        imgproc::threshold(
            &blurred,
            &mut threshold,
            0.,
            255.,
            THRESH_BINARY + THRESH_OTSU,
        )?;
        let mut contours = VectorOfVectorOfPoint::new();
        imgproc::find_contours(
            &threshold,
            &mut contours,
            RETR_EXTERNAL,
            CHAIN_APPROX_SIMPLE,
            opencv::core::Point_::new(0, 0),
        )?;
        contours
    } else {
        let img = imgcodecs::imread_def(path)?;
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&img, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        let mut blurred = Mat::default();
        imgproc::gaussian_blur_def(&gray, &mut blurred, Size::new(7, 7), 1.5)?;
        let mut threshold = Mat::default();
        imgproc::threshold(
            &blurred,
            &mut threshold,
            0.,
            255.,
            THRESH_BINARY + THRESH_OTSU,
        )?;
        let mut contours = VectorOfVectorOfPoint::new();
        imgproc::find_contours(
            &threshold,
            &mut contours,
            RETR_EXTERNAL,
            CHAIN_APPROX_SIMPLE,
            opencv::core::Point_::new(0, 0),
        )?;
        contours
    }
    .iter()
    .map(|x| bounding_rect(&x).unwrap())
    .filter_map(|x| {
        if x.width * x.height > 10000 {
            Some((x.width, x.height, x.x, x.y))
        } else {
            None
        }
    })
    .collect())
}

/*
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
    for (idx, i) in (1..=vertical_box_starts.len() / 4).enumerate() {
        let idx = idx * 2;
        vertical_box_starts_trimmed[idx] = vertical_box_starts[i * 4 - 3];
        vertical_box_starts_trimmed[idx + 1] = vertical_box_starts[i * 4 - 2];
    }
    println!("vertical lines {:#?}", vertical_box_starts_trimmed);

    let largest = *horizontal_lines.iter().max().unwrap_or(&0);
    let max_range = largest as f32 * 0.85;
    let mut horizontal_box_starts = Vec::with_capacity(4);
    for (idx, row) in horizontal_lines.iter().enumerate() {
        if *row > (max_range + 0.5) as i32 {
            horizontal_box_starts.push(idx);
        }
    }
    assert!(horizontal_box_starts.len() % 4 == 0);
    let mut horizontal_box_starts_trimmed = vec![0; horizontal_box_starts.len() / 2];
    for (idx, i) in (1..=horizontal_box_starts.len() / 4).enumerate() {
        let idx = idx * 2;
        horizontal_box_starts_trimmed[idx] = horizontal_box_starts[i * 4 - 3];
        horizontal_box_starts_trimmed[idx + 1] = horizontal_box_starts[i * 4 - 2];
    }
    println!("horizontal lines {:#?}", horizontal_box_starts_trimmed);

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
*/

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

    println!("Saving og pdf as image");
    let (original_width, original_height) = save_pdf_as_image("input.pdf", "work/original.png")?;

    let work_dir = fs::read_dir("work")?;
    let mut pages = vec![];
    for entry in work_dir {
        let entry = entry?;
        if let Some(v) = entry.file_name().to_str() {
            println!("{v}");
            if !v.starts_with("original") {
                continue;
            }
            pages.push(format!("work/{v}"));
        }
    }
    pages.sort();
    println!("{:#?}", pages);

    println!("finding boxes");
    for page in pages {
        let boxes = find_answer_boxes(&page);
        println!("page: {page} \n{:#?}", boxes);
    }

    /*
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
    */

    Ok(())
}
