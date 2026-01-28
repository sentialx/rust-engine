// Shader for rendering solid color quads

struct QuadInstance {
    // Position and size in pixels
    @location(0) pos_size: vec4<f32>,  // x, y, width, height
    // Color RGBA (0-1)
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    screen_w: f32,
    screen_h: f32,
    scroll_y: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: QuadInstance,
) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices from vertex_index (0-5 for two triangles)
    // 0: top-left, 1: top-right, 2: bottom-right
    // 3: top-left, 4: bottom-right, 5: bottom-left
    var local_pos: vec2<f32>;
    switch vertex_index {
        case 0u, 3u: { local_pos = vec2<f32>(0.0, 0.0); }
        case 1u: { local_pos = vec2<f32>(1.0, 0.0); }
        case 2u, 4u: { local_pos = vec2<f32>(1.0, 1.0); }
        case 5u: { local_pos = vec2<f32>(0.0, 1.0); }
        default: { local_pos = vec2<f32>(0.0, 0.0); }
    }

    // Convert to pixel position
    let pixel_x = instance.pos_size.x + local_pos.x * instance.pos_size.z;
    let pixel_y = instance.pos_size.y + local_pos.y * instance.pos_size.w - u.scroll_y;

    // Convert to NDC
    let ndc_x = (pixel_x / u.screen_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_y / u.screen_h) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Output premultiplied alpha
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
