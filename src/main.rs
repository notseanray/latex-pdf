use anyhow::Result;
use image::open;
use image::DynamicImage;
use image::ImageFormat;
use image::ImageReader;
use image::RgbaImage;
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
use opencv::imgproc::CHAIN_APPROX_SIMPLE;
use opencv::imgproc::RETR_EXTERNAL;
use opencv::imgproc::THRESH_BINARY;
use opencv::imgproc::THRESH_BINARY_INV;
use opencv::imgproc::THRESH_OTSU;
use opencv::types::VectorOfVectorOfPoint;
use pdfium_render::prelude::PdfPageImageObject;
use pdfium_render::prelude::PdfPageObjectsCommon;
use pdfium_render::prelude::PdfPoints;
use pdfium_render::prelude::PdfRenderConfig;
use pdfium_render::prelude::Pdfium;
use pdfium_render::prelude::PdfiumError;
use std::fs;
use std::fs::File;
use std::io::stdin;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::io::{BufWriter, Cursor};
use std::process::Command;

pub fn save_pdf_as_image(pdfium: &Pdfium, path: &str, output_path: &str, enumerate: bool, input_width: Option<i32>, input_height: Option<i32>) -> Result<(f64, f64)> {
    // Renders each page in the PDF file at the given path to a separate JPEG file.

    // Bind to a Pdfium library in the same directory as our Rust executable.
    // See the "Dynamic linking" section below.

    // Load the document from the given path...

    println!("pages");
    let document = pdfium.load_pdf_from_file(path, None)?;

    // ... set rendering options that will be applied to all pages...

    let (width, height) = (input_width.unwrap_or(794), input_height.unwrap_or(1028));
    let render_config = PdfRenderConfig::new()
        .set_target_width(width)
        .set_maximum_height(height);

    // ... then render each page to a bitmap image, saving each image to a JPEG file.

    for (index, page) in document.pages().iter().enumerate() {
        let name = if enumerate {
            if let Some((path, _extension)) = output_path.split_once(".") {
                let mut padded_page_num = format!("{index}");
                let page_num_str_len = format!("{index}").len();
                if page_num_str_len < 3 {
                    for _ in 0..3 - page_num_str_len {
                        padded_page_num = format!("0{padded_page_num}");
                    }
                }
                format!("{path}_{padded_page_num}.png")
            } else {
                unimplemented!()
            }
        } else {
            output_path.to_owned()
        };
        println!("rendering page");
        page.render_with_config(&render_config)?
            .as_image() // Renders this page to an image::DynamicImage...
            .into_rgba8() // ... then converts it to an image::Image...
            .save_with_format(
                &name,
                image::ImageFormat::Png
            ) // ... and saves it to a file.
            .map_err(|_| PdfiumError::ImageError)?;
        //render_pdf_to_png_and_resize(&name, &name, width  as u32, height as u32)?;
    }
    Ok((0., 0.))
}
/*
pub fn save_pdf_as_image(path: &str, output_path: &str, enumerate: bool, input_width: Option<i32>, input_height: Option<i32>) -> Result<(f64, f64)> {
    let doc: PopplerDocument = PopplerDocument::new_from_file(path, Some("upw")).unwrap();
    let version_string = doc.get_pdf_version_string();
    let permissions = doc.get_permissions();
    let width = 0.;
    let height = 0.;
    for page_num in 0..doc.get_n_pages() {
        let page: PopplerPage = doc.get_page(page_num).unwrap();
        let (w, h) = page.get_size();
        if width != 0.0 && width != w {
            panic!("width changes");
        }
        let mut width = w as i32;
        let mut height = h as i32;

        if let (Some(w), Some(h)) = (input_width, input_height) {
            width = w;
            height = h;
        }

        println!(
            "Version: {:?}, Permissions: {:x?}",
            version_string, permissions
        );

        let surface = ImageSurface::create(Format::A8, width * 2, height * 2).unwrap();
        let ctx = Context::new(&surface).unwrap();
        ctx.set_antialias(cairo::Antialias::Subpixel);
        ctx.font_options().unwrap().set_antialias(cairo::Antialias::Subpixel);
        ctx.font_options().unwrap().set_hint_style(cairo::HintStyle::Full);
        let lib = Library::init().unwrap();
        // Load a font face
        let face = lib.new_face("./menlo.ttf", 0).unwrap();
        // Set the font size
        face.set_char_size(40 * 64, 0, 50, 0).unwrap();
        ctx.set_font_face(&FontFace::create_from_ft(&face)?);

        ctx.save().unwrap();
        page.render(&ctx);
        ctx.restore().unwrap();
        ctx.show_page().unwrap();

        if enumerate {
            if let Some((path, _extension)) = output_path.split_once(".") {
                let mut padded_page_num = format!("{page_num}");
                let page_num_str_len = format!("{page_num}").len();
                if page_num_str_len < 3 {
                    for _ in 0..3 - page_num_str_len {
                        padded_page_num = format!("0{padded_page_num}");
                    }
                }
                let mut f: File = File::create(format!("{path}_{padded_page_num}.png")).unwrap();
                surface.set_device_scale(0.1, 0.1);
                surface.write_to_png(&mut f).expect("Unable to write PNG");
            }
        } else {
            let mut f: File = File::create(output_path).unwrap();
            surface.write_to_png(&mut f).expect("Unable to write PNG");
            render_pdf_to_png_and_resize(output_path, output_path, width as u32, height as u32)?;
        }
    }
    Ok((width, height))
}
*/

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
        imgproc::gaussian_blur_def(&gray, &mut blurred, Size::new(1, 1), 0.)?;
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
        let mut threshold = Mat::default();
        imgproc::threshold(
            &gray,
            &mut threshold,
            0.,
            255.,
            THRESH_BINARY_INV + THRESH_OTSU,
        )?;
        let mut blurred = Mat::default();
        imgproc::gaussian_blur_def(&threshold, &mut blurred, Size::new(1, 1), 0.)?;
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
    target_width: u32,
    target_height: u32,
) -> Result<()> {
    let img_data = std::fs::read(input_path)?;
    let output_file = File::create(output_path)?;
    let img = ImageReader::with_format(Cursor::new(&img_data), image::ImageFormat::Png).decode()?;

    let mut writer = BufWriter::new(output_file);
    let (width, height) = (img.width(), img.height());
    let mut rgb_img = match img {
        DynamicImage::ImageLuma8(rgba_img) => {
            let rgba_img = rgba_img.as_raw();

            let mut rgb_img = vec![0u8; (width * height * 4) as usize];
            for (idx, chunk) in rgba_img.iter().enumerate() {
                let i = idx * 4;
                rgb_img[i..i + 4].copy_from_slice(&[255 - *chunk, 255 - *chunk, 255 - *chunk, if *chunk > 0 { 0 } else { 255 }]);
            }
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, rgb_img).expect("Corrupt png"),
            )
        }
        _ => img.into_rgba8().into(),
    };
    let rgb_img = rgb_img.crop(0, 0, target_width, target_height);
    rgb_img.write_to(&mut writer, ImageFormat::Png)?;
    writer.flush()?;
    Ok(())
}

pub fn place_png_on_pdf(
    input_pdf: &str,
    page_num: u32,
    image_input: &str,
    x: f32,
    y: f32,
    width: usize,
    height: usize,
) -> Result<()> {
    Ok(())
}

/*
pub fn place_png_on_pdf(
    input_pdf: &str,
    page_num: u32,
    image_input: &str,
    x: f64,
    y: f64,
    width: usize,
    height: usize,
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
        (width as f64, height as f64),
    );

    let file = Cursor::new(image_mem);
    let mut test_doc = PDFSigningDocument::new(doc);
    let object_id = test_doc.add_object_from_scaled_vec(scaled_vec);
    let page_id = *test_doc
        .get_document_ref()
        .get_pages()
        .get(&page_num)
        .unwrap_or(&(0, 0));

    test_doc
        .add_signature_to_form(file.clone(), image_input, page_id, object_id)
        .unwrap();

    test_doc.finished().save("output.pdf").unwrap();
    Ok(())
}
*/



fn check_for_yes(quit_msg: &str) -> Result<()> {
    println!("type yes if you need ");
    let mut buffer = String::new();
    stdin().read_line(&mut buffer)?;
    if buffer.trim() != "Yes" {
        panic!("{quit_msg}");
    }
    Ok(())
}

fn render_latex(pdfium: &Pdfium, latex_solution: &str, idx: usize, width: i32, height: i32) -> Result<String> {
    let path = format!("work/problem_{idx}.tex");
    fs::write(&path, latex_solution)?;
    let output = Command::new("tectonic").args([&path]).output()?;
    let output = String::from_utf8(output.stdout)?;
    if !output.contains("Writing ") {
        println!("tectonic command might have failed: ");
        println!("{output}");
        check_for_yes("quit due to user selection")?;
    }
    //let png_path = format!("work/problem_{idx}.png");
    let path = format!("work/problem_{idx}.pdf");
    println!("{path}");
    println!("saving");
    //save_pdf_as_image(pdfium, &path, &png_path, false, Some(width), Some(height))?;
    Ok(path)
}

fn main() -> Result<()> {
    if !fs::exists("work")? {
        fs::create_dir("work")?;
    }
    for entry in std::fs::read_dir("work")? {
        let entry = entry?;
        std::fs::remove_file(entry.path())?;
    }
    println!("test");
    let pdfium = Pdfium::default();
    println!("test2");

    //let latex = render_latex("math.tex")?;
    let f = File::open("math.tex")?;
    let reader = BufReader::new(f);

    // random choice
    let mut latex = Vec::with_capacity(5);
    let mut prev_buf = String::with_capacity(500);
    for line in reader.lines() {
        let line = line?;
        if &line == "<<-->>" {
            latex.push(prev_buf.to_owned());
            prev_buf.clear();
        } else {
            prev_buf.push_str(&line);
        }
    }
    latex.push(prev_buf.to_owned());

    //println!("Saving og pdf as image");
    let (original_width, original_height) = save_pdf_as_image(&pdfium, "input.pdf", "work/original.png", true, None, None)?;

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
    let mut page_boxes = Vec::with_capacity(pages.len());
    for (page_num, page) in pages.iter().enumerate() {
        println!("{page}");
        let boxes = find_answer_boxes(page).expect("Failed to get boxes");
        for box_rect in boxes {
            page_boxes.push((page_num, box_rect));
        }
    }
    println!("boxes");
    println!("{:#?}", page_boxes);

    let mut page = pdfium.load_pdf_from_file("input.pdf", None)?;
    let a4_width = 8.27;
    let a4_height = 11.69;
    let padding = 0.001;
    for (idx, ((page_num, rect), latex)) in page_boxes.iter().zip(latex).enumerate() {
        let latex = render_latex(&pdfium, &latex, idx, rect.0, rect.1)?;
        println!("placing problem {page_num} {latex}");
        //place_png_on_pdf("input.pdf", *page_num as u32, &latex, rect.2 as f32, rect.3 as f32, rect.0 as usize, rect.1 as usize)?;
        let latex = pdfium.load_pdf_from_file(&latex, None)?.pages().first()?.render_with_config(&PdfRenderConfig::new().set_target_width(794 * 3))?.as_image();
        //let latex = image::open(&latex)?;
        let latex = latex.crop_imm(0, 0, latex.width(), rect.1 as u32);
        let mut object = PdfPageImageObject::new_with_width(
            &page,
            &latex,
            PdfPoints::new(794.),
        )?;
        object.scale(0.5, 0.5)?;
        object.translate(PdfPoints::from_inches((rect.2 as f32 / 794.) * a4_width + padding), PdfPoints::from_inches(((rect.3 as f32) / 1028.) * a4_height - padding))?;
        page.pages_mut().get(*page_num as u16)?.objects_mut().add_image_object(object)?;
    }
    page.save_to_file("output.pdf")?;

    //save_pdf_as_image("test.pdf", "work/latex.png")?;

    Ok(())
}
