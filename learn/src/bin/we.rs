use image::{Rgb, RgbImage};
use std::f32::consts::PI;

fn main() {
    // 1. Setup Resolution
    let width = 1920;
    let height = 1080;

    // Create a new black image buffer
    let mut img = RgbImage::new(width, height);

    // 2. Define a Beautiful Color Palette (Gradient Stops)
    // Format: (Position 0.0-1.0, Red, Green, Blue)
    // This creates a "Deep Ocean to Neon Sunset" vibe
    let palette = vec![
        (0.0, [10u8, 10, 30]),    // Deepest Midnight Blue
        (0.2, [60, 20, 100]),     // Deep Purple
        (0.5, [200, 50, 120]),    // Hot Pink
        (0.8, [255, 150, 50]),    // Sunset Orange
        (1.0, [255, 255, 200]),   // Pale Cream Highlight
    ];

    println!("Generating wallpaper... This may take a few seconds.");

    // 3. Iterate over every pixel
    for x in 0..width {
        for y in 0..height {
            // Normalize coordinates to 0.0 - 1.0 range for math
            let nx = x as f32 / width as f32;
            let ny = y as f32 / height as f32;

            // 4. The Plasma Algorithm
            // We combine multiple sine waves at different frequencies and angles
            // This creates the "fluid" look
            
            let v1 = (nx * 10.0 + PI).sin();
            let v2 = (ny * 10.0 + PI / 2.0).sin();
            let v3 = ((nx + ny) * 10.0 + PI / 4.0).sin();
            
            // Add a "ripple" from the center
            let cx = nx - 0.5;
            let cy = ny - 0.5;
            let dist = (cx * cx + cy * cy).sqrt() * 20.0;
            let v4 = (dist).sin();

            // Combine values. 
            // Result will be roughly between -3.0 and 3.0
            let value = v1 + v2 + v3 + v4;

            // Map the value (-4.0 to 4.0 approx) to a 0.0 - 1.0 range
            // We clamp it to ensure it stays strictly within bounds
            let t = ((value + 4.0) / 8.0).clamp(0.0, 1.0);

            // 5. Color Mapping (Linear Interpolation)
            let pixel_color = get_color_from_palette(t, &palette);

            // Set the pixel
            img.put_pixel(x, y, Rgb(pixel_color));
        }
        
        // Optional: Print progress every 100 lines
        if x % 100 == 0 {
            println!("Scanning line {} of {}", x, width);
        }
    }

    // 6. Save the file
    img.save("wallpaper.png").unwrap();
    println!("Done! Wallpaper saved to 'wallpaper.png'");
}

/// Helper function to blend colors between palette stops based on value t (0.0 to 1.0)
fn get_color_from_palette(t: f32, palette: &[(f32, [u8; 3])]) -> [u8; 3] { // Fixed typo: changed [u8; 8] to [u8; 3]
    // Find the two palette colors that 't' sits between
    for i in 0..palette.len() - 1 {
        let (p1_pos, p1_col) = palette[i];
        let (p2_pos, p2_col) = palette[i + 1];

        if t >= p1_pos && t <= p2_pos {
            // Calculate how far 't' is between p1 and p2 (0.0 to 1.0)
            let local_t = (t - p1_pos) / (p2_pos - p1_pos);

            return [
                lerp(p1_col[0], p2_col[0], local_t),
                lerp(p1_col[1], p2_col[1], local_t),
                lerp(p1_col[2], p2_col[2], local_t),
            ];
        }
    }
    
    // Fallback (should technically not be reached if t is clamped 0-1)
    palette[palette.len() - 1].1
}

/// Linear Interpolation for a single byte color channel
fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t) as u8
}
