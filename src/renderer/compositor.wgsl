// Compositor shader for blitting textures to screen

struct Uniforms {
    dest_x: f32,
    dest_y: f32,
    dest_w: f32,
    dest_h: f32,
    src_y: f32,
    src_h: f32,
    screen_w: f32,
    screen_h: f32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(0) @binding(1)
var t_texture: texture_2d<f32>;

@group(0) @binding(2)
var s_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Convert from pixel coordinates to NDC (-1 to 1)
    // in.position is 0-1, scale to dest size and position
    let pixel_x = u.dest_x + in.position.x * u.dest_w;
    let pixel_y = u.dest_y + in.position.y * u.dest_h;

    // Convert to NDC: x goes -1 to 1, y goes 1 to -1 (flipped)
    let ndc_x = (pixel_x / u.screen_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_y / u.screen_h) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    // Texture coordinates with scroll offset
    out.tex_coords = vec2<f32>(
        in.tex_coords.x,
        u.src_y + in.tex_coords.y * u.src_h
    );

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_texture, s_sampler, in.tex_coords);
}
