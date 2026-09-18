//! Icone desenhado em codigo: um asterisco de oito raios, na cor da conta.
//!
//! A cor e' o que faz o icone valer a pena - de relance, sem abrir o menu,
//! da' para saber em qual conta a proxima sessao vai nascer.

/// Paleta escolhida para cores distinguiveis entre si e sobre bandejas
/// claras ou escuras.
const PALETTE: [[u8; 3]; 6] = [
    [0xD9, 0x77, 0x57], // terracota
    [0x3B, 0x82, 0xF6], // azul
    [0x10, 0xB9, 0x81], // verde
    [0x8B, 0x5C, 0xF6], // roxo
    [0xEC, 0x48, 0x99], // rosa
    [0xF5, 0x9E, 0x0B], // ambar
];

/// Mesma conta, mesma cor, sempre - inclusive entre maquinas.
pub fn color_for(name: &str) -> [u8; 3] {
    let mut h: u32 = 2166136261;
    for b in name.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    PALETTE[(h as usize) % PALETTE.len()]
}

/// Ponto dentro do raio de meia-largura `half`, girado por `angle`?
fn in_ray(x: f32, y: f32, angle: f32, len: f32, half: f32) -> bool {
    let (s, c) = angle.sin_cos();
    // Leva o ponto ao referencial do raio.
    let u = x * c + y * s; // ao longo
    let v = -x * s + y * c; // atravessado
    if !(0.0..=len).contains(&u) {
        return false;
    }
    // Afina na ponta: e' o que da' o formato de raio, em vez de barra.
    let taper = half * (1.0 - 0.55 * (u / len));
    v.abs() <= taper
}

/// Gera RGBA quadrado do tamanho pedido. Antialias por supersampling 3x3.
pub fn rgba(size: u32, color: [u8; 3]) -> Vec<u8> {
    let n = size as f32;
    let center = n / 2.0;
    let len = n * 0.44;
    let half = n * 0.085;
    let mut out = vec![0u8; (size * size * 4) as usize];

    for py in 0..size {
        for px in 0..size {
            let mut hits = 0u32;
            for sy in 0..3 {
                for sx in 0..3 {
                    let x = px as f32 + (sx as f32 + 0.5) / 3.0 - center;
                    let y = py as f32 + (sy as f32 + 0.5) / 3.0 - center;
                    let mut on = false;
                    for k in 0..8 {
                        let a = std::f32::consts::PI * 2.0 * (k as f32) / 8.0;
                        if in_ray(x, y, a, len, half) {
                            on = true;
                            break;
                        }
                    }
                    if on {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let i = ((py * size + px) * 4) as usize;
                out[i] = color[0];
                out[i + 1] = color[1];
                out[i + 2] = color[2];
                out[i + 3] = (255 * hits / 9) as u8;
            }
        }
    }
    out
}

/// ksni espera ARGB32 em ordem de rede (big endian).
#[cfg(target_os = "linux")]
pub fn argb(size: u32, color: [u8; 3]) -> Vec<u8> {
    let rgba = rgba(size, color);
    let mut out = Vec::with_capacity(rgba.len());
    // RGBA vira ARGB: o alfa sai da última posição para a primeira.
    for i in (0..rgba.len()).step_by(4) {
        out.extend_from_slice(&[rgba[i + 3], rgba[i], rgba[i + 1], rgba[i + 2]]);
    }
    out
}
