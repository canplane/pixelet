// The world's depth as the badge shader reads it, where the frame is drawn
// multisampled: textureLoad's third argument is then the sample, and the
// first sample stands for the pixel.
@group(0) @binding(2) var world_depth: texture_depth_multisampled_2d;
