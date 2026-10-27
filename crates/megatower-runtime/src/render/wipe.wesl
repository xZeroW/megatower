// `gameplay_screen_transition.gdshader`: a diamond wipe. `progress` runs 0..1
// to cover (from the top-left corner) and 1..2 to clear again.
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

// x: progress, y/z: region origin in logical px, w: unused.
@group(1) @binding(0) var<uniform> params: vec4<f32>;
// x/y: region size in logical px.
@group(1) @binding(1) var<uniform> region: vec4<f32>;
@group(1) @binding(2) var<uniform> color: vec4<f32>;

fn when_lt(x: f32, y: f32) -> f32 {
    return max(sign(y - x), 0.0);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let lx = params.y + in.uv.x * region.x;
    let ly = params.z + in.uv.y * region.y;
    let x_distance = abs(fract(lx / 10.0) - 0.5);
    let y_distance = abs(fract(ly / 10.0) - 0.5);
    let t = x_distance + y_distance + in.uv.x + in.uv.y;
    var cover = 0.0;
    if (params.x > 1.0) {
        cover = 1.0 - when_lt(t, (params.x - 1.0) * 4.0);
    } else {
        cover = when_lt(t, params.x * 4.0);
    }
    return vec4<f32>(color.rgb, color.a * cover);
}
