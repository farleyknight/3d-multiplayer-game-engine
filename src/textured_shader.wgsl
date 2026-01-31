// Vertex shader and fragment shader for textured 3D cube rendering with directional lighting

struct Uniforms {
    mvp: mat4x4<f32>,
};

struct LightUniforms {
    sun_direction: vec3<f32>,
    _padding1: f32,
    sun_color: vec3<f32>,
    ambient_strength: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

@group(0) @binding(3)
var<uniform> light: LightUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);

    // Normalize the normal vector
    let normal = normalize(in.normal);

    // Calculate diffuse lighting (Lambert)
    // sun_direction points FROM the sun TO the scene, so negate it
    let light_dir = normalize(-light.sun_direction);
    let diffuse = max(dot(normal, light_dir), 0.0);

    // Combine ambient and diffuse lighting
    let ambient = light.ambient_strength;
    let lighting = ambient + (1.0 - ambient) * diffuse;

    // Apply lighting to the texture color multiplied by vertex color for tinting
    let base_color = tex_color.rgb * in.color;
    let lit_color = base_color * light.sun_color * lighting;

    return vec4<f32>(lit_color, tex_color.a);
}
