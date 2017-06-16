#![allow(dead_code)]

use std::f32::consts::PI as PI_32;

use color::{XYZ, SpectralSample, Color};
use math::{Vector, Normal, dot, clamp, zup_to_vec};
use sampling::cosine_sample_hemisphere;
use lerp::lerp;


const INV_PI: f32 = 1.0 / PI_32;
const H_PI: f32 = PI_32 / 2.0;

#[derive(Debug, Copy, Clone)]
pub enum SurfaceClosureUnion {
    EmitClosure(EmitClosure),
    LambertClosure(LambertClosure),
    GTRClosure(GTRClosure),
}

impl SurfaceClosureUnion {
    pub fn as_surface_closure(&self) -> &SurfaceClosure {
        match self {
            &SurfaceClosureUnion::EmitClosure(ref closure) => closure as &SurfaceClosure,
            &SurfaceClosureUnion::LambertClosure(ref closure) => closure as &SurfaceClosure,
            &SurfaceClosureUnion::GTRClosure(ref closure) => closure as &SurfaceClosure,
        }
    }
}

/// Trait for surface closures.
pub trait SurfaceClosure {
    /// Returns whether the closure has a delta distribution or not.
    fn is_delta(&self) -> bool;

    /// Given an incoming ray and sample values, generates an outgoing ray and
    /// color filter.
    ///
    /// inc: Incoming light direction.
    /// nor: The surface normal at the surface point.
    /// uv:  The sampling values.
    /// wavelength: The wavelength of light to sample at.
    ///
    /// Returns a tuple with the generated outgoing light direction, color filter, and pdf.
    fn sample(
        &self,
        inc: Vector,
        nor: Normal,
        uv: (f32, f32),
        wavelength: f32,
    ) -> (Vector, SpectralSample, f32);

    /// Evaluates the closure for the given incoming and outgoing rays.
    ///
    /// inc: The incoming light direction.
    /// out: The outgoing light direction.
    /// nor: The surface normal of the reflecting/transmitting surface point.
    /// wavelength: The wavelength of light to evaluate for.
    ///
    /// Returns the resulting filter color.
    fn evaluate(&self, inc: Vector, out: Vector, nor: Normal, wavelength: f32) -> SpectralSample;

    /// Returns the pdf for the given 'in' direction producing the given 'out'
    /// direction with the given differential geometry.
    ///
    /// inc: The incoming light direction.
    /// out: The outgoing light direction.
    /// nor: The surface normal of the reflecting/transmitting surface point.
    fn sample_pdf(&self, inc: Vector, out: Vector, nor: Normal) -> f32;

    /// Returns an estimate of the sum total energy that evaluate() would return
    /// when 'out' is evaluated over a circular solid angle.
    /// This is used for importance sampling, so does not need to be exact,
    /// but it does need to be non-zero anywhere that an exact solution would
    /// be non-zero.
    fn estimate_eval_over_solid_angle(
        &self,
        inc: Vector,
        out: Vector,
        nor: Normal,
        cos_theta: f32,
    ) -> f32;
}


/// Utility function that calculates the fresnel reflection factor of a given
/// incoming ray against a surface with the given ior outside/inside ratio.
///
/// ior_ratio: The ratio of the outside material ior (probably 1.0 for air)
///            over the inside ior.
/// c: The cosine of the angle between the incoming light and the
///    surface's normal.  Probably calculated e.g. with a normalized
///    dot product.
#[allow(dead_code)]
fn dielectric_fresnel(ior_ratio: f32, c: f32) -> f32 {
    let g = (ior_ratio - 1.0 + (c * c)).sqrt();

    let f1 = g - c;
    let f2 = g + c;
    let f3 = (f1 * f1) / (f2 * f2);

    let f4 = (c * f2) - 1.0;
    let f5 = (c * f1) + 1.0;
    let f6 = 1.0 + ((f4 * f4) / (f5 * f5));

    return 0.5 * f3 * f6;
}


/// Schlick's approximation of the fresnel reflection factor.
///
/// Same interface as dielectric_fresnel(), above.
#[allow(dead_code)]
fn schlick_fresnel(ior_ratio: f32, c: f32) -> f32 {
    let f1 = (1.0 - ior_ratio) / (1.0 + ior_ratio);
    let f2 = f1 * f1;
    let c1 = 1.0 - c;
    let c2 = c1 * c1;
    return f2 + ((1.0 - f2) * c1 * c2 * c2);
}


/// Utility function that calculates the fresnel reflection factor of a given
/// incoming ray against a surface with the given normal-reflectance factor.
///
/// frensel_fac: The ratio of light reflected back if the ray were to
///              hit the surface head-on (perpendicular to the surface).
/// c The cosine of the angle between the incoming light and the
///   surface's normal.  Probably calculated e.g. with a normalized
///   dot product.
#[allow(dead_code)]
fn dielectric_fresnel_from_fac(fresnel_fac: f32, c: f32) -> f32 {
    let tmp1 = fresnel_fac.sqrt() - 1.0;

    // Protect against divide by zero.
    if tmp1.abs() < 0.000001 {
        return 1.0;
    }

    // Find the ior ratio
    let tmp2 = (-2.0 / tmp1) - 1.0;
    let ior_ratio = tmp2 * tmp2;

    // Calculate fresnel factor
    return dielectric_fresnel(ior_ratio, c);
}


/// Schlick's approximation version of dielectric_fresnel_from_fac() above.
#[allow(dead_code)]
fn schlick_fresnel_from_fac(frensel_fac: f32, c: f32) -> f32 {
    let c1 = 1.0 - c;
    let c2 = c1 * c1;
    return frensel_fac + ((1.0 - frensel_fac) * c1 * c2 * c2);
}


/// Emit closure.
///
/// NOTE: this needs to be handled specially by the integrator!  It does not
/// behave like a standard closure!
#[derive(Debug, Copy, Clone)]
pub struct EmitClosure {
    col: XYZ,
}

impl EmitClosure {
    pub fn emitted_color(&self, wavelength: f32) -> SpectralSample {
        self.col.to_spectral_sample(wavelength)
    }
}

impl SurfaceClosure for EmitClosure {
    fn is_delta(&self) -> bool {
        false
    }

    fn sample(
        &self,
        inc: Vector,
        nor: Normal,
        uv: (f32, f32),
        wavelength: f32,
    ) -> (Vector, SpectralSample, f32) {
        let _ = (inc, nor, uv); // Not using these, silence warning

        (
            Vector::new(0.0, 0.0, 0.0),
            SpectralSample::new(wavelength),
            1.0,
        )
    }

    fn evaluate(&self, inc: Vector, out: Vector, nor: Normal, wavelength: f32) -> SpectralSample {
        let _ = (inc, out, nor); // Not using these, silence warning

        SpectralSample::new(wavelength)
    }

    fn sample_pdf(&self, inc: Vector, out: Vector, nor: Normal) -> f32 {
        let _ = (inc, out, nor); // Not using these, silence warning

        1.0
    }

    fn estimate_eval_over_solid_angle(
        &self,
        inc: Vector,
        out: Vector,
        nor: Normal,
        cos_theta: f32,
    ) -> f32 {
        let _ = (inc, out, nor, cos_theta); // Not using these, silence warning

        // TODO: what to do here?
        unimplemented!()
    }
}


/// Lambertian surface closure
#[derive(Debug, Copy, Clone)]
pub struct LambertClosure {
    col: XYZ,
}

impl LambertClosure {
    pub fn new(col: XYZ) -> LambertClosure {
        LambertClosure { col: col }
    }
}

impl SurfaceClosure for LambertClosure {
    fn is_delta(&self) -> bool {
        false
    }

    fn sample(
        &self,
        inc: Vector,
        nor: Normal,
        uv: (f32, f32),
        wavelength: f32,
    ) -> (Vector, SpectralSample, f32) {
        let nn = if dot(nor.into_vector(), inc) <= 0.0 {
            nor.normalized()
        } else {
            -nor.normalized()
        }.into_vector();

        // Generate a random ray direction in the hemisphere
        // of the surface.
        let dir = cosine_sample_hemisphere(uv.0, uv.1);
        let pdf = dir.z() * INV_PI;
        let out = zup_to_vec(dir, nn);
        let filter = self.evaluate(inc, out, nor, wavelength);

        (out, filter, pdf)
    }

    fn evaluate(&self, inc: Vector, out: Vector, nor: Normal, wavelength: f32) -> SpectralSample {
        let v = out.normalized();
        let nn = if dot(nor.into_vector(), inc) <= 0.0 {
            nor.normalized()
        } else {
            -nor.normalized()
        }.into_vector();
        let fac = dot(nn, v).max(0.0) * INV_PI;

        self.col.to_spectral_sample(wavelength) * fac
    }

    fn sample_pdf(&self, inc: Vector, out: Vector, nor: Normal) -> f32 {
        let v = out.normalized();
        let nn = if dot(nor.into_vector(), inc) <= 0.0 {
            nor.normalized()
        } else {
            -nor.normalized()
        }.into_vector();

        dot(nn, v).max(0.0) * INV_PI
    }

    fn estimate_eval_over_solid_angle(
        &self,
        inc: Vector,
        out: Vector,
        nor: Normal,
        cos_theta: f32,
    ) -> f32 {
        assert!(cos_theta >= -1.0 && cos_theta <= 1.0);

        // Analytically calculates lambert shading from a uniform light source
        // subtending a circular solid angle.
        // Only works for solid angle subtending equal to or less than a hemisphere.
        //
        // Formula taken from "Area Light Sources for Real-Time Graphics"
        // by John M. Snyder
        fn sphere_lambert(nlcos: f32, rcos: f32) -> f32 {
            assert!(nlcos >= -1.0 && nlcos <= 1.0);
            assert!(rcos >= 0.0 && rcos <= 1.0);

            let nlsin: f32 = (1.0 - (nlcos * nlcos)).sqrt();
            let rsin2: f32 = 1.0 - (rcos * rcos);
            let rsin: f32 = rsin2.sqrt();
            let ysin: f32 = rcos / nlsin;
            let ycos2: f32 = 1.0 - (ysin * ysin);
            let ycos: f32 = ycos2.sqrt();

            let g: f32 = (-2.0 * nlsin * rcos * ycos) + H_PI - ysin.asin() + (ysin * ycos);
            let h: f32 = nlcos * ((ycos * (rsin2 - ycos2).sqrt()) + (rsin2 * (ycos / rsin).asin()));

            let nl: f32 = nlcos.acos();
            let r: f32 = rcos.acos();

            if nl < (H_PI - r) {
                nlcos * rsin2
            } else if nl < H_PI {
                (nlcos * rsin2) + g - h
            } else if nl < (H_PI + r) {
                (g + h) * INV_PI
            } else {
                0.0
            }
        }

        if cos_theta < 0.0 {
            return 1.0;
        } else {
            let v = out.normalized();
            let nn = if dot(nor.into_vector(), inc) <= 0.0 {
                nor.normalized()
            } else {
                -nor.normalized()
            }.into_vector();

            let cos_nv = dot(nn, v).max(-1.0).min(1.0);

            return sphere_lambert(cos_nv, cos_theta);
        }
    }
}


/// The GTR microfacet BRDF from the Disney Principled BRDF paper.
#[derive(Debug, Copy, Clone)]
pub struct GTRClosure {
    col: XYZ,
    roughness: f32,
    tail_shape: f32,
    fresnel: f32, // [0.0, 1.0] determines how much fresnel reflection comes into play
    normalization_factor: f32,
}

impl GTRClosure {
    pub fn new(col: XYZ, roughness: f32, tail_shape: f32, fresnel: f32) -> GTRClosure {
        let mut closure = GTRClosure {
            col: col,
            roughness: roughness,
            tail_shape: tail_shape,
            fresnel: fresnel,
            normalization_factor: GTRClosure::normalization(roughness, tail_shape),
        };

        closure.validate();

        closure
    }

    // Returns the normalization factor for the distribution function
    // of the BRDF.
    fn normalization(r: f32, t: f32) -> f32 {
        let r2 = r * r;
        let top = (t - 1.0) * (r2 - 1.0);
        let bottom = PI_32 * (1.0 - r2.powf(1.0 - t));
        top / bottom
    }

    // Makes sure values are in a valid range
    fn validate(&mut self) {
        debug_assert!(self.fresnel >= 0.0 && self.fresnel <= 1.0);

        // Clamp values to valid ranges
        self.roughness = clamp(self.roughness, 0.0, 0.9999);
        self.tail_shape = (0.0001f32).max(self.tail_shape);

        // When roughness is too small, but not zero, there are floating point accuracy issues
        if self.roughness < 0.000244140625 {
            // (2^-12)
            self.roughness = 0.0;
        }

        // If tail_shape is too near 1.0, push it away a tiny bit.
        // This avoids having to have a special form of various equations
        // due to a singularity at tail_shape = 1.0
        // That in turn avoids some branches in the code, and the effect of
        // tail_shape is sufficiently subtle that there is no visible
        // difference in renders.
        const TAIL_EPSILON: f32 = 0.0001;
        if (self.tail_shape - 1.0).abs() < TAIL_EPSILON {
            self.tail_shape = 1.0 + TAIL_EPSILON;
        }

        // Precalculate normalization factor
        self.normalization_factor = GTRClosure::normalization(self.roughness, self.tail_shape);
    }

    // Returns the cosine of the half-angle that should be sampled, given
    // a random variable in [0,1]
    fn half_theta_sample(&self, u: f32) -> f32 {
        let roughness2 = self.roughness * self.roughness;

        // Calculate top half of equation
        let top = 1.0 -
            ((roughness2.powf(1.0 - self.tail_shape) * (1.0 - u)) + u)
                .powf(1.0 / (1.0 - self.tail_shape));

        // Calculate bottom half of equation
        let bottom = 1.0 - roughness2;

        (top / bottom).sqrt()
    }

    /// Microfacet distribution function.
    ///
    /// nh: cosine of the angle between the surface normal and the microfacet normal.
    fn dist(&self, nh: f32, rough: f32) -> f32 {
        // Other useful numbers
        let roughness2 = rough * rough;

        // Calculate D - Distribution
        let dist = if nh <= 0.0 {
            0.0
        } else {
            let nh2 = nh * nh;
            self.normalization_factor / (1.0 + ((roughness2 - 1.0) * nh2)).powf(self.tail_shape)
        };

        dist
    }
}

impl SurfaceClosure for GTRClosure {
    fn is_delta(&self) -> bool {
        self.roughness == 0.0
    }


    fn sample(
        &self,
        inc: Vector,
        nor: Normal,
        uv: (f32, f32),
        wavelength: f32,
    ) -> (Vector, SpectralSample, f32) {
        // Get normalized surface normal
        let nn = if dot(nor.into_vector(), inc) < 0.0 {
            nor.normalized()
        } else {
            -nor.normalized() // If back-facing, flip normal
        }.into_vector();

        // Generate a random ray direction in the hemisphere
        // of the surface.
        let theta_cos = self.half_theta_sample(uv.0);
        let theta_sin = (1.0 - (theta_cos * theta_cos)).sqrt();
        let angle = uv.1 * PI_32 * 2.0;
        let mut half_dir = Vector::new(angle.cos() * theta_sin, angle.sin() * theta_sin, theta_cos);
        half_dir = zup_to_vec(half_dir, nn).normalized();

        let out = inc - (half_dir * 2.0 * dot(inc, half_dir));
        let filter = self.evaluate(inc, out, nor, wavelength);
        let pdf = self.sample_pdf(inc, out, nor);

        (out, filter, pdf)
    }


    fn evaluate(&self, inc: Vector, out: Vector, nor: Normal, wavelength: f32) -> SpectralSample {
        // Calculate needed vectors, normalized
        let aa = -inc.normalized(); // Vector pointing to where "in" came from
        let bb = out.normalized(); // Out
        let hh = (aa + bb).normalized(); // Half-way between aa and bb

        // Surface normal
        let nn = if dot(nor.into_vector(), hh) < 0.0 {
            -nor.normalized() // If back-facing, flip normal
        } else {
            nor.normalized()
        }.into_vector();

        // Calculate needed dot products
        let na = clamp(dot(nn, aa), -1.0, 1.0);
        let nb = clamp(dot(nn, bb), -1.0, 1.0);
        let ha = clamp(dot(hh, aa), -1.0, 1.0);
        let hb = clamp(dot(hh, bb), -1.0, 1.0);
        let nh = clamp(dot(nn, hh), -1.0, 1.0);

        // Other useful numbers
        let roughness2 = self.roughness * self.roughness;

        // Calculate F - Fresnel
        let col_f = {
            let mut col_f = self.col.to_spectral_sample(wavelength);

            let rev_fresnel = 1.0 - self.fresnel;
            let c0 = lerp(
                schlick_fresnel_from_fac(col_f.e.get_0(), hb),
                col_f.e.get_0(),
                rev_fresnel,
            );
            let c1 = lerp(
                schlick_fresnel_from_fac(col_f.e.get_1(), hb),
                col_f.e.get_1(),
                rev_fresnel,
            );
            let c2 = lerp(
                schlick_fresnel_from_fac(col_f.e.get_2(), hb),
                col_f.e.get_2(),
                rev_fresnel,
            );
            let c3 = lerp(
                schlick_fresnel_from_fac(col_f.e.get_3(), hb),
                col_f.e.get_3(),
                rev_fresnel,
            );

            col_f.e.set_0(c0);
            col_f.e.set_1(c1);
            col_f.e.set_2(c2);
            col_f.e.set_3(c3);

            col_f
        };

        // Calculate everything else
        if self.roughness == 0.0 {
            // If sharp mirror, just return col * fresnel factor
            return col_f;
        } else {
            // Calculate D - Distribution
            let dist = if nh > 0.0 {
                let nh2 = nh * nh;
                self.normalization_factor / (1.0 + ((roughness2 - 1.0) * nh2)).powf(self.tail_shape)
            } else {
                0.0
            };

            // Calculate G1 - Geometric microfacet shadowing
            let g1 = {
                let na2 = na * na;
                let tan_na = ((1.0 - na2) / na2).sqrt();
                let g1_pos_char = if (ha * na) > 0.0 { 1.0 } else { 0.0 };
                let g1_a = roughness2 * tan_na;
                let g1_b = ((1.0 + (g1_a * g1_a)).sqrt() - 1.0) * 0.5;
                g1_pos_char / (1.0 + g1_b)
            };

            // Calculate G2 - Geometric microfacet shadowing
            let g2 = {
                let nb2 = nb * nb;
                let tan_nb = ((1.0 - nb2) / nb2).sqrt();
                let g2_pos_char = if (hb * nb) > 0.0 { 1.0 } else { 0.0 };
                let g2_a = roughness2 * tan_nb;
                let g2_b = ((1.0 + (g2_a * g2_a)).sqrt() - 1.0) * 0.5;
                g2_pos_char / (1.0 + g2_b)
            };

            // Final result
            return col_f * (dist * g1 * g2) * INV_PI;
        }
    }


    fn sample_pdf(&self, inc: Vector, out: Vector, nor: Normal) -> f32 {
        // Calculate needed vectors, normalized
        let aa = -inc.normalized(); // Vector pointing to where "in" came from
        let bb = out.normalized(); // Out
        let hh = (aa + bb).normalized(); // Half-way between aa and bb

        // Surface normal
        let nn = if dot(nor.into_vector(), hh) < 0.0 {
            -nor.normalized() // If back-facing, flip normal
        } else {
            nor.normalized()
        }.into_vector();

        // Calculate needed dot products
        let nh = clamp(dot(nn, hh), -1.0, 1.0);

        return self.dist(nh, self.roughness) * INV_PI;
    }


    fn estimate_eval_over_solid_angle(
        &self,
        inc: Vector,
        out: Vector,
        nor: Normal,
        cos_theta: f32,
    ) -> f32 {
        // TODO: all of the stuff in this function is horribly hacky.
        // Find a proper way to approximate the light contribution from a
        // solid angle.
        assert!(cos_theta >= -1.0);
        assert!(cos_theta <= 1.0);

        // Surface normal
        let nn = if dot(nor.into_vector(), inc) < 0.0 {
            nor.normalized()
        } else {
            -nor.normalized() // If back-facing, flip normal
        }.into_vector();

        let aa = -inc.normalized(); // Vector pointing to where "in" came from
        let bb = out.normalized(); // Out

        // Brute-force method
        //let mut fac = 0.0;
        //const N: usize = 256;
        //for i in 0..N {
        //    let uu = Halton::sample(0, i);
        //    let vv = Halton::sample(1, i);
        //    let mut samp = uniform_sample_cone(uu, vv, cos_theta);
        //    samp = zup_to_vec(samp, bb).normalized();
        //    if dot(nn, samp) > 0.0 {
        //        let hh = (aa+samp).normalized();
        //        fac += self.dist(dot(nn, hh), roughness);
        //    }
        //}
        //fac /= N * N;

        // Approximate method
        let theta = cos_theta.acos();
        let hh = (aa + bb).normalized();
        let nh = clamp(dot(nn, hh), -1.0, 1.0);
        let fac = self.dist(
            nh,
            (1.0f32).min(self.roughness.sqrt() + (2.0 * theta / PI_32)),
        );

        return fac * (1.0f32).min(1.0 - cos_theta) * INV_PI;
    }
}
