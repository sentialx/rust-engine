// Text rendering shader - renders textured quads from glyph atlas

struct Uniforms {
    screen_w: f32,
    screen_h: f32,
    _padding: vec2<f32>,
}

struct TextInstance {
    // Screen position (x, y, width, height)
    @location(0) pos: vec4<f32>,
    // UV in atlas (u, v, u_size, v_size)
    @location(1) uv: vec4<f32>,
    // Color (r, g, b, a)
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: TextInstance,
) -> VertexOutput {
    // Generate quad vertices (0-5 for two triangles)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );

    let local_pos = positions[vertex_index];

    // Screen position
    let screen_x = instance.pos.x + local_pos.x * instance.pos.z;
    let screen_y = instance.pos.y + local_pos.y * instance.pos.w;

    // Convert to clip space (-1 to 1)
    let clip_x = (screen_x / uniforms.screen_w) * 2.0 - 1.0;
    let clip_y = 1.0 - (screen_y / uniforms.screen_h) * 2.0;

    // UV coordinates
    let tex_u = instance.uv.x + local_pos.x * instance.uv.z;
    let tex_v = instance.uv.y + local_pos.y * instance.uv.w;

    var output: VertexOutput;
    output.position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    output.tex_coord = vec2<f32>(tex_u, tex_v);
    output.color = instance.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample alpha from atlas (stored as white with alpha)
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, input.tex_coord);
    let alpha = atlas_sample.a * input.color.a;

    // Output premultiplied alpha
    return vec4<f32>(input.color.rgb * alpha, alpha);
}
