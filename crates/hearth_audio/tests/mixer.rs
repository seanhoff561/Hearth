//! The mixer rendered offline: every sound is heard and ends, volumes and pausing silence what
//! they should, the wind grows with its speed, a roof dulls the rain, water muffles the world,
//! the heart and the breath keep their rates, a cave echoes, and nothing passes full scale.

use hearth_audio::{Ambience, BUSES, Bus, Command, Mixer, Sound, Surface};

const RATE: f32 = 48_000.0;

fn render(m: &mut Mixer, seconds: f32) -> Vec<f32> {
    let mut out = vec![0.0; (seconds * RATE) as usize * 2];
    m.render(&mut out);
    out
}

fn peak(s: &[f32]) -> f32 {
    s.iter().fold(0.0f32, |a, &x| a.max(x.abs()))
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|&x| (x * x) as f64).sum::<f64>() / s.len().max(1) as f64).sqrt() as f32
}

/// The share of the signal's energy in its changes from sample to sample: high for bright
/// sounds, low for dull ones.
fn brightness(s: &[f32]) -> f32 {
    let left: Vec<f32> = s.iter().step_by(2).copied().collect();
    let diff: Vec<f32> = left.windows(2).map(|w| w[1] - w[0]).collect();
    rms(&diff) / rms(&left).max(1e-9)
}

fn play(m: &mut Mixer, sound: Sound, bus: Bus) {
    m.apply(Command::Play {
        sound,
        bus,
        gain: 1.0,
        pan: 0.0,
    });
}

fn every_sound() -> Vec<(String, Sound, Bus)> {
    let mut all = Vec::new();
    for s in Surface::ALL {
        all.push((
            format!("step {s:?}"),
            Sound::Step {
                surface: s,
                force: 0.55,
            },
            Bus::Players,
        ));
    }
    all.push((
        "land 8 m/s on soil".into(),
        Sound::Land {
            surface: Surface::Soil,
            impact: 8.0,
        },
        Bus::Players,
    ));
    all.push((
        "splash 6 m/s".into(),
        Sound::Splash { speed: 6.0 },
        Bus::Players,
    ));
    all.push((
        "stroke".into(),
        Sound::Stroke { under: false },
        Bus::Players,
    ));
    all.push((
        "stroke under".into(),
        Sound::Stroke { under: true },
        Bus::Players,
    ));
    all.push(("gasp".into(), Sound::Gasp { force: 1.0 }, Bus::Players));
    all.push((
        "hurt, fracture".into(),
        Sound::Hurt {
            force: 1.0,
            fracture: true,
        },
        Bus::Players,
    ));
    all.push((
        "stomach".into(),
        Sound::Stomach { force: 1.0 },
        Bus::Players,
    ));
    all.push(("swallow".into(), Sound::Swallow, Bus::Players));
    all.push(("click".into(), Sound::Click, Bus::Ui));
    all
}

#[test]
fn silent_until_told() {
    let mut m = Mixer::new(RATE, 1);
    assert_eq!(peak(&render(&mut m, 0.5)), 0.0);
}

#[test]
fn every_sound_is_heard_and_ends() {
    for (name, sound, bus) in every_sound() {
        let mut m = Mixer::new(RATE, 2);
        play(&mut m, sound, bus);
        let out = render(&mut m, 2.5);
        assert!(out.iter().all(|x| x.is_finite()), "{name}: not finite");
        let p = peak(&out);
        eprintln!(
            "{name:24} peak {p:.3} ({:+.0} dBFS)",
            20.0 * p.max(1e-9).log10()
        );
        assert!((0.01..0.95).contains(&p), "{name}: peak {p}");
        assert_eq!(m.voices(), 0, "{name}: still playing after 2.5 s");
        let tail = &out[(2.0 * RATE) as usize * 2..];
        assert!(peak(tail) < 1e-3, "{name}: tail {}", peak(tail));
    }
}

#[test]
fn steps_differ_from_one_ground_to_another() {
    // Ice and stone ring bright; moss and mud are dull.
    let bright = |surface| {
        let mut m = Mixer::new(RATE, 3);
        play(
            &mut m,
            Sound::Step {
                surface,
                force: 0.8,
            },
            Bus::Players,
        );
        brightness(&render(&mut m, 0.3))
    };
    let (ice, stone, moss, mud) = (
        bright(Surface::Ice),
        bright(Surface::Stone),
        bright(Surface::Moss),
        bright(Surface::Mud),
    );
    eprintln!("brightness: ice {ice:.3}, stone {stone:.3}, moss {moss:.3}, mud {mud:.3}");
    assert!(ice > moss * 2.0 && stone > mud * 2.0);
}

#[test]
fn volumes_and_pausing_silence_their_sounds() {
    let mut buses = [1.0; BUSES];
    buses[Bus::Players.index()] = 0.0;
    let mut m = Mixer::new(RATE, 4);
    m.apply(Command::Volumes { master: 1.0, buses });
    // Volumes ease over 30 ms.
    render(&mut m, 0.5);
    play(&mut m, Sound::Splash { speed: 6.0 }, Bus::Players);
    assert!(peak(&render(&mut m, 1.0)) < 1e-4, "players muted");
    play(&mut m, Sound::Click, Bus::Ui);
    assert!(
        peak(&render(&mut m, 0.3)) > 0.01,
        "the interface still sounds"
    );

    let mut m = Mixer::new(RATE, 4);
    m.apply(Command::Volumes {
        master: 0.0,
        buses: [1.0; BUSES],
    });
    render(&mut m, 0.5);
    play(&mut m, Sound::Click, Bus::Ui);
    assert!(peak(&render(&mut m, 0.3)) < 1e-4, "master muted");

    // Paused: the world's sounds fade and new ones wait; the interface still clicks.
    let mut m = Mixer::new(RATE, 4);
    m.apply(Command::Ambience(Ambience {
        wind_m_s: 15.0,
        paused: true,
        ..Ambience::default()
    }));
    render(&mut m, 2.0);
    play(
        &mut m,
        Sound::Step {
            surface: Surface::Stone,
            force: 1.0,
        },
        Bus::Players,
    );
    assert!(peak(&render(&mut m, 0.5)) < 1e-3, "paused world");
    play(&mut m, Sound::Click, Bus::Ui);
    assert!(peak(&render(&mut m, 0.3)) > 0.01, "a click while paused");
}

fn ambience_rms(a: Ambience, seed: u32) -> (f32, Vec<f32>) {
    let mut m = Mixer::new(RATE, seed);
    m.apply(Command::Ambience(a));
    render(&mut m, 6.0);
    let out = render(&mut m, 4.0);
    (rms(&out), out)
}

#[test]
fn the_wind_grows_with_its_speed() {
    let at = |v: f32| {
        ambience_rms(
            Ambience {
                wind_m_s: v,
                ..Ambience::default()
            },
            5,
        )
        .0
    };
    let (calm, breeze, gale, storm) = (at(1.0), at(5.0), at(15.0), at(28.0));
    eprintln!("wind rms: 1 m/s {calm:.4}, 5 {breeze:.4}, 15 {gale:.4}, 28 {storm:.4}");
    assert!(calm < 0.005);
    assert!(calm < breeze && breeze < gale && gale < storm);
    assert!(storm < 0.4);
    // Sheltered, a gale is quieter and duller.
    let (open, open_s) = ambience_rms(
        Ambience {
            wind_m_s: 15.0,
            ..Ambience::default()
        },
        6,
    );
    let (inside, inside_s) = ambience_rms(
        Ambience {
            wind_m_s: 15.0,
            sheltered: 1.0,
            ..Ambience::default()
        },
        6,
    );
    assert!(inside < open * 0.6, "{inside} vs {open}");
    assert!(brightness(&inside_s) < brightness(&open_s));
}

#[test]
fn a_roof_dulls_the_rain() {
    let rain = |sheltered: f32, mm: f32| {
        ambience_rms(
            Ambience {
                rain_mm_h: mm,
                sheltered,
                ..Ambience::default()
            },
            7,
        )
    };
    let (light, _) = rain(0.0, 1.0);
    let (heavy, open) = rain(0.0, 12.0);
    let (_, roofed) = rain(1.0, 12.0);
    eprintln!(
        "rain rms: 1 mm/h {light:.4}, 12 mm/h {heavy:.4}; brightness open {:.3}, roofed {:.3}",
        brightness(&open),
        brightness(&roofed)
    );
    assert!(light > 0.002 && heavy > light * 1.5 && heavy < 0.3);
    assert!(brightness(&roofed) < brightness(&open) * 0.7);
    // Deep in a cave a storm outside is a murmur.
    let (storm, _) = ambience_rms(
        Ambience {
            wind_m_s: 22.0,
            rain_mm_h: 15.0,
            ..Ambience::default()
        },
        7,
    );
    let (deep, _) = ambience_rms(
        Ambience {
            wind_m_s: 22.0,
            rain_mm_h: 15.0,
            sheltered: 1.0,
            buried: 1.0,
            enclosed: 1.0,
            ..Ambience::default()
        },
        7,
    );
    eprintln!("storm rms {storm:.4}, deep in a cave {deep:.4}");
    assert!(deep < storm * 0.1, "{deep} vs {storm}");
}

#[test]
fn water_muffles_the_world_and_hides_the_weather() {
    let step_under = |underwater: bool| {
        let mut m = Mixer::new(RATE, 8);
        m.apply(Command::Ambience(Ambience {
            underwater,
            ..Ambience::default()
        }));
        render(&mut m, 1.0);
        play(&mut m, Sound::Splash { speed: 5.0 }, Bus::Players);
        let out = render(&mut m, 0.6);
        (brightness(&out), rms(&out))
    };
    let (dry, _) = step_under(false);
    let (wet, wet_rms) = step_under(true);
    assert!(wet_rms > 0.0005, "still heard");
    assert!(wet < dry * 0.5, "{wet} vs {dry}");
    let (storm_under, _) = ambience_rms(
        Ambience {
            wind_m_s: 20.0,
            rain_mm_h: 20.0,
            underwater: true,
            ..Ambience::default()
        },
        9,
    );
    // What is left is the water's own low hush.
    assert!(storm_under < 0.06, "{storm_under}");
}

/// Times the signal's envelope rises through half its peak.
fn onsets(s: &[f32], window_s: f32) -> usize {
    let left: Vec<f32> = s.iter().step_by(2).map(|x| x.abs()).collect();
    let w = (window_s * RATE) as usize;
    let env: Vec<f32> = left
        .chunks(w)
        .map(|c| c.iter().copied().fold(0.0, f32::max))
        .collect();
    let top = env.iter().copied().fold(0.0, f32::max);
    let mut count = 0;
    let mut above = false;
    for &e in &env {
        if !above && e > top * 0.5 {
            count += 1;
            above = true;
        } else if above && e < top * 0.2 {
            above = false;
        }
    }
    count
}

#[test]
fn the_heart_and_the_breath_keep_their_rates() {
    let (_, heart) = ambience_rms(
        Ambience {
            heart_bpm: 120.0,
            heart: 1.0,
            ..Ambience::default()
        },
        10,
    );
    // 4 s at 120 a minute: 8 beats of two sounds each, the second softer.
    let beats = onsets(&heart, 0.02);
    assert!((7..=17).contains(&beats), "{beats} onsets");
    let quiet_heart = rms(&heart);
    let (_, breath) = ambience_rms(
        Ambience {
            breaths_per_min: 30.0,
            breath: 1.0,
            ..Ambience::default()
        },
        11,
    );
    // 4 s at 30 a minute: 2 breaths, in and out.
    let breaths = onsets(&breath, 0.05);
    assert!((2..=5).contains(&breaths), "{breaths} onsets");
    eprintln!("heart rms {quiet_heart:.4}, breath rms {:.4}", rms(&breath));
    let (silent, _) = ambience_rms(
        Ambience {
            heart_bpm: 120.0,
            breaths_per_min: 30.0,
            ..Ambience::default()
        },
        12,
    );
    assert_eq!(silent, 0.0);
}

#[test]
fn a_cave_echoes() {
    let tail = |enclosed: f32| {
        let mut m = Mixer::new(RATE, 13);
        m.apply(Command::Ambience(Ambience {
            enclosed,
            ..Ambience::default()
        }));
        render(&mut m, 2.0);
        play(
            &mut m,
            Sound::Step {
                surface: Surface::Stone,
                force: 1.0,
            },
            Bus::Players,
        );
        let out = render(&mut m, 1.0);
        rms(&out[(0.25 * RATE) as usize * 2..(0.8 * RATE) as usize * 2])
    };
    let (open, cave) = (tail(0.0), tail(1.0));
    eprintln!("tail rms: open {open:.6}, cave {cave:.6}");
    assert!(cave > 0.002 && cave > open * 20.0);
}

#[test]
fn a_crowd_of_sounds_stays_under_full_scale() {
    let mut m = Mixer::new(RATE, 14);
    m.apply(Command::Ambience(Ambience {
        wind_m_s: 40.0,
        rain_mm_h: 100.0,
        heart_bpm: 180.0,
        heart: 1.0,
        breaths_per_min: 50.0,
        breath: 1.0,
        enclosed: 1.0,
        ..Ambience::default()
    }));
    for i in 0..200 {
        m.apply(Command::Play {
            sound: Sound::Land {
                surface: Surface::Stone,
                impact: 20.0,
            },
            bus: Bus::Players,
            gain: 1.0,
            pan: (i as f32 / 50.0).sin(),
        });
    }
    let out = render(&mut m, 2.0);
    assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    assert!(m.voices() <= 48);
}

#[test]
fn the_same_seed_makes_the_same_sound() {
    let run = || {
        let mut m = Mixer::new(RATE, 15);
        m.apply(Command::Ambience(Ambience {
            wind_m_s: 12.0,
            rain_mm_h: 4.0,
            ..Ambience::default()
        }));
        play(&mut m, Sound::Splash { speed: 3.0 }, Bus::Players);
        render(&mut m, 0.5)
    };
    assert_eq!(run(), run());
}

fn write_wav(path: &std::path::Path, samples: &[f32]) {
    let rate = RATE as u32;
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b).expect("wav");
}

/// Every sound, and a walk through changing weather, to listen to (`bench-out/sounds/*.wav`).
#[test]
fn sounds_to_listen_to() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out/sounds");
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, sound, bus) in every_sound() {
        let mut m = Mixer::new(RATE, 16);
        play(&mut m, sound, bus);
        let file: String = name
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join("_")
            .to_lowercase();
        write_wav(&dir.join(format!("{file}.wav")), &render(&mut m, 1.5));
    }
    // A walk: grass in a breeze and light rain; a jog over gravel as the wind rises; a sprint
    // on stone in a storm, heart and breath labouring; into a cave out of the rain; a jump
    // into cold water.
    let mut m = Mixer::new(RATE, 17);
    let mut out = Vec::new();
    let scenes: [(f32, Ambience, Option<(Surface, f32, f32)>); 5] = [
        (
            6.0,
            Ambience {
                wind_m_s: 5.0,
                rain_mm_h: 1.0,
                heart_bpm: 75.0,
                breaths_per_min: 14.0,
                ..Ambience::default()
            },
            Some((Surface::Grass, 0.55, 0.54)),
        ),
        (
            6.0,
            Ambience {
                wind_m_s: 12.0,
                rain_mm_h: 3.0,
                heart_bpm: 120.0,
                breaths_per_min: 24.0,
                breath: 0.3,
                ..Ambience::default()
            },
            Some((Surface::Gravel, 0.8, 0.38)),
        ),
        (
            6.0,
            Ambience {
                wind_m_s: 22.0,
                rain_mm_h: 15.0,
                heart_bpm: 165.0,
                heart: 0.65,
                breaths_per_min: 38.0,
                breath: 0.9,
                ..Ambience::default()
            },
            Some((Surface::Stone, 1.0, 0.3)),
        ),
        (
            6.0,
            Ambience {
                wind_m_s: 22.0,
                rain_mm_h: 15.0,
                sheltered: 1.0,
                buried: 1.0,
                enclosed: 1.0,
                heart_bpm: 110.0,
                heart: 0.2,
                breaths_per_min: 24.0,
                breath: 0.4,
                ..Ambience::default()
            },
            Some((Surface::Stone, 0.55, 0.54)),
        ),
        (
            6.0,
            Ambience {
                wind_m_s: 8.0,
                underwater: true,
                heart_bpm: 130.0,
                heart: 0.5,
                ..Ambience::default()
            },
            None,
        ),
    ];
    for (k, (seconds, ambience, steps)) in scenes.into_iter().enumerate() {
        m.apply(Command::Ambience(ambience));
        if k == 4 {
            play(&mut m, Sound::Splash { speed: 7.0 }, Bus::Players);
        }
        let mut t = 0.0;
        let mut foot = false;
        while t < seconds {
            let dt = match steps {
                Some((surface, force, every)) => {
                    foot = !foot;
                    m.apply(Command::Play {
                        sound: Sound::Step { surface, force },
                        bus: Bus::Players,
                        gain: 1.0,
                        pan: if foot { -0.12 } else { 0.12 },
                    });
                    every
                }
                None => {
                    play(&mut m, Sound::Stroke { under: true }, Bus::Players);
                    1.5
                }
            };
            out.extend(render(&mut m, dt));
            t += dt;
        }
    }
    assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    write_wav(&dir.join("walk.wav"), &out);
}
