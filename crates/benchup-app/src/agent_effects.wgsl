// Shared by retained geometry, precision geometry and departing snapshots.
// Time wraps every 4096 seconds so f32 uniforms retain millisecond precision.
fn agent_age(now: f32, started: f32) -> f32 {
    let delta = now - started;
    return delta - floor(delta / 4096.0) * 4096.0;
}
fn agent_hash(cell: vec2<f32>) -> f32 {
    // Integer mixing avoids transcendental instructions in every fragment of
    // the color, visibility, and ghost passes, and agrees across backends.
    var hash = (bitcast<u32>(i32(cell.x)) * 0x9e3779b9u) ^ (bitcast<u32>(i32(cell.y)) * 0x85ebca6bu);
    hash = (hash ^ (hash >> 16u)) * 0x7feb352du;
    hash = (hash ^ (hash >> 15u)) * 0x846ca68bu;
    return f32((hash ^ (hash >> 16u)) >> 8u) / 16777216.0;
}
fn agent_glyph(cell: vec2<f32>, pixel: vec2<f32>) -> f32 {
    let p = abs(pixel - vec2<f32>(0.5));
    let one = p.x < 0.09 && p.y < 0.34;
    let zero = p.x < 0.27 && p.y < 0.36 && (p.x > 0.14 || p.y > 0.23);
    return select(0.0, 1.0, select(zero, one, agent_hash(cell) > 0.5));
}
fn agent_reveal(stamp: f32, now: f32, uv: vec2<f32>, size: vec2<f32>) -> f32 {
    if stamp < 0.0 { return 1.0; }
    let progress = clamp(agent_age(now, stamp) / 0.95, 0.0, 1.0);
    if progress >= 1.0 { return 1.0; }
    let cell = floor(uv * size / 7.0);
    let threshold = mix(0.0, 0.82, agent_hash(cell)) + (1.0 - uv.y) * 0.18;
    return progress - threshold;
}
fn agent_surface(color: vec4<f32>, stamp: f32, uv: vec2<f32>, size: vec2<f32>, effect: vec4<f32>) -> vec4<f32> {
    if stamp < 0.0 && effect.y < 0.0 { return color; }
    let reveal = agent_reveal(stamp, effect.x, uv, size);
    if reveal < 0.0 { discard; }
    var tint = 0.0;
    if stamp >= 0.0 && agent_age(effect.x, stamp) < 0.95 {
        // Pixel cells and small binary strokes resolve into the real material.
        let pixel = fract(uv * size / 7.0);
        let code = mix(0.18, 0.95, agent_glyph(floor(uv * size / 7.0), pixel));
        tint = (1.0 - smoothstep(0.0, 0.24, reveal)) * code;
    }
    if stamp >= -1.0 && effect.y >= 0.0 {
        let height = max(effect.w - effect.z, 0.01);
        let distance = abs(uv.y - effect.y) / height;
        let band = exp(-distance * distance * 1700.0);
        let line = 1.0 - smoothstep(0.0, 1.8 / max(size.y, 1.0), abs(uv.y - effect.y));
        tint = max(tint, band * 0.38 + line * 0.48);
    }
    return vec4<f32>(mix(color.rgb, vec3<f32>(0.12, 0.85, 1.0) * color.a, clamp(tint, 0.0, 0.9)), color.a);
}
