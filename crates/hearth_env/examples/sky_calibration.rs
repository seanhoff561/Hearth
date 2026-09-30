//! Prints the clear-sky illuminance of the atmosphere model against the sun's elevation, for
//! calibrating it against measurements (CIE clear sky; twilight tables: ~750 lx at sunset,
//! 3.4 lx at the end of civil twilight, 0.008 lx at the end of nautical twilight). See D32.
//!
//! `cargo run --release -p hearth_env --example sky_calibration`

use glam::DVec3;
use hearth_env::sky::{Atmosphere, SUN_ILLUMINANCE};

fn main() {
    for haze in [1.0, 3.0] {
        let atm = Atmosphere::new(haze);
        println!("haze {haze}");
        for elev in [
            60.0f64, 30.0, 10.0, 0.0, -2.0, -4.0, -6.0, -8.0, -10.0, -12.0,
        ] {
            let e = elev.to_radians();
            let sun = DVec3::new(e.cos(), e.sin(), 0.0);
            let diffuse = atm.sky_irradiance_with(80.0, sun, 32, 16, 64);
            let direct = atm.transmittance(6_360_080.0, e.sin());
            let direct_h = direct[1] * e.sin().max(0.0) * SUN_ILLUMINANCE;
            println!(
                "  sun {elev:>6.1}°: diffuse {:>10.3} lx  direct {:>9.1} lx  total {:>10.3} lx  sky b/g {:.2}",
                diffuse[1] * SUN_ILLUMINANCE,
                direct_h,
                diffuse[1] * SUN_ILLUMINANCE + direct_h,
                diffuse[2] / diffuse[1].max(1e-12),
            );
        }
    }
}
