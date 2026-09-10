@group(0) @binding(1) var Texture: texture_2d<f32>;
@group(0) @binding(2) var texture_sampler: sampler;
struct Vertex { @location(0) position: vec2<f32>, @location(1) texcoord: vec2<f32> };
struct Varyings { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(vertex: Vertex) -> Varyings {
    var out: Varyings;
    out.position = vec4(vertex.position, 0.5, 1.0);
    out.uv = vertex.texcoord;
    return out;
}
@fragment fn fs_main(in: Varyings) -> @location(0) vec4<f32> {
    return textureSample(Texture, texture_sampler, in.uv);
}
