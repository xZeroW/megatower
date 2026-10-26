// Native staging port of room_normal.tscn's triplanar palette remap. The
// source light() attenuation quantization is intentionally not claimed here.
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> room: RoomMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var wall_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var wall_sampler: sampler;

struct RoomMaterial {
    dim: f32,
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c / 12.92, c < vec3<f32>(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let weights = pow(abs(normalize(in.world_normal)), vec3<f32>(1.0));
    let total = weights.x + weights.y + weights.z;
    let p = in.world_position.xyz * vec3<f32>(1.75, -1.75, 1.75);
    let sampled = (
        textureSample(wall_texture, wall_sampler, p.xy) * weights.z +
        textureSample(wall_texture, wall_sampler, p.xz) * weights.y +
        textureSample(wall_texture, wall_sampler, p.zy * vec2<f32>(-1.0, 1.0)) * weights.x
    ) / total;
    // palette: background_color, border_color, darker_border_color (sRGB -> linear,
    // `fix_colors`).
    let base = srgb_to_linear(vec3<f32>(0.0431373, 0.0588235, 0.0901961));
    let border = srgb_to_linear(vec3<f32>(0.196078, 0.2, 0.239216));
    let darker_border = srgb_to_linear(vec3<f32>(0.145098, 0.14902, 0.188235));
    // `get_new_tex`: recolor the wall texture's three marker greys.
    // The marker greys are the PNG's raw sRGB bytes (17, 56, 43), so compare
    // against the re-encoded sample rather than the linear one.
    let encoded = select(1.055 * pow(sampled.rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), sampled.rgb * 12.92, sampled.rgb < vec3<f32>(0.0031308));
    var albedo = sampled.rgb;
    if (distance(encoded, vec3<f32>(0.066)) < 0.001) { albedo = base; }
    else if (distance(encoded, vec3<f32>(0.219)) < 0.001) { albedo = border; }
    else if (distance(encoded, vec3<f32>(0.168)) < 0.001) { albedo = darker_border; }
    // `light()`: AnimatedLight omni (range 14.763, attenuation 0.05).
    let light_pos = vec3<f32>(0.0, -1.04679, -0.340801);
    let dist = max(distance(in.world_position.xyz, light_pos), 0.0001);
    var nd = dist / 14.763;
    nd = nd * nd;
    nd = nd * nd;
    nd = max(1.0 - nd, 0.0);
    let attenuation = nd * nd * pow(dist, -0.05);
    let lig = pow(attenuation, 2.0);
    let source = mix(vec3<f32>(0.0), albedo + vec3<f32>(lig * 0.03), lig * 0.9) * room.dim;
    var closest = base;
    if (distance(source, border) < distance(source, closest)) { closest = border; }
    if (distance(source, darker_border) < distance(source, closest)) { closest = darker_border; }
    var out: FragmentOutput;
    out.color = vec4<f32>(closest, sampled.a);
    return out;
}
