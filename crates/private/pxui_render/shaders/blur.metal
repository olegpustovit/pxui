#include <metal_stdlib>
using namespace metal;
// pxui_render backdrop blur, Metal. Line-for-line parallel with blur.vert + blur.frag; layout in src/lib.rs.
struct BOut {
  float4 position [[position]];
  float2 uv;
  float4 rect [[flat]];
  float4 tap [[flat]];
  float4 mode [[flat]];
};

vertex BOut blur_vertex(uint vid [[vertex_id]], device const float4* g [[buffer(0)]]) {
  BOut o;
  float2 uv = float2(float((vid << 1) & 2u), float(vid & 2u));
  o.position = float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
  o.uv = uv;
  o.rect = g[0];  // source region in uv: x y w h
  o.tap = g[1];   // uv step per tap x y, sigma in taps, tap radius
  o.mode = g[2];  // block side (0: second pass), source width/height or half-texel UV, saturation
  return o;
}

fragment float4 blur_fragment(BOut in [[stage_in]], texture2d<float> src [[texture(1)]], sampler samp [[sampler(2)]]) {
  float2 base = in.rect.xy + in.uv * in.rect.zw;
  float4 sum = float4(0.0);
  float total = 0.0;
  int r = int(in.tap.w);
  int n = int(in.mode.x);
  for (int k = -r; k <= r; k++) {
    float w = exp(-0.5 * float(k * k) / (in.tap.z * in.tap.z));
    float2 at = base + in.tap.xy * float(k);
    float4 c;
    if (n > 0) {
      // first pass: the n x n source block around the tap, premultiplied, averaged (straight-alpha source)
      int2 lo = int2(floor(at * in.mode.yz - 0.5 * float(n) + 0.5)), hi = int2(in.mode.yz) - 1;
      c = float4(0.0);
      for (int j = 0; j < n; j++) {
        for (int i = 0; i < n; i++) {
          float4 t = src.read(uint2(clamp(lo + int2(i, j), int2(0), hi)));
          c += float4(t.rgb * t.a, t.a);
        }
      }
      c /= float(n * n);
    } else {
      float2 half_texel = in.mode.yz;
      c = src.sample(samp, clamp(at, half_texel, in.rect.xy + in.rect.zw - half_texel), level(0.0));
    }
    sum += w * c;
    total += w;
  }
  sum /= total;
  // the first pass stays premultiplied for the second; the second answers straight alpha
  if (n > 0) return sum;
  if (sum.a <= 0.0) return float4(0.0);
  float3 rgb = sum.rgb / sum.a;
  float gray = dot(rgb, float3(0.2126, 0.7152, 0.0722));
  return float4(mix(float3(gray), rgb, in.mode.w), sum.a);
}
