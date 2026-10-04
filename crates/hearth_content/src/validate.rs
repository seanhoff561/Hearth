//! Unit sanity: every numeric field must be in a physically plausible range for its unit.

use crate::content::{Content, Origin, Table};
use crate::diag::Report;
use crate::schema::Range;
use crate::schema::process::Output;

struct Check<'a> {
    report: &'a mut Report,
    file: std::path::PathBuf,
    line: Option<usize>,
    id: String,
}

impl Check<'_> {
    /// A field's values out of the order they must keep.
    fn order(&mut self, field: &str, what: &str) {
        self.report.error(
            "unit-order",
            Some(self.file.clone()),
            self.line,
            format!("`{}`: {field} must {what}", self.id),
        );
    }

    fn range(&mut self, field: &str, v: f32, lo: f32, hi: f32) {
        if !v.is_finite() || v < lo || v > hi {
            self.report.error(
                "unit-range",
                Some(self.file.clone()),
                self.line,
                format!(
                    "`{}`: {field} = {v} is outside the plausible range {lo}..={hi}",
                    self.id
                ),
            );
        }
    }

    fn opt(&mut self, field: &str, v: Option<f32>, lo: f32, hi: f32) {
        if let Some(v) = v {
            self.range(field, v, lo, hi);
        }
    }

    fn pair(&mut self, field: &str, r: Range, lo: f32, hi: f32) {
        self.range(field, r.0, lo, hi);
        self.range(field, r.1, lo, hi);
        if r.0 > r.1 {
            self.report.error(
                "unit-range",
                Some(self.file.clone()),
                self.line,
                format!(
                    "`{}`: {field} range ({}, {}) has min above max",
                    self.id, r.0, r.1
                ),
            );
        }
    }

    fn opt_pair(&mut self, field: &str, r: Option<Range>, lo: f32, hi: f32) {
        if let Some(r) = r {
            self.pair(field, r, lo, hi);
        }
    }
}

fn each<T>(
    table: &Table<T>,
    report: &mut Report,
    id: impl Fn(&T) -> &str,
    mut f: impl FnMut(&T, &mut Check<'_>),
) {
    for (e, Origin { file, line, .. }) in table.iter_with_origin() {
        let mut c = Check {
            report,
            file: file.clone(),
            line: *line,
            id: id(e).to_owned(),
        };
        f(e, &mut c);
    }
}

fn outputs(c: &mut Check<'_>, outs: &[Output]) {
    for o in outs {
        c.pair("output amount", o.amount, 0.0, 1.0e6);
        c.range("output chance", o.chance, 0.0, 1.0);
        c.range("quality base", o.quality.base, 0.0, 1.0);
    }
}

/// Checks every numeric field against plausible physical ranges.
pub fn validate(content: &Content, report: &mut Report) {
    each(
        &content.materials,
        report,
        |e| &e.id,
        |m, c| {
            c.range("density_kg_m3", m.density_kg_m3, 1.0, 25_000.0);
            c.opt("hardness_mohs", m.hardness_mohs, 0.0, 10.0);
            c.opt("knapping", m.knapping, 0.0, 1.0);
            c.opt("compressive_mpa", m.strength.compressive_mpa, 0.0, 6_000.0);
            c.opt("tensile_mpa", m.strength.tensile_mpa, 0.0, 6_000.0);
            c.opt("bending_mpa", m.strength.bending_mpa, 0.0, 6_000.0);
            c.opt("elastic_modulus_gpa", m.elastic_modulus_gpa, 0.0, 1_500.0);
            c.opt("conductivity_w_mk", m.thermal.conductivity_w_mk, 0.0, 500.0);
            c.opt(
                "specific_heat_j_kgk",
                m.thermal.specific_heat_j_kgk,
                100.0,
                5_000.0,
            );
            c.opt("melting_c", m.thermal.melting_c, -100.0, 4_000.0);
            c.opt("ignition_c", m.thermal.ignition_c, 100.0, 1_200.0);
            c.opt_pair("firing_c", m.thermal.firing_c, 100.0, 2_000.0);
            c.opt("fuel_mj_kg", m.fuel_mj_kg, 0.0, 60.0);
            c.opt("workability", m.workability, 0.0, 1.0);
            c.opt("durability", m.durability, 0.0, 1.0);
            c.opt("porosity", m.porosity, 0.0, 1.0);
            c.opt("kcal_per_kg", m.kcal_per_kg, 0.0, 9_100.0);
            c.opt("roughness", m.appearance.roughness, 0.0, 1.0);
        },
    );
    each(
        &content.provinces,
        report,
        |e| &e.id,
        |p, c| {
            for l in &p.sequence {
                c.pair("thickness_m", l.thickness_m, 0.0, 20_000.0);
                c.range("presence", l.presence, 0.0, 1.0);
            }
            c.pair("dip_deg", p.dip_deg, 0.0, 90.0);
            c.range("weight", p.weight, 0.0, 100.0);
            for word in &p.conditions {
                if !crate::schema::geology::PROVINCE_CONDITIONS.contains(&word.as_str()) {
                    c.report.error(
                        "unknown-condition",
                        Some(c.file.clone()),
                        c.line,
                        format!(
                            "`{}`: condition {word:?} is not one the generator understands ({})",
                            c.id,
                            crate::schema::geology::PROVINCE_CONDITIONS.join(", ")
                        ),
                    );
                }
            }
        },
    );
    each(
        &content.soils,
        report,
        |e| &e.id,
        |soil, c| {
            for h in &soil.horizons {
                c.pair("thickness_m", h.thickness_m, 0.0, 50.0);
            }
            c.range("drainage", soil.drainage, 0.0, 1.0);
            c.range("ph", soil.ph, 2.0, 11.0);
            c.range("workability", soil.workability, 0.0, 1.0);
            let f = &soil.formation;
            c.opt_pair("formation drainage", f.drainage, 0.0, 1.0);
            c.opt("max_slope_deg", f.max_slope_deg, 0.0, 90.0);
            for word in &f.vegetation {
                if !crate::schema::geology::SOIL_VEGETATION.contains(&word.as_str()) {
                    c.report.error(
                        "unknown-condition",
                        Some(c.file.clone()),
                        c.line,
                        format!(
                            "`{}`: vegetation {word:?} is not one the generator understands",
                            c.id
                        ),
                    );
                }
            }
            for code in &f.climates {
                let ok = code.len() <= 3
                    && code.starts_with(['A', 'B', 'C', 'D', 'E'])
                    && code.chars().all(|ch| ch.is_ascii_alphabetic());
                if !ok {
                    c.report.error(
                        "unknown-condition",
                        Some(c.file.clone()),
                        c.line,
                        format!("`{}`: climate {code:?} is not a Köppen code or group", c.id),
                    );
                }
            }
        },
    );
    // Every tectonic setting needs a province, or its regions borrow another's rocks.
    for setting in crate::schema::geology::TectonicSetting::ALL {
        if !content.provinces.iter().any(|p| p.setting == setting) {
            report.warning(
                "province-coverage",
                None,
                None,
                format!("no geological province for the {setting:?} setting"),
            );
        }
    }
    each(
        &content.deposits,
        report,
        |e| &e.id,
        |d, c| {
            c.pair("depth_m", d.depth_m, 0.0, 20_000.0);
            c.pair("size_m", d.size_m, 0.0, 100_000.0);
            c.opt_pair("thickness_m", d.thickness_m, 0.0, 1_000.0);
            c.opt_pair("extent_m", d.extent_m, 0.0, 100_000.0);
            for word in &d.conditions {
                if !crate::schema::geology::DEPOSIT_CONDITIONS.contains(&word.as_str()) {
                    c.report.error(
                        "unknown-condition",
                        Some(c.file.clone()),
                        c.line,
                        format!(
                            "`{}`: condition {word:?} is not one the generator understands ({})",
                            c.id,
                            crate::schema::geology::DEPOSIT_CONDITIONS.join(", ")
                        ),
                    );
                }
            }
            c.pair("grade", d.grade, 0.0, 1.0);
            c.range("frequency_per_km2", d.frequency_per_km2, 0.0, 10_000.0);
            c.range("era", d.era as f32, 0.0, 8.0);
        },
    );
    each(
        &content.soils,
        report,
        |e| &e.id,
        |s, c| {
            for h in &s.horizons {
                c.pair("horizon thickness_m", h.thickness_m, 0.0, 50.0);
            }
            for (n, v) in [
                ("n", s.fertility.n),
                ("p", s.fertility.p),
                ("k", s.fertility.k),
                ("organic", s.fertility.organic),
            ] {
                c.range(&format!("fertility.{n}"), v, 0.0, 1.0);
            }
            c.range("drainage", s.drainage, 0.0, 1.0);
            c.range("ph", s.ph, 2.5, 11.0);
            c.range("workability", s.workability, 0.0, 1.0);
        },
    );
    each(
        &content.plants,
        report,
        |e| &e.id,
        |p, c| {
            c.range("max_height_m", p.max_height_m, 0.0, 130.0);
            c.opt("max_trunk_diameter_m", p.max_trunk_diameter_m, 0.0, 12.0);
            c.opt("lifespan_years", p.lifespan_years, 0.0, 6_000.0);
            c.opt("growth_m_per_year", p.growth_m_per_year, 0.0, 30.0);
            c.range("shade_tolerance", p.shade_tolerance, 0.0, 1.0);
            c.range("flammability", p.flammability, 0.0, 1.0);
            c.opt_pair("climate.temp_c", p.climate.temp_c, -60.0, 45.0);
            c.opt_pair("climate.precip_mm", p.climate.precip_mm, 0.0, 12_000.0);
            c.opt("phenology.leaf_out", p.phenology.leaf_out, 0.0, 1.0);
            c.opt_pair("phenology.flowering", p.phenology.flowering, 0.0, 1.0);
            c.opt_pair("phenology.fruiting", p.phenology.fruiting, 0.0, 1.0);
            c.opt_pair("phenology.leaf_fall", p.phenology.leaf_fall, 0.0, 1.0);
            for part in &p.parts {
                c.opt_pair("part yield_kg", part.yield_kg, 0.0, 10_000.0);
            }
        },
    );
    each(
        &content.animals,
        report,
        |e| &e.id,
        |a, c| {
            c.pair("mass_kg", a.mass_kg, 0.0, 200_000.0);
            c.opt("dimorphism", a.dimorphism, 0.3, 3.0);
            c.range("length_m", a.length_m, 0.0, 35.0);
            c.opt("shoulder_height_m", a.shoulder_height_m, 0.0, 6.0);
            c.range("speed.walk_m_s", a.speed.walk_m_s, 0.0, 10.0);
            c.opt("speed.run_m_s", a.speed.run_m_s, 0.0, 35.0);
            c.opt("speed.swim_m_s", a.speed.swim_m_s, 0.0, 35.0);
            c.opt("speed.fly_m_s", a.speed.fly_m_s, 0.0, 100.0);
            c.range("senses.vision_m", a.senses.vision_m, 0.0, 10_000.0);
            c.range("senses.hearing_m", a.senses.hearing_m, 0.0, 20_000.0);
            c.range("senses.smell_m", a.senses.smell_m, 0.0, 30_000.0);
            c.range("senses.night_vision", a.senses.night_vision, 0.0, 1.0);
            c.range("danger.aggression", a.danger.aggression, 0.0, 1.0);
            c.opt("growth_rate", a.growth_rate, -1.0, 10.0);
            c.opt("density_per_km2", a.density_per_km2, 0.0, 10_000_000.0);
            c.opt("diet.daily_food_kg", a.diet.daily_food_kg, 0.0, 500.0);
            for f in &a.diet.foods {
                c.range("diet preference", f.preference, 0.0, 1.0);
            }
            if let Some(d) = &a.domestication {
                c.range("domestication.difficulty", d.difficulty, 0.0, 1.0);
                c.range("domestication.era", d.era as f32, 0.0, 8.0);
            }
            if let Some(l) = &a.life {
                c.range("life.maturity_years", l.maturity_years, 0.05, 40.0);
                c.range("life.lifespan_years", l.lifespan_years, 0.2, 250.0);
                c.pair("life.litter", l.litter, 1.0, 1_000_000.0);
                c.range("life.births_per_year", l.births_per_year, 0.05, 12.0);
                c.range("life.birth_day", l.birth_day as f32, 0.0, 365.0);
                c.range("life.adult_survival", l.adult_survival, 0.0, 1.0);
                c.range("life.young_survival", l.young_survival, 0.0, 1.0);
                c.range("life.birth_mass_kg", l.birth_mass_kg, 0.0, 10_000.0);
            }
            if let Some(r) = &a.ranging {
                c.range("ranging.home_range_km2", r.home_range_km2, 0.0, 100_000.0);
                c.range("ranging.dispersal_km", r.dispersal_km, 0.0, 10_000.0);
                c.range("ranging.cover", r.cover, 0.0, 1.0);
            }
            if let Some(y) = &a.yields {
                for (what, v) in [
                    ("meat", y.meat),
                    ("fat", y.fat),
                    ("organs", y.organs),
                    ("bone", y.bone),
                    ("hide", y.hide),
                    ("sinew", y.sinew),
                ] {
                    c.range(&format!("yields.{what}"), v, 0.0, 1.0);
                }
                c.range(
                    "yields (their sum)",
                    y.meat + y.fat + y.organs + y.bone + y.hide + y.sinew,
                    0.0,
                    1.0,
                );
            }
            if let Some(t) = &a.track {
                c.range("track.length_cm", t.length_cm, 0.0, 100.0);
                c.range("track.stride_m", t.stride_m, 0.0, 10.0);
            }
            for call in &a.calls {
                c.range("call loudness_db", call.loudness_db, 0.0, 200.0);
                c.pair("call pitch_hz", call.pitch_hz, 1.0, 100_000.0);
                c.range("call seconds", call.seconds, 0.0, 60.0);
            }
            if let Some(t) = &a.temperament {
                c.range("temperament.flight_m", t.flight_m, 0.0, 2_000.0);
                c.range("temperament.boldness", t.boldness, 0.0, 1.0);
            }
            if let Some(h) = &a.habits {
                c.opt("habits.vigilance", h.vigilance, 0.0, 10.0);
                c.opt("habits.grooming", h.grooming, 0.0, 10.0);
                c.opt("habits.roaming", h.roaming, 0.0, 10.0);
                c.opt("habits.sociability", h.sociability, 0.0, 10.0);
            }
            if let Some(s) = &a.shape {
                c.opt("shape.neck", s.neck, 0.0, 1.0);
                c.opt("shape.head", s.head, 0.0, 0.6);
                // An elephant's snout is its trunk, longer than its head.
                let snout_max = if a.body_plan == crate::schema::fauna::BodyPlan::Elephant {
                    3.0
                } else {
                    1.0
                };
                c.opt("shape.snout", s.snout, 0.0, snout_max);
                c.opt("shape.tail", s.tail, 0.0, 2.0);
                c.opt("shape.tail_width", s.tail_width, 0.0, 2.0);
                c.opt("shape.ears", s.ears, 0.0, 2.0);
                c.opt("shape.legs", s.legs, 0.3, 3.0);
                c.opt("shape.hump", s.hump, 0.0, 0.5);
                match s.head_gear {
                    Some(crate::schema::fauna::HeadGear::Antlers {
                        length_m, tines, ..
                    }) => {
                        c.range("shape antlers length_m", length_m, 0.0, 2.5);
                        c.range("shape antlers tines", tines as f32, 0.0, 20.0);
                    }
                    Some(crate::schema::fauna::HeadGear::Horns {
                        length_m, curve, ..
                    }) => {
                        c.range("shape horns length_m", length_m, 0.0, 2.5);
                        c.range("shape horns curve", curve, 0.0, 1.0);
                    }
                    Some(crate::schema::fauna::HeadGear::NasalHorns { length_m }) => {
                        c.range("shape nasal horns length_m", length_m, 0.0, 1.5);
                    }
                    Some(crate::schema::fauna::HeadGear::Tusks { length_m, .. }) => {
                        c.range("shape tusks length_m", length_m, 0.0, 3.0);
                    }
                    None => {}
                }
            }
        },
    );
    each(
        &content.forms,
        report,
        |e| &e.id,
        |f, c| {
            for (axis, v) in f.size_m.iter().enumerate() {
                c.range(&format!("size_m[{axis}]"), *v, 0.0005, 30.0);
            }
            c.range("fill", f.fill, 0.001, 1.0);
            c.range("footprint width", f.footprint.0 as f32, 1.0, 16.0);
            c.range("footprint height", f.footprint.1 as f32, 1.0, 16.0);
        },
    );
    each(
        &content.explicit_items,
        report,
        |e| &e.id,
        |i, c| {
            c.range("mass_kg", i.mass_kg, 0.0001, 50_000.0);
            c.range("volume_l", i.volume_l, 0.0001, 100_000.0);
        },
    );
    each(
        &content.processes,
        report,
        |e| &e.id,
        |p, c| {
            c.range("duration.hours", p.duration.hours, 0.0001, 100_000.0);
            for i in &p.inputs {
                c.range("input amount", i.amount, 0.0, 1.0e6);
            }
            outputs(c, &p.outputs);
            outputs(c, &p.byproducts);
            for f in &p.failures {
                c.range("failure chance", f.chance, 0.0, 1.0);
                c.range("failure min_chance", f.min_chance, 0.0, f.chance.max(0.0));
            }
        },
    );
    each(
        &content.knowledge,
        report,
        |e| &e.id,
        |k, c| {
            c.range("era", k.era as f32, 0.0, 8.0);
            for d in &k.discovery {
                c.range("insight", d.insight, 0.001, 1.0);
            }
            // Years before 1950; later inventions are negative.
            if !(k.history.years_bp >= -100.0 && k.history.years_bp < 1.0e7) {
                c.range("history.years_bp", k.history.years_bp as f32, -100.0, 1.0e7);
            }
        },
    );
    each(
        &content.workstations,
        report,
        |e| &e.id,
        |w, c| {
            c.range("build.hours", w.build.hours, 0.0, 100_000.0);
            for cap in &w.provides {
                if let crate::schema::station::Capability::MaxTempC(t) = cap {
                    c.range("MaxTempC", *t, 0.0, 3_000.0);
                }
            }
        },
    );
    each(
        &content.construction,
        report,
        |e| &e.id,
        |p, c| {
            for (axis, v) in p.size_m.iter().enumerate() {
                c.range(&format!("size_m[{axis}]"), *v, 0.001, 30.0);
            }
            c.range("fill", p.fill, 0.001, 1.0);
            c.opt(
                "sheds_rain_min_pitch_deg",
                p.sheds_rain_min_pitch_deg,
                0.0,
                90.0,
            );
            c.opt("insulation_r", p.insulation_r, 0.0, 20.0);
        },
    );
    each(
        &content.injuries,
        report,
        |e| &e.id,
        |i, c| {
            c.pair("bleeding_ml_per_min", i.bleeding_ml_per_min, 0.0, 2_000.0);
            c.range("pain", i.pain, 0.0, 1.0);
            c.range("infection_risk", i.infection_risk, 0.0, 1.0);
            c.range("heal.hours", i.heal.hours, 0.0, 100_000.0);
        },
    );
    each(
        &content.illnesses,
        report,
        |e| &e.id,
        |i, c| {
            c.range("lethality", i.lethality, 0.0, 1.0);
            c.range("onset.hours", i.onset.hours, 0.0, 10_000.0);
            c.range("lasts.hours", i.lasts.hours, 0.0, 100_000.0);
        },
    );
    each(
        &content.garments,
        report,
        |e| &e.id,
        |g, c| {
            c.range("clo", g.clo, 0.0, 6.0);
            c.range("wind", g.wind, 0.0, 1.0);
            c.range("water", g.water, 0.0, 1.0);
            c.range("mass_kg", g.mass_kg, 0.0, 40.0);
            c.range("capacity_l", g.capacity_l, 0.0, 200.0);
            c.range("noise", g.noise, 0.0, 1.0);
        },
    );
    each(
        &content.eras,
        report,
        |e| &e.id,
        |e, c| {
            c.range("sea_level_offset_m", e.sea_level_offset_m, -200.0, 100.0);
            c.range("temperature_offset_c", e.temperature_offset_c, -20.0, 20.0);
        },
    );
    each(
        &content.life_tables,
        report,
        |e| &e.id,
        |t, c| {
            c.range("nursing_years", t.nursing_years, 0.0, 6.0);
            c.range("nursing_factor", t.nursing_factor, 0.0, 1.0);
            c.range("twins", t.twins, 0.0, 0.1);
            c.range("maternal_death", t.maternal_death, 0.0, 0.2);
            c.range("density", t.density, 0.001, 10.0);
            c.range("crowding children", t.crowding.children, 0.0, 10.0);
            c.range("crowding conception", t.crowding.conception, 0.0, 10.0);
            for (age, f) in &t.fecundability {
                c.range("fecundability age", *age, 0.0, 70.0);
                c.range("fecundability", *f, 0.0, 1.0);
            }
        },
    );
    each(
        &content.tendencies,
        report,
        |e| &e.id,
        |t, c| {
            c.range("middle", t.middle, t.range.0, t.range.1);
            c.range("spread", t.spread, 0.0, 0.5);
        },
    );
    each(
        &content.feelings,
        report,
        |e| &e.id,
        |f, c| {
            c.range("valence", f.valence, -1.0, 1.0);
            c.range("contagion", f.contagion, 0.0, 1.0);
        },
    );
    each(
        &content.values,
        report,
        |e| &e.id,
        |v, c| {
            c.range("default", v.default, 0.0, 1.0);
        },
    );
    each(
        &content.traits,
        report,
        |e| &e.id,
        |t, c| {
            c.range("heritability", t.heritability, 0.01, 0.999);
            c.range("polygenic", t.polygenic as f32, 0.0, 200.0);
        },
    );
    each(
        &content.loci,
        report,
        |e| &e.id,
        |l, c| {
            let sum: f32 = l.alleles.iter().map(|a| a.frequency).sum();
            c.range("alleles' frequencies' sum", sum, 0.99, 1.01);
            c.range("position_cm", l.position_cm, 0.0, 300.0);
        },
    );
    each(
        &content.chromosomes,
        report,
        |e| &e.id,
        |ch, c| {
            c.range("length_cm", ch.length_cm, 0.0, 400.0);
        },
    );
    each(
        &content.species,
        report,
        |e| &e.id,
        |h, c| {
            for (name, r) in [
                ("body.height_m.female", h.body.height_m.female),
                ("body.height_m.male", h.body.height_m.male),
            ] {
                c.pair(name, r, 0.5, 2.5);
            }
            for (name, r) in [
                ("body.mass_kg.female", h.body.mass_kg.female),
                ("body.mass_kg.male", h.body.mass_kg.male),
            ] {
                c.pair(name, r, 10.0, 200.0);
            }
            c.pair("social.group_size", h.social.group_size, 1.0, 1_000.0);
            c.range("life.gestation_days", h.life.gestation_days, 150.0, 330.0);
            c.range("life.weaning_years", h.life.weaning_years, 0.5, 8.0);
            c.range("life.maturity_years", h.life.maturity_years, 5.0, 25.0);
            // Stages in order, each later than the one before; growth rising to all of it.
            let mut last: Option<(crate::schema::humans::LifeStage, f32)> = None;
            for &(stage, from) in &h.life.stages {
                c.range("life.stages age", from, 0.0, 100.0);
                if let Some((s0, a0)) = last
                    && (stage <= s0 || from <= a0)
                {
                    c.order(
                        "life.stages",
                        "list each stage once, in order, each from a later age",
                    );
                }
                last = Some((stage, from));
            }
            let mut prev = (-1.0f32, 0.0f32, 0.0f32);
            for &(age, height, mass) in &h.life.growth {
                c.range("life.growth height share", height, 0.05, 1.0);
                c.range("life.growth mass share", mass, 0.01, 1.0);
                if age <= prev.0 || height < prev.1 || mass < prev.2 {
                    c.order("life.growth", "rise with age");
                }
                prev = (age, height, mass);
            }
            c.pair(
                "life.adult_death_years",
                h.life.adult_death_years,
                20.0,
                110.0,
            );
            c.range(
                "life.birth_interval_years",
                h.life.birth_interval_years,
                1.0,
                8.0,
            );
            c.range(
                "cognition.planning_depth",
                h.cognition.planning_depth as f32,
                0.0,
                12.0,
            );
        },
    );
    each(
        &content.balance_keys,
        report,
        |e| &e.id,
        |k, c| {
            c.range("default", k.default, k.min, k.max);
        },
    );
    // Singletons.
    let body = &content.body;
    let mut c = Check {
        report,
        file: "body/human.ron".into(),
        line: None,
        id: "body/human".into(),
    };
    c.pair("height_m", body.height_m, 1.2, 2.3);
    c.pair("mass_kg", body.mass_kg, 30.0, 200.0);
    c.range("bmr_kcal_per_kg_day", body.bmr_kcal_per_kg_day, 15.0, 35.0);
    c.range("stomach_capacity_l", body.stomach_capacity_l, 0.5, 5.0);
    c.range("water_l_per_day", body.water_l_per_day, 1.0, 10.0);
    c.range("fatal_water_loss", body.fatal_water_loss, 0.05, 0.3);
    c.range("core_temp_c", body.core_temp_c, 36.0, 38.0);
    c.range("walk_m_s", body.walk_m_s, 0.8, 2.0);
    c.range("jog_m_s", body.jog_m_s, 2.0, 4.5);
    c.range("sprint_m_s", body.sprint_m_s, 4.5, 11.0);
    c.range("swim_m_s", body.swim_m_s, 0.3, 2.5);
    c.range("comfortable_load", body.comfortable_load, 0.05, 0.6);
    c.range("max_load", body.max_load, body.comfortable_load, 1.5);
    for (activity, met) in &body.activity_met {
        c.range(&format!("activity_met.{activity}"), *met, 0.8, 20.0);
    }
    let t = &content.time;
    let mut c = Check {
        report,
        file: "time.ron".into(),
        line: None,
        id: "time".into(),
    };
    for (name, s) in [
        ("day_length_min", t.day_length_min),
        ("days_per_season", t.days_per_season),
    ] {
        c.range(
            &format!("{name}.default"),
            s.default as f32,
            s.min as f32,
            s.max as f32,
        );
    }
    c.range(
        "axial_tilt_deg.default",
        t.axial_tilt_deg.default as f32,
        t.axial_tilt_deg.min as f32,
        t.axial_tilt_deg.max as f32,
    );
    c.range("real_year_days", t.real_year_days as f32, 300.0, 400.0);
    c.range(
        "synodic_month_days",
        t.synodic_month_days as f32,
        20.0,
        40.0,
    );
}
