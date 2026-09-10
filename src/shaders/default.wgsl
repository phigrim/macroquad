struct Uniforms {
    Projection: mat4x4<f32>,
    Model: mat4x4<f32>,
    _Time: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var Texture: texture_2d<f32>;
@group(0) @binding(2) var texture_sampler: sampler;
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) color0: vec4<f32>,
};
struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex fn vs_main(vertex: Vertex) -> Varyings {
    var out: Varyings;
    var clip = uniforms.Projection * uniforms.Model * vec4(vertex.position, 1.0);
    clip.z = (clip.z + clip.w) * 0.5;
    out.position = clip;
    out.uv = vertex.texcoord;
    out.color = vertex.color0 / 255.0;
    return out;
}
@fragment fn fs_main(in: Varyings) -> @location(0) vec4<f32> {
    return textureSample(Texture, texture_sampler, in.uv) * in.color;
}
