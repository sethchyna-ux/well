mod render;

fn frame_count(seconds: u32, fps: u32) -> u32 {
    // BUG: this returns seconds instead of the number of frames.
    seconds
}

fn main() {
    println!("{} frames", frame_count(2, 60));
}
