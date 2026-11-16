struct Uniform {
    matrix: mat4x4<f32>,
    viewport: vec4<f32>,
    robot: vec4<f32>,
    animation: vec4<f32>,
    paper: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniform;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) clip: vec3<f32>,
    @location(2) @interpolate(flat) kind: u32,
}
fn agent_target(color: vec4<f32>) -> vec4<f32> {
    if u.viewport.w > 0.0 {
        return vec4<f32>(finish_srgb_to_linear(color.rgb), color.a);
    }
    return color;
}
@vertex fn robot_vertex(@builtin(vertex_index) vertex: u32) -> Output {
    let corners = array<vec2<f32>, 6>(vec2(-1.,-1.), vec2(1.,-1.), vec2(-1.,1.), vec2(-1.,1.), vec2(1.,-1.), vec2(1.,1.));
    let uv = corners[vertex];
    let bob = sin(u.animation.y * 2.5) * 2.0;
    let pixel = u.robot.xy + vec2<f32>(0.0,bob) + uv * u.robot.z;
    var out: Output;
    out.position = vec4<f32>(pixel / u.viewport.xy * vec2<f32>(2.0,-2.0) + vec2<f32>(-1.0,1.0),0.0,1.0);
    out.uv = uv; out.clip = vec3<f32>(0.0); out.kind = 0u;
    return out;
}
fn box_distance(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - size + vec2<f32>(radius);
    return min(max(q.x,q.y),0.0) + length(max(q,vec2<f32>(0.0))) - radius;
}
fn fill(distance: f32, aa: f32) -> f32 { return 1.0-smoothstep(-aa,aa,distance); }
@fragment fn robot_color(in: Output) -> @location(0) vec4<f32> {
    let p = in.uv;
    let aa = 1.0 / (u.robot.z * u.viewport.z);
    let blue = vec3<f32>(0.12,0.85,1.0);
    let ink = vec3<f32>(0.04,0.15,0.21);
    // Quiet circular badge, antenna, ears, head and two tracking eyes.
    var color = u.paper.rgb;
    var alpha = fill(length(p)-0.87,aa) * 0.94;
    let ring = fill(abs(length(p)-0.84)-0.015,aa);
    color = mix(color,blue,ring*0.55);
    let antenna = min(box_distance(p-vec2<f32>(0.0,-0.51),vec2<f32>(0.018,0.1),0.01),length(p-vec2<f32>(0.0,-0.63))-0.055);
    let ears = box_distance(vec2<f32>(abs(p.x)-0.46,p.y+0.08),vec2<f32>(0.055,0.13),0.035);
    color = mix(color,blue,max(fill(antenna,aa),fill(ears,aa)));
    let head = box_distance(p-vec2<f32>(0.0,-0.08),vec2<f32>(0.4,0.32),0.10);
    color = mix(color,blue,fill(head,aa));
    let face = box_distance(p-vec2<f32>(0.0,-0.09),vec2<f32>(0.32,0.23),0.065);
    color = mix(color,ink,fill(face,aa));
    let look = sin(u.animation.y*2.1)*0.06;
    let blink = 0.022 + 0.06 * smoothstep(0.04,0.13,abs(sin(u.animation.y*1.8)));
    let eye = box_distance(vec2<f32>(abs(p.x-look)-0.145,p.y+0.13),vec2<f32>(0.055,blink),0.018);
    color = mix(color,blue,fill(eye,aa));
    let mouth = box_distance(p-vec2<f32>(0.0,0.035),vec2<f32>(0.10,0.016),0.01);
    color = mix(color,blue,fill(mouth,aa)*0.75);
    // Preparation uses a rotating, toothed gear under the same robot.
    let q = p-vec2<f32>(0.0,0.51);
    let radius = length(q);
    if u.robot.w > 0.5 {
        let angle = atan2(q.y,q.x) - u.animation.y*2.6;
        let teeth = 0.175 + 0.033*smoothstep(-0.2,0.2,cos(angle*8.0));
        let gear = fill(radius-teeth,aa) * (1.0-fill(radius-0.072,aa));
        color = mix(color,blue,gear);
    } else {
        // A small radar arc follows the looking motion.
        let angle = atan2(q.y,q.x) + u.animation.y*2.3;
        let radar = fill(abs(radius-0.15)-0.018,aa) * smoothstep(-0.7,0.9,cos(angle));
        color = mix(color,blue,radar);
    }
    if alpha < 0.001 { discard; }
    return agent_target(vec4<f32>(color,1.0)) * alpha;
}
@vertex fn ghost_vertex(@location(0) a: vec3<f32>, @location(1) b: vec3<f32>, @location(2) corner: vec2<f32>, @location(3) kind: u32) -> Output {
    var out: Output;
    var clip = u.matrix * vec4<f32>(a,1.0);
    if kind == 1u {
        var end = u.matrix * vec4<f32>(b,1.0);
        if clip.z < 0.0 && end.z >= 0.0 { clip=mix(clip,end,clip.z/(clip.z-end.z)); }
        if end.z < 0.0 && clip.z >= 0.0 { end=mix(end,clip,end.z/(end.z-clip.z)); }
        let delta = (end.xy / max(end.w,0.00001) - clip.xy / max(clip.w,0.00001)) * u.viewport.xy;
        let normal = vec2<f32>(-delta.y,delta.x)/max(length(delta),0.001);
        clip = mix(clip,end,corner.x);
        clip = vec4<f32>(clip.xy + normal*corner.y*2.0/u.viewport.xy*clip.w,clip.zw);
    }
    out.position=clip; out.clip=clip.xyw; out.uv=corner; out.kind=kind;
    return out;
}
@fragment fn ghost_color(in: Output) -> @location(0) vec4<f32> {
    let uv = in.clip.xy / in.clip.z * vec2<f32>(0.5,-0.5) + vec2<f32>(0.5);
    let progress = u.animation.x;
    let cell = floor(uv * u.viewport.xy / 7.0);
    let noise = agent_hash(cell);
    if noise < progress { discard; }
    let pixel = fract(uv * u.viewport.xy / 7.0);
    let glyph = mix(0.2,0.9,agent_glyph(cell,pixel));
    var alpha = (1.0-progress)*select(0.32*glyph,0.8,in.kind==1u);
    if in.kind==1u { alpha*=1.0-smoothstep(0.6,1.0,abs(in.uv.y)); }
    let color = agent_target(vec4<f32>(1.0,0.48,0.27,1.0));
    return color * alpha;
}
