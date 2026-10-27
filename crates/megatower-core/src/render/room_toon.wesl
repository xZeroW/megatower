// Native port of `pixel_toon.gdshader`: triplanar wall texture recolored to
// the project palette, then each OmniLight's `light()` pass picks the nearest
// palette color for its attenuated brightness and adds it scaled by that
// attenuation (`SPECULAR_LIGHT += new_albedo_tex * ATTENUATION`).
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> room: RoomMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var wall_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var wall_sampler: sampler;

struct RoomMaterial {
    dim: f32,
    uv_scale: f32,
    light_count: u32,
    pad: f32,
    // xyz position, w range.
    light0: vec4<f32>,
    light1: vec4<f32>,
    // `omni_attenuation` of each light.
    decay: vec2<f32>,
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c / 12.92, c < vec3<f32>(0.04045));
}

// Godot's `omni_attenuation`.
fn omni_attenuation(dist: f32, range: f32, decay: f32) -> f32 {
    var nd = dist / range;
    nd = nd * nd;
    nd = nd * nd;
    nd = max(1.0 - nd, 0.0);
    return nd * nd * pow(max(dist, 0.0001), -decay);
}

// One light's `light()` contribution.
fn light_pass(world: vec3<f32>, light: vec4<f32>, decay: f32, albedo: vec3<f32>, base: vec3<f32>, border: vec3<f32>, darker: vec3<f32>) -> vec3<f32> {
    let att = omni_attenuation(distance(world, light.xyz), light.w, decay);
    let lig = pow(att, 2.0);
    let source = mix(vec3<f32>(0.0), albedo + vec3<f32>(lig * 0.03), lig * 0.9) * room.dim;
    var closest = base;
    if (distance(source, border) < distance(source, closest)) { closest = border; }
    if (distance(source, darker) < distance(source, closest)) { closest = darker; }
    return closest * att;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let weights = pow(abs(normalize(in.world_normal)), vec3<f32>(1.0));
    let total = weights.x + weights.y + weights.z;
    let p = in.world_position.xyz * vec3<f32>(room.uv_scale, -room.uv_scale, room.uv_scale);
    let sampled = (
        textureSample(wall_texture, wall_sampler, p.xy) * weights.z +
        textureSample(wall_texture, wall_sampler, p.xz) * weights.y +
        textureSample(wall_texture, wall_sampler, p.zy * vec2<f32>(-1.0, 1.0)) * weights.x
    ) / total;
    // palette: background_color, border_color, darker_border_color.
    let base = srgb_to_linear(vec3<f32>(0.0431373, 0.0588235, 0.0901961));
    let border = srgb_to_linear(vec3<f32>(0.196078, 0.2, 0.239216));
    let darker_border = srgb_to_linear(vec3<f32>(0.145098, 0.14902, 0.188235));
    // `get_new_tex`: recolor the wall texture's marker greys. They are the
    // PNG's raw sRGB bytes (17, 56, 43), so compare the re-encoded sample.
    let encoded = select(1.055 * pow(sampled.rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), sampled.rgb * 12.92, sampled.rgb < vec3<f32>(0.0031308));
    var albedo = sampled.rgb;
    if (distance(encoded, vec3<f32>(0.066)) < 0.001) { albedo = base; }
    else if (distance(encoded, vec3<f32>(0.219)) < 0.001) { albedo = border; }
    else if (distance(encoded, vec3<f32>(0.168)) < 0.001) { albedo = darker_border; }
    var color = vec3<f32>(0.0);
    if (room.light_count > 0u) {
        color += light_pass(in.world_position.xyz, room.light0, room.decay.x, albedo, base, border, darker_border);
    }
    if (room.light_count > 1u) {
        color += light_pass(in.world_position.xyz, room.light1, room.decay.y, albedo, base, border, darker_border);
    }
    var out: FragmentOutput;
    out.color = vec4<f32>(color, sampled.a);
    return out;
}
