// 2D point and cubic Bézier helpers
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

pub type Cubic = [P; 4];

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        P { x: self.x + o.x, y: self.y + o.y }
    }
}

impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        P { x: self.x - o.x, y: self.y - o.y }
    }
}

impl Mul<f32> for P {
    type Output = P;
    fn mul(self, s: f32) -> P {
        P { x: self.x * s, y: self.y * s }
    }
}

impl P {
    pub fn polar(a: f32) -> P {
        P { x: a.cos(), y: a.sin() }
    }
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn dot(self, o: P) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn perp(self) -> P {
        P { x: -self.y, y: self.x }
    }
    // +x when degenerate
    pub fn unit(self) -> P {
        let l = self.len();
        if l > 1e-4 {
            self * (1.0 / l)
        } else {
            P { x: 1.0, y: 0.0 }
        }
    }
    pub fn near(self, o: P, r: f32) -> bool {
        (self - o).dot(self - o) < r * r
    }
}

pub fn lerp(a: P, b: P, t: f32) -> P {
    a + (b - a) * t
}

pub fn point(c: &Cubic, t: f32) -> P {
    let m = 1.0 - t;
    c[0] * (m * m * m) + c[1] * (3.0 * m * m * t) + c[2] * (3.0 * m * t * t) + c[3] * (t * t * t)
}

// unnormalized
pub fn tangent(c: &Cubic, t: f32) -> P {
    let m = 1.0 - t;
    (c[1] - c[0]) * (3.0 * m * m) + (c[2] - c[1]) * (6.0 * m * t) + (c[3] - c[2]) * (3.0 * t * t)
}

// [0,t0] and [t1,1] via double De Casteljau
pub fn split(c: &Cubic, t0: f32, t1: f32) -> (Cubic, Cubic) {
    let q0 = lerp(c[0], c[1], t0);
    let q1 = lerp(c[1], c[2], t0);
    let q2 = lerp(c[2], c[3], t0);
    let r0 = lerp(q0, q1, t0);
    let r1 = lerp(q1, q2, t0);
    let a = lerp(r0, r1, t0);

    let u = (t1 - t0) / (1.0 - t0);
    let b0 = lerp(a, r1, u);
    let b1 = lerp(r1, q2, u);
    let b2 = lerp(q2, c[3], u);
    let s0 = lerp(b0, b1, u);
    let s1 = lerp(b1, b2, u);
    ([c[0], q0, r0, a], [lerp(s0, s1, u), s1, b2, c[3]])
}
