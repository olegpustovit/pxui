#version 450
// pxui_render backdrop blur, fragment stage: one separable gaussian pass (direction = tap step) with
// linear sampling. Line-for-line parallel with blur.metal. Rebuild blur.frag.spv with shaders/build.sh.
layout(set = 0, binding = 1) uniform texture2D src;
layout(set = 0, binding = 2) uniform sampler samp;
layout(location = 0) in vec2 v_uv;
layout(location = 1) flat in vec4 v_rect;
layout(location = 2) flat in vec4 v_tap;
layout(location = 3) flat in vec4 v_mode;
layout(location = 0) out vec4 out_color;

void main() {
  vec2 base = v_rect.xy + v_uv * v_rect.zw;
  vec4 sum = vec4(0.0);
  float total = 0.0;
  int r = int(v_tap.w);
  int n = int(v_mode.x);
  if (n < 0) {
    // Cache every horizontal phase of a 4x4 premultiplied source block for the Gaussian taps.
    ivec2 lo = ivec2(int(gl_FragCoord.x) + int(v_rect.x), int(floor(base.y * v_mode.z - 1.5)));
    ivec2 hi = ivec2(v_mode.yz) - 1;
    for (int j = 0; j < 4; j++) {
      for (int i = 0; i < 4; i++) {
        vec4 t = texelFetch(sampler2D(src, samp), clamp(lo + ivec2(i, j), ivec2(0), hi), 0);
        sum += vec4(t.rgb * t.a, t.a);
      }
    }
    // Keep straight RGB in the 8-bit scratch so transparent texels preserve their hue.
    out_color = sum.a > 0.0 ? vec4(sum.rgb / sum.a, sum.a / 16.0) : vec4(0.0); return;
  }

  for (int k = -r; k <= r; k++) {
    float w = exp(-0.5 * float(k * k) / (v_tap.z * v_tap.z));
    vec2 at = base + v_tap.xy * float(k);
    vec4 c;
    if (n == 1) {
      int lo = int(floor(at.x * v_mode.y - 1.5));
      int x = clamp(lo - int(v_mode.w), 0, int(v_mode.z) - 1);
      vec4 t = texelFetch(sampler2D(src, samp), ivec2(x, int(gl_FragCoord.y)), 0);
      c = vec4(t.rgb * t.a, t.a);
    } else if (n > 0) {
      // first pass: the n x n source block around the tap, premultiplied, averaged (straight-alpha source)
      ivec2 lo = ivec2(floor(at * v_mode.yz - 0.5 * float(n) + 0.5)), hi = ivec2(v_mode.yz) - 1;
      c = vec4(0.0);
      for (int j = 0; j < n; j++) {
        for (int i = 0; i < n; i++) {
          vec4 t = texelFetch(sampler2D(src, samp), clamp(lo + ivec2(i, j), ivec2(0), hi), 0);
          c += vec4(t.rgb * t.a, t.a);
        }
      }
      c /= float(n * n);
    } else {
      vec2 half_texel = v_mode.yz;
      c = textureLod(sampler2D(src, samp), clamp(at, half_texel, v_rect.xy + v_rect.zw - half_texel), 0.0);
    }
    sum += w * c;
    total += w;
  }
  sum /= total;
  // the first pass stays premultiplied for the second; the second answers straight alpha
  if (n > 0) { out_color = sum; return; }
  if (sum.a <= 0.0) { out_color = vec4(0.0); return; }
  vec3 rgb = sum.rgb / sum.a;
  float gray = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
  out_color = vec4(mix(vec3(gray), rgb, v_mode.w), sum.a);
}
