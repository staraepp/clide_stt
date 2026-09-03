/**
 * clide's ambient background.
 *
 * A pale blue wash over paper — the same light the marketing site's hero sits
 * in. It is almost invisible at rest and only gathers while clide is listening,
 * which makes it part of the "blue means voice" rule rather than decoration.
 *
 * Deliberately cheap: one fullscreen triangle, three fbm evaluations, no
 * textures, no framebuffers.
 */

export const VERTEX_SHADER = /* glsl */ `
attribute vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

export const FRAGMENT_SHADER = /* glsl */ `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

uniform vec2  u_resolution;
uniform float u_time;
/** 0 = reduced, 0.5 = normal, 1 = high. Scales how far the wash can travel. */
uniform float u_intensity;
/** Eased 0..1: how present the wash is. Rises while clide is listening. */
uniform float u_presence;
/** Microphone level, 0..1. Only non-zero at High intensity while dictating. */
uniform float u_energy;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  // Quintic interpolation: no visible grid seams in a slow-moving field.
  vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  return mix(
    mix(hash(i + vec2(0.0, 0.0)), hash(i + vec2(1.0, 0.0)), u.x),
    mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
    u.y
  );
}

float fbm(vec2 p) {
  float total = 0.0;
  float amplitude = 0.5;
  for (int octave = 0; octave < 4; octave++) {
    total += amplitude * noise(p);
    p = p * 2.02 + vec2(1.7, 9.2);
    amplitude *= 0.5;
  }
  return total;
}

void main() {
  vec2 p = (gl_FragCoord.xy * 2.0 - u_resolution) / min(u_resolution.x, u_resolution.y);

  float t = u_time * 0.045;

  // Two rounds of domain warping give the wash its slow drift with no
  // per-frame CPU work.
  vec2 q = vec2(fbm(p * 0.9 + vec2(0.0, t)), fbm(p * 0.9 + vec2(5.2, 1.3 - t)));
  vec2 r = vec2(
    fbm(p + 1.5 * q + vec2(1.7, 9.2) + t * 0.55),
    fbm(p + 1.5 * q + vec2(8.3, 2.8) - t * 0.4)
  );

  float field = clamp(fbm(p + (1.15 + u_energy * 0.4) * r), 0.0, 1.0);

  vec3 paper = vec3(0.957, 0.976, 0.992); // #F4F9FD
  vec3 wash  = vec3(0.847, 0.918, 0.965);
  vec3 deep  = vec3(0.741, 0.859, 0.929);

  // At rest the wash is a hint in the upper left, echoing the site's hero.
  // Presence is what lets it gather while clide is actually listening.
  float resting = smoothstep(1.2, -0.9, p.x + p.y) * 0.42;
  float weight = resting + u_presence * (0.30 + 0.34 * u_intensity);

  vec3 color = mix(paper, wash, clamp(field * weight, 0.0, 1.0));
  color = mix(color, deep, pow(field, 3.0) * u_presence * 0.42);

  // Keep the corners clean so cards never sit on a busy patch.
  float radius = length(p * vec2(0.55, 0.7));
  color = mix(color, paper, clamp(radius * 0.5, 0.0, 0.45));

  gl_FragColor = vec4(color, 1.0);
}
`;
