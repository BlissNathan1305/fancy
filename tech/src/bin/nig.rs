use image::{ImageBuffer, Rgb};

fn main() {
    let width = 600;
    let height = 400;

    // Explicitly type the image buffer to use u8
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);

    // Add 'u8' to the numbers so Rust treats them as unsigned 8-bit integers
    let green = Rgb([0u8, 135u8, 81u8]);
    let white = Rgb([255u8, 255u8, 255u8]);

    let stripe_width = width / 3;

    for y in 0..height {
        for x in 0..width {
            let pixel = if x < stripe_width {
                green
            } else if x < stripe_width * 2 {
                white
            } else {
                green
            };
            
            img.put_pixel(x, y, pixel);
        }
    }

    match img.save("nigeria_flag.jpeg") {
        Ok(_) => println!("Successfully saved nigeria_flag.jpeg"),
        Err(e) => eprintln!("Failed to save image: {}", e),
    }
}
