// pxl-gpu-link/admit.rs

//! Admission says whether the GPU's host can take another frame now without
//! blocking. Every host holds a frame to one rule: a guest draws a frame only
//! while fewer of its frames than its frame latency are unfinished on the
//! GPU.

pub fn admits(unfinished: u32, latency: u32) -> bool {
    unfinished < latency
}
