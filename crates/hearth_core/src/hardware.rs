//! The machine the game runs on (E4.1 §6): its cores, memory and graphics adapter, and the video
//! settings that suit it, set on the first run as a safe preset the player can raise. The world's
//! workers and the caches scale from the machine themselves (`jobs`, `memory`).

use crate::options::{GraphicsPreset, VideoOptions};

/// What kind of graphics adapter draws the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gpu {
    /// A card of its own, with memory of its own.
    Discrete,
    /// Part of the processor, sharing the machine's memory.
    Integrated,
    /// The processor drawing (a software rasterizer).
    Software,
    Unknown,
}

/// The machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    pub cores: usize,
    /// Its memory (bytes), where the system tells.
    pub ram: Option<u64>,
    pub gpu: Gpu,
    /// The adapter's own memory (bytes), where the system tells.
    pub vram: Option<u64>,
}

/// The video settings that suit a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suited {
    pub preset: GraphicsPreset,
    pub render_distance: u32,
    pub vertical_render_distance: u32,
    pub lod_distance: u32,
    pub lod_vram_budget_mb: u32,
}

const GIB: u64 = 1 << 30;

impl Machine {
    /// This machine's cores and memory, with the adapter the renderer found.
    pub fn this(gpu: Gpu, vram: Option<u64>) -> Self {
        let (cores, ram) = crate::prof::machine();
        Self {
            cores,
            ram,
            gpu,
            vram,
        }
    }

    /// The settings that suit it, on the safe side where it is not known: the low preset on a
    /// software adapter, few cores or little memory; the high one on a card with 6 GB or more
    /// beside six cores and 16 GB; the medium one otherwise. A quarter of a card's own memory
    /// goes to the distant terrain.
    pub fn suited(&self) -> Suited {
        let ram = self.ram.unwrap_or(8 * GIB);
        let medium = Suited {
            preset: GraphicsPreset::Medium,
            render_distance: 12,
            vertical_render_distance: 8,
            lod_distance: 256,
            lod_vram_budget_mb: 768,
        };
        if self.gpu == Gpu::Software || self.cores < 4 || ram < 8 * GIB {
            return Suited {
                preset: GraphicsPreset::Low,
                render_distance: 8,
                vertical_render_distance: 6,
                lod_distance: 128,
                lod_vram_budget_mb: 256,
            };
        }
        match self.gpu {
            Gpu::Discrete => {
                let strong =
                    self.vram.is_some_and(|v| v >= 6 * GIB) && self.cores >= 6 && ram >= 16 * GIB;
                Suited {
                    preset: if strong {
                        GraphicsPreset::High
                    } else {
                        GraphicsPreset::Medium
                    },
                    lod_distance: if strong { 384 } else { 256 },
                    lod_vram_budget_mb: self
                        .vram
                        .map_or(1024, |v| ((v / 4) >> 20).clamp(512, 2048) as u32),
                    ..medium
                }
            }
            // Sharing the machine's memory: the distant terrain's share kept small.
            Gpu::Integrated => Suited {
                render_distance: 10,
                lod_vram_budget_mb: 512,
                ..medium
            },
            Gpu::Software | Gpu::Unknown => medium,
        }
    }
}

impl Suited {
    /// Sets these on the video options (the player's other choices left as they are).
    pub fn apply(&self, v: &mut VideoOptions) {
        v.apply_preset(self.preset);
        v.render_distance = self.render_distance;
        v.vertical_render_distance = self.vertical_render_distance;
        v.lod_distance = self.lod_distance;
        v.lod_vram_budget_mb = self.lod_vram_budget_mb;
    }
}

/// The memory of the graphics adapter of this name (bytes), where the system tells: Windows keeps
/// it with the display adapter's driver in the registry, Linux's amdgpu driver in sysfs.
pub fn video_memory(adapter: &str) -> Option<u64> {
    sys::video_memory(adapter)
}

/// Whether two names of an adapter name the same one (one may hold the other: a driver's
/// description and the adapter's name differ in what they add).
#[cfg(any(windows, test))]
fn same_adapter(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_lowercase(), b.trim().to_lowercase());
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

#[cfg(windows)]
mod sys {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_ANY, RegGetValueW};

    /// The display adapters' drivers.
    const CLASS: &str =
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A value under `HKEY_LOCAL_MACHINE`, as bytes.
    fn value(key: &str, name: &str) -> Option<Vec<u8>> {
        let (k, n) = (wide(key), wide(name));
        let mut len: u32 = 0;
        // SAFETY: nul-terminated wide strings; asked first for the value's size alone.
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                k.as_ptr(),
                n.as_ptr(),
                RRF_RT_ANY,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut len,
            )
        };
        if r != 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        // SAFETY: as above, into a buffer of the size the registry gave.
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                k.as_ptr(),
                n.as_ptr(),
                RRF_RT_ANY,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        (r == 0).then(|| {
            buf.truncate(len as usize);
            buf
        })
    }

    pub fn video_memory(adapter: &str) -> Option<u64> {
        for i in 0..16 {
            let key = format!("{CLASS}\\{i:04}");
            let Some(desc) = value(&key, "DriverDesc") else {
                continue;
            };
            let desc: Vec<u16> = desc
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|c| *c != 0)
                .collect();
            if !super::same_adapter(&String::from_utf16_lossy(&desc), adapter) {
                continue;
            }
            // The 64-bit size where the driver gives it (cards past 4 GB), else the 32-bit one.
            let size = value(&key, "HardwareInformation.qwMemorySize")
                .or_else(|| value(&key, "HardwareInformation.MemorySize"))?;
            let mut b = [0u8; 8];
            let n = size.len().min(8);
            b[..n].copy_from_slice(&size[..n]);
            return Some(u64::from_le_bytes(b)).filter(|m| *m > 0);
        }
        None
    }
}

#[cfg(target_os = "linux")]
mod sys {
    /// The largest of the cards' own memory that the drivers tell (amdgpu does; others do not).
    pub fn video_memory(_adapter: &str) -> Option<u64> {
        std::fs::read_dir("/sys/class/drm")
            .ok()?
            .flatten()
            .filter_map(|e| {
                std::fs::read_to_string(e.path().join("device/mem_info_vram_total"))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
            .max()
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod sys {
    pub fn video_memory(_adapter: &str) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(cores: usize, ram_gb: u64, gpu: Gpu, vram_gb: Option<u64>) -> Machine {
        Machine {
            cores,
            ram: Some(ram_gb * GIB),
            gpu,
            vram: vram_gb.map(|v| v * GIB),
        }
    }

    #[test]
    fn the_settings_follow_the_machine_on_the_safe_side() {
        // The cloud machine (four cores, a software adapter): low.
        let s = machine(4, 16, Gpu::Software, None).suited();
        assert_eq!((s.preset, s.render_distance), (GraphicsPreset::Low, 8));
        // A laptop with an 8 GB card, eight cores and 16 GB: high, a quarter of the card for
        // the distant terrain.
        let s = machine(8, 16, Gpu::Discrete, Some(8)).suited();
        assert_eq!(s.preset, GraphicsPreset::High);
        assert_eq!(s.lod_vram_budget_mb, 2048);
        // A card of unknown memory: medium, a gigabyte.
        let s = machine(8, 16, Gpu::Discrete, None).suited();
        assert_eq!(
            (s.preset, s.lod_vram_budget_mb),
            (GraphicsPreset::Medium, 1024)
        );
        // Integrated graphics: medium, less.
        let s = machine(8, 16, Gpu::Integrated, None).suited();
        assert_eq!((s.preset, s.render_distance), (GraphicsPreset::Medium, 10));
        // Little memory, even with a card: low.
        let s = machine(8, 6, Gpu::Discrete, Some(8)).suited();
        assert_eq!(s.preset, GraphicsPreset::Low);
        // Applied, the rest of the player's choices stay.
        let mut v = VideoOptions {
            fov: 90.0,
            ..VideoOptions::default()
        };
        machine(8, 16, Gpu::Discrete, Some(8))
            .suited()
            .apply(&mut v);
        assert_eq!(
            (v.graphics, v.render_distance, v.fov),
            (GraphicsPreset::High, 12, 90.0)
        );
        assert!(same_adapter(
            "NVIDIA GeForce RTX 4060 Laptop GPU",
            "nvidia geforce rtx 4060 laptop gpu"
        ));
        assert!(!same_adapter("", "llvmpipe"));
    }
}
