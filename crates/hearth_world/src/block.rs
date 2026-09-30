//! Data-driven blocks and their states.
//!
//! A block is defined by a [`BlockDef`] (usually loaded from a data pack). Every combination of
//! its property values is a *state* with a dense global [`BlockStateId`]. Everything the engine
//! needs per state in hot loops (flags, light, shapes, fluid amount) is precomputed into
//! struct-of-arrays tables in [`BlockRegistry`].

use std::collections::BTreeMap;
use std::fmt;

use hearth_core::{Registry, ResourceLocation};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::shape::{PropertyLookup, Shape, ShapeId, ShapeKind, shapes_for};

/// A tiny bitflags macro (avoids a dependency for one type).
macro_rules! bitflags_lite {
    ($(#[$m:meta])* pub struct $name:ident: $t:ty { $($(#[$fm:meta])* const $f:ident = $v:expr;)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub $t);
        impl $name {
            $($(#[$fm])* pub const $f: $name = $name($v);)*
            #[inline] pub const fn empty() -> Self { Self(0) }
            #[inline] pub const fn contains(self, o: Self) -> bool { self.0 & o.0 == o.0 }
            #[inline] pub const fn intersects(self, o: Self) -> bool { self.0 & o.0 != 0 }
            #[inline] pub fn insert(&mut self, o: Self) { self.0 |= o.0 }
            #[inline] pub fn set(&mut self, o: Self, on: bool) { if on { self.0 |= o.0 } else { self.0 &= !o.0 } }
            #[inline] pub const fn union(self, o: Self) -> Self { Self(self.0 | o.0) }
        }
        impl std::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, o: Self) -> Self { Self(self.0 | o.0) }
        }
    };
}

/// Dense id of a block type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BlockId(pub u16);

/// Dense id of a block state. State 0 is always `hearth:air`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
#[repr(transparent)]
pub struct BlockStateId(pub u16);

impl BlockStateId {
    pub const AIR: BlockStateId = BlockStateId(0);

    #[inline]
    pub fn is_air(self) -> bool {
        self.0 == 0
    }
}

/// Tool class that mines a block efficiently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    #[default]
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Sword,
    Shears,
}

/// Tool tiers in increasing strength. Gold mines fast but at wood tier for drop requirements.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ToolTier {
    #[default]
    None,
    Wood,
    Stone,
    Iron,
    Diamond,
}

/// How a block is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderKind {
    /// Not drawn (air, barriers).
    Invisible,
    /// Six-faced cube (greedy-meshable when opaque).
    #[default]
    Cube,
    /// Two crossed quads (plants).
    Cross,
    /// Baked block model from the resource pack.
    Model,
    /// Fluid surface.
    Fluid,
}

/// Render pass a block's faces belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderLayer {
    #[default]
    Opaque,
    /// Alpha-tested (leaves, plants).
    Cutout,
    /// Alpha-blended, sorted (water, glass, ice).
    Translucent,
}

/// Biome/climate tint applied to (parts of) a block's texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TintKind {
    #[default]
    None,
    Grass,
    Foliage,
    Water,
    Birch,
    Spruce,
    DryGrass,
}

/// Emission that depends on a property value, e.g. a lit furnace.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ConditionalEmission {
    pub property: String,
    pub values: BTreeMap<String, u8>,
}

/// Definition of a block type, as written in data packs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BlockDef {
    /// Ordered property declarations: `"facing:north,south,west,east"`, `"lit:bool"`,
    /// `"age:0..7"`.
    pub properties: Vec<String>,
    /// Default property values (otherwise the first allowed value).
    pub defaults: BTreeMap<String, String>,
    /// Time-to-break scale; negative = unbreakable.
    pub hardness: f32,
    pub resistance: f32,
    pub tool: ToolKind,
    pub tier: ToolTier,
    /// Drops only when mined with the right tool at or above `tier`.
    pub requires_tool: bool,
    pub shape: ShapeKind,
    /// False for plants, torches, fluids: no collision boxes.
    pub collision: bool,
    pub render: RenderKind,
    pub layer: RenderLayer,
    /// Full opaque cube: occludes neighbour faces, casts AO, blocks all light.
    pub opaque: bool,
    /// Light absorbed when passing through (defaults to 15 for opaque blocks, else 0).
    pub light_opacity: Option<u8>,
    pub emission: u8,
    pub emission_when: Option<ConditionalEmission>,
    /// Surface slipperiness (0.6 normal, 0.98 ice).
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    pub random_ticks: bool,
    /// Can be replaced by placing another block into it (air, grass, water, snow layer 1).
    pub replaceable: bool,
    /// Adds a `waterlogged:bool` property automatically.
    pub waterloggable: bool,
    /// This block *is* a fluid (e.g. `water`, with a `level:0..15` property).
    pub fluid: Option<String>,
    /// Always contains water (seagrass, kelp): like a permanently waterlogged block.
    pub water_filled: bool,
    pub climbable: bool,
    pub tint: TintKind,
    pub sound: String,
    /// Map colour as `#rrggbb`.
    pub map_color: String,
    /// Loot table id (defaults to `blocks/<name>`).
    pub drops: Option<String>,
    /// Register a matching block item.
    pub item: bool,
    /// Named behaviour implemented in code (ticks, interaction).
    pub behavior: Option<String>,
    /// Resource pack model id (defaults to `block/<name>`).
    pub model: Option<String>,
    /// Burn/ignite values (no fire in the base game, kept for mods).
    pub flammable: bool,
}

impl Default for BlockDef {
    fn default() -> Self {
        Self {
            properties: Vec::new(),
            defaults: BTreeMap::new(),
            hardness: 1.0,
            resistance: 1.0,
            tool: ToolKind::None,
            tier: ToolTier::None,
            requires_tool: false,
            shape: ShapeKind::Full,
            collision: true,
            render: RenderKind::Cube,
            layer: RenderLayer::Opaque,
            opaque: true,
            light_opacity: None,
            emission: 0,
            emission_when: None,
            friction: 0.6,
            speed_factor: 1.0,
            jump_factor: 1.0,
            random_ticks: false,
            replaceable: false,
            waterloggable: false,
            fluid: None,
            water_filled: false,
            climbable: false,
            tint: TintKind::None,
            sound: "stone".to_owned(),
            map_color: "#707070".to_owned(),
            drops: None,
            item: true,
            behavior: None,
            model: None,
            flammable: false,
        }
    }
}

impl BlockDef {
    /// Definition of air.
    pub fn air() -> Self {
        Self {
            hardness: 0.0,
            resistance: 0.0,
            shape: ShapeKind::Empty,
            collision: false,
            render: RenderKind::Invisible,
            opaque: false,
            replaceable: true,
            sound: "none".into(),
            map_color: "#000000".into(),
            item: false,
            ..Self::default()
        }
    }

    /// Definition of the placeholder used for blocks from missing mods.
    pub fn unknown() -> Self {
        Self {
            hardness: 0.5,
            map_color: "#ff00ff".into(),
            item: false,
            ..Self::default()
        }
    }
}

/// A parsed property declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub values: Vec<String>,
}

impl Property {
    /// Parses `name:v1,v2`, `name:bool` or `name:lo..hi`.
    pub fn parse(decl: &str) -> Result<Property, BlockError> {
        let bad = || BlockError::BadProperty(decl.to_owned());
        let (name, spec) = decl.split_once(':').ok_or_else(bad)?;
        let name = name.trim();
        let spec = spec.trim();
        if name.is_empty() || spec.is_empty() {
            return Err(bad());
        }
        let values: Vec<String> = if spec == "bool" {
            vec!["false".into(), "true".into()]
        } else if let Some((lo, hi)) = spec.split_once("..") {
            let lo: i64 = lo.trim().parse().map_err(|_| bad())?;
            let hi: i64 = hi.trim().parse().map_err(|_| bad())?;
            if hi < lo || hi - lo > 63 {
                return Err(bad());
            }
            (lo..=hi).map(|v| v.to_string()).collect()
        } else {
            spec.split(',').map(|v| v.trim().to_owned()).collect()
        };
        if values.iter().any(String::is_empty) {
            return Err(bad());
        }
        Ok(Property {
            name: name.to_owned(),
            values,
        })
    }
}

/// Errors while building the block registry or parsing state strings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockError {
    #[error("invalid property declaration {0:?}")]
    BadProperty(String),
    #[error("block {block}: default {property}={value} is not an allowed value")]
    BadDefault {
        block: String,
        property: String,
        value: String,
    },
    #[error("block {0}: too many states")]
    TooManyStates(String),
    #[error("more than 65535 block states in total")]
    StateSpaceFull,
    #[error("unknown block {0}")]
    UnknownBlock(String),
    #[error("block {block} has no property {property}")]
    UnknownProperty { block: String, property: String },
    #[error("block {block}: {property} cannot be {value}")]
    BadValue {
        block: String,
        property: String,
        value: String,
    },
    #[error("malformed block state string {0:?}")]
    Malformed(String),
    #[error("registry error: {0}")]
    Registry(#[from] hearth_core::RegistryError),
}

/// A block type after registration.
#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub name: ResourceLocation,
    pub def: BlockDef,
    pub properties: Vec<Property>,
    /// Mixed-radix strides for packing property value indices into a state offset.
    strides: Vec<u16>,
    pub first_state: BlockStateId,
    pub state_count: u16,
    pub default_state: BlockStateId,
    pub map_color: [u8; 3],
}

impl Block {
    pub fn property_index(&self, name: &str) -> Option<usize> {
        self.properties.iter().position(|p| p.name == name)
    }

    /// Value index of property `p` in `state` (which must belong to this block).
    #[inline]
    fn value_index(&self, state: BlockStateId, p: usize) -> usize {
        let offset = (state.0 - self.first_state.0) as usize;
        (offset / self.strides[p] as usize) % self.properties[p].values.len()
    }

    fn with_value_index(&self, state: BlockStateId, p: usize, v: usize) -> BlockStateId {
        let cur = self.value_index(state, p);
        let stride = self.strides[p] as i32;
        BlockStateId((state.0 as i32 + (v as i32 - cur as i32) * stride) as u16)
    }
}

bitflags_lite! {
    /// Precomputed per-state flags.
    pub struct StateFlags: u32 {
        const AIR = 1 << 0;
        /// Full opaque cube (occludes neighbours, casts AO, blocks all light).
        const OPAQUE = 1 << 1;
        /// Collision shape is exactly the unit cube.
        const FULL_COLLISION = 1 << 2;
        /// Has any collision boxes.
        const HAS_COLLISION = 1 << 3;
        const REPLACEABLE = 1 << 4;
        /// This state is a fluid block.
        const FLUID = 1 << 5;
        /// Contains water (fluid block or waterlogged).
        const WATER = 1 << 6;
        const RANDOM_TICKS = 1 << 7;
        const EMITS_LIGHT = 1 << 8;
        /// Top face can support blocks such as torches.
        const SOLID_TOP = 1 << 9;
        const CLIMBABLE = 1 << 10;
        /// Visual shape is a full cube (glass, leaves) — used for face culling between equals.
        const FULL_CUBE_SHAPE = 1 << 11;
        const LAYER_CUTOUT = 1 << 12;
        const LAYER_TRANSLUCENT = 1 << 13;
        const INVISIBLE = 1 << 14;
        const WATERLOGGED = 1 << 15;
        /// Blocks any sky light (opacity > 0): used by the column heightmap.
        const BLOCKS_SKY = 1 << 16;
    }
}

/// All blocks and states. Built once at load time and then shared read-only.
#[derive(Debug, Clone)]
pub struct BlockRegistry {
    blocks: Registry<Block>,
    // Per-state tables (index = BlockStateId).
    state_block: Vec<BlockId>,
    state_flags: Vec<StateFlags>,
    /// `emission << 4 | opacity`.
    state_light: Vec<u8>,
    state_collision: Vec<ShapeId>,
    state_outline: Vec<ShapeId>,
    /// Fluid amount 0..=8 (8 = source / full block of water).
    state_fluid: Vec<u8>,
    shapes: Vec<Shape>,
    shape_ids: FxHashMap<ShapeKey, ShapeId>,
    unknown: BlockStateId,
    water_source: BlockStateId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShapeKey(Vec<[i64; 6]>);

impl ShapeKey {
    fn of(s: &Shape) -> Self {
        let q = |v: f64| (v * 4096.0).round() as i64;
        ShapeKey(
            s.boxes
                .iter()
                .map(|b| {
                    [
                        q(b.min.x),
                        q(b.min.y),
                        q(b.min.z),
                        q(b.max.x),
                        q(b.max.y),
                        q(b.max.z),
                    ]
                })
                .collect(),
        )
    }
}

struct StateProps<'a> {
    block: &'a Block,
    offset: usize,
}

impl PropertyLookup for StateProps<'_> {
    fn value(&self, name: &str) -> Option<&str> {
        let p = self.block.property_index(name)?;
        let prop = &self.block.properties[p];
        let v = (self.offset / self.block.strides[p] as usize) % prop.values.len();
        Some(prop.values[v].as_str())
    }
}

fn parse_color(s: &str) -> [u8; 3] {
    let h = s.trim_start_matches('#');
    if h.len() == 6
        && let Ok(v) = u32::from_str_radix(h, 16)
    {
        return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
    }
    [255, 0, 255]
}

impl BlockRegistry {
    /// Builds the registry. `hearth:air` is always state 0 and `hearth:unknown` is always
    /// present; they are added automatically if `defs` doesn't contain them.
    pub fn build(defs: Vec<(ResourceLocation, BlockDef)>) -> Result<Self, BlockError> {
        let mut reg = Self {
            blocks: Registry::new("block", u16::MAX as usize),
            state_block: Vec::new(),
            state_flags: Vec::new(),
            state_light: Vec::new(),
            state_collision: Vec::new(),
            state_outline: Vec::new(),
            state_fluid: Vec::new(),
            shapes: Vec::new(),
            shape_ids: FxHashMap::default(),
            unknown: BlockStateId(0),
            water_source: BlockStateId(0),
        };
        reg.intern_shape(Shape::empty());
        reg.intern_shape(Shape::full());

        let air = ResourceLocation::game("air");
        let unknown = ResourceLocation::game("unknown");
        let mut defs = defs;
        let air_def = match defs.iter().position(|(n, _)| *n == air) {
            Some(i) => defs.remove(i).1,
            None => BlockDef::air(),
        };
        reg.add_block(air, air_def)?;
        if !defs.iter().any(|(n, _)| *n == unknown) {
            defs.push((unknown.clone(), BlockDef::unknown()));
        }
        for (name, def) in defs {
            reg.add_block(name, def)?;
        }
        reg.unknown = reg
            .default_state_of(&unknown)
            .expect("unknown block registered above");
        reg.water_source = reg
            .parse_state("hearth:water[level=0]")
            .or_else(|_| reg.parse_state("hearth:water"))
            .unwrap_or(BlockStateId::AIR);
        reg.blocks.freeze();
        Ok(reg)
    }

    fn intern_shape(&mut self, s: Shape) -> ShapeId {
        let key = ShapeKey::of(&s);
        if let Some(id) = self.shape_ids.get(&key) {
            return *id;
        }
        let id = ShapeId(self.shapes.len() as u16);
        self.shapes.push(s);
        self.shape_ids.insert(key, id);
        id
    }

    fn add_block(&mut self, name: ResourceLocation, mut def: BlockDef) -> Result<(), BlockError> {
        let mut properties = def
            .properties
            .iter()
            .map(|d| Property::parse(d))
            .collect::<Result<Vec<_>, _>>()?;
        if def.waterloggable && !properties.iter().any(|p| p.name == "waterlogged") {
            properties.push(Property {
                name: "waterlogged".into(),
                values: vec!["false".into(), "true".into()],
            });
            def.properties.push("waterlogged:bool".into());
        }
        // Mixed-radix strides; the last property varies fastest.
        let mut strides = vec![0u16; properties.len()];
        let mut count: usize = 1;
        for i in (0..properties.len()).rev() {
            strides[i] = count as u16;
            count *= properties[i].values.len();
            if count > 4096 {
                return Err(BlockError::TooManyStates(name.to_string()));
            }
        }
        let first = self.state_block.len();
        if first + count > u16::MAX as usize {
            return Err(BlockError::StateSpaceFull);
        }
        // Default state offset.
        let mut default_offset = 0usize;
        for (i, p) in properties.iter().enumerate() {
            let vi =
                match def.defaults.get(&p.name) {
                    Some(v) => p.values.iter().position(|x| x == v).ok_or_else(|| {
                        BlockError::BadDefault {
                            block: name.to_string(),
                            property: p.name.clone(),
                            value: v.clone(),
                        }
                    })?,
                    None => 0,
                };
            default_offset += vi * strides[i] as usize;
        }
        let id = BlockId(self.blocks.len() as u16);
        let map_color = parse_color(&def.map_color);
        let block = Block {
            id,
            name: name.clone(),
            def,
            properties,
            strides,
            first_state: BlockStateId(first as u16),
            state_count: count as u16,
            default_state: BlockStateId((first + default_offset) as u16),
            map_color,
        };
        // Per-state tables.
        for offset in 0..count {
            let props = StateProps {
                block: &block,
                offset,
            };
            let def = &block.def;
            let (mut collision, outline) = shapes_for(&def.shape, &props);
            if !def.collision {
                collision = Shape::empty();
            }
            let waterlogged = props.flag("waterlogged") || def.water_filled;
            let is_fluid = def.fluid.is_some();
            let fluid_amount = if is_fluid {
                let level = props.int("level").clamp(0, 15);
                if level == 0 || level >= 8 {
                    8
                } else {
                    (8 - level) as u8
                }
            } else if waterlogged {
                8
            } else {
                0
            };
            let mut flags = StateFlags::empty();
            flags.set(StateFlags::AIR, id.0 == 0);
            flags.set(StateFlags::OPAQUE, def.opaque);
            flags.set(StateFlags::FULL_COLLISION, collision.is_full_cube());
            flags.set(StateFlags::HAS_COLLISION, !collision.is_empty());
            flags.set(StateFlags::REPLACEABLE, def.replaceable);
            flags.set(StateFlags::FLUID, is_fluid);
            flags.set(
                StateFlags::WATER,
                (is_fluid && def.fluid.as_deref() == Some("water")) || waterlogged,
            );
            flags.set(StateFlags::WATERLOGGED, waterlogged);
            flags.set(StateFlags::RANDOM_TICKS, def.random_ticks);
            flags.set(
                StateFlags::SOLID_TOP,
                collision.covers_face(hearth_math::Direction::Up),
            );
            flags.set(StateFlags::CLIMBABLE, def.climbable);
            flags.set(StateFlags::FULL_CUBE_SHAPE, outline.is_full_cube());
            flags.set(StateFlags::LAYER_CUTOUT, def.layer == RenderLayer::Cutout);
            flags.set(
                StateFlags::LAYER_TRANSLUCENT,
                def.layer == RenderLayer::Translucent,
            );
            flags.set(StateFlags::INVISIBLE, def.render == RenderKind::Invisible);
            let mut emission = def.emission;
            if let Some(cond) = &def.emission_when
                && let Some(v) = props.value(&cond.property)
                && let Some(e) = cond.values.get(v)
            {
                emission = *e;
            }
            let emission = emission.min(15);
            let mut opacity = def
                .light_opacity
                .unwrap_or(if def.opaque { 15 } else { 0 })
                .min(15);
            if waterlogged && opacity < 2 {
                // Waterlogged blocks attenuate like water.
                opacity = 2;
            }
            flags.set(StateFlags::EMITS_LIGHT, emission > 0);
            flags.set(StateFlags::BLOCKS_SKY, opacity > 0);
            let collision_id = self.intern_shape(collision);
            let outline_id = self.intern_shape(outline);
            self.state_block.push(id);
            self.state_flags.push(flags);
            self.state_light.push((emission << 4) | opacity);
            self.state_collision.push(collision_id);
            self.state_outline.push(outline_id);
            self.state_fluid.push(fluid_amount);
        }
        self.blocks.register(name, block)?;
        Ok(())
    }

    // ------------------------------------------------------------------ lookups

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn state_count(&self) -> usize {
        self.state_block.len()
    }

    #[inline]
    pub fn block(&self, id: BlockId) -> &Block {
        self.blocks.get(id.0 as u32).expect("valid block id")
    }

    #[inline]
    pub fn block_of(&self, state: BlockStateId) -> &Block {
        self.block(self.state_block[state.0 as usize])
    }

    #[inline]
    pub fn block_id_of(&self, state: BlockStateId) -> BlockId {
        self.state_block[state.0 as usize]
    }

    pub fn block_by_name(&self, name: &ResourceLocation) -> Option<&Block> {
        self.blocks.by_name(name)
    }

    pub fn block_id(&self, name: &str) -> Option<BlockId> {
        self.blocks.id_of_str(name).map(|i| BlockId(i as u16))
    }

    pub fn default_state_of(&self, name: &ResourceLocation) -> Option<BlockStateId> {
        self.blocks.by_name(name).map(|b| b.default_state)
    }

    /// Default state of a block by string name. Panics if missing — use for built-in ids only.
    pub fn default_state(&self, name: &str) -> BlockStateId {
        self.blocks
            .id_of_str(name)
            .and_then(|i| self.blocks.get(i))
            .map(|b| b.default_state)
            .unwrap_or_else(|| panic!("unknown built-in block {name}"))
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, _, b)| b)
    }

    pub fn block_registry(&self) -> &Registry<Block> {
        &self.blocks
    }

    /// The placeholder state for unknown blocks.
    pub fn unknown_state(&self) -> BlockStateId {
        self.unknown
    }

    /// Water source state (air if no water block is registered).
    pub fn water_source(&self) -> BlockStateId {
        self.water_source
    }

    #[inline]
    pub fn flags(&self, s: BlockStateId) -> StateFlags {
        self.state_flags[s.0 as usize]
    }

    #[inline]
    pub fn has(&self, s: BlockStateId, f: StateFlags) -> bool {
        self.state_flags[s.0 as usize].contains(f)
    }

    #[inline]
    pub fn is_opaque(&self, s: BlockStateId) -> bool {
        self.has(s, StateFlags::OPAQUE)
    }

    #[inline]
    pub fn light_emission(&self, s: BlockStateId) -> u8 {
        self.state_light[s.0 as usize] >> 4
    }

    #[inline]
    pub fn light_opacity(&self, s: BlockStateId) -> u8 {
        self.state_light[s.0 as usize] & 15
    }

    /// Fluid amount 0..=8.
    #[inline]
    pub fn fluid_amount(&self, s: BlockStateId) -> u8 {
        self.state_fluid[s.0 as usize]
    }

    #[inline]
    pub fn collision_shape(&self, s: BlockStateId) -> &Shape {
        &self.shapes[self.state_collision[s.0 as usize].0 as usize]
    }

    #[inline]
    pub fn outline_shape(&self, s: BlockStateId) -> &Shape {
        &self.shapes[self.state_outline[s.0 as usize].0 as usize]
    }

    /// Raw flag table (for workers that want a slice).
    pub fn flag_table(&self) -> &[StateFlags] {
        &self.state_flags
    }

    /// Raw light table (`emission << 4 | opacity`).
    pub fn light_table(&self) -> &[u8] {
        &self.state_light
    }

    // ------------------------------------------------------------------ properties

    /// Value of property `name` in `state`, if the block has it.
    pub fn get(&self, state: BlockStateId, name: &str) -> Option<&str> {
        let b = self.block_of(state);
        let p = b.property_index(name)?;
        let v = b.value_index(state, p);
        Some(b.properties[p].values[v].as_str())
    }

    pub fn get_bool(&self, state: BlockStateId, name: &str) -> bool {
        self.get(state, name) == Some("true")
    }

    pub fn get_int(&self, state: BlockStateId, name: &str) -> Option<i64> {
        self.get(state, name).and_then(|v| v.parse().ok())
    }

    pub fn get_dir(&self, state: BlockStateId, name: &str) -> Option<hearth_math::Direction> {
        self.get(state, name)
            .and_then(hearth_math::Direction::from_name)
    }

    /// Returns `state` with property `name` set to `value`.
    pub fn with(
        &self,
        state: BlockStateId,
        name: &str,
        value: &str,
    ) -> Result<BlockStateId, BlockError> {
        let b = self.block_of(state);
        let p = b
            .property_index(name)
            .ok_or_else(|| BlockError::UnknownProperty {
                block: b.name.to_string(),
                property: name.to_owned(),
            })?;
        let v = b.properties[p]
            .values
            .iter()
            .position(|x| x == value)
            .ok_or_else(|| BlockError::BadValue {
                block: b.name.to_string(),
                property: name.to_owned(),
                value: value.to_owned(),
            })?;
        Ok(b.with_value_index(state, p, v))
    }

    /// Like [`Self::with`] but returns `state` unchanged when the property doesn't exist.
    pub fn with_or_same(&self, state: BlockStateId, name: &str, value: &str) -> BlockStateId {
        self.with(state, name, value).unwrap_or(state)
    }

    /// Parses `ns:block[prop=value,...]` (missing properties take defaults).
    pub fn parse_state(&self, s: &str) -> Result<BlockStateId, BlockError> {
        let s = s.trim();
        let (name, props) = match s.find('[') {
            Some(i) => {
                if !s.ends_with(']') {
                    return Err(BlockError::Malformed(s.to_owned()));
                }
                (&s[..i], Some(&s[i + 1..s.len() - 1]))
            }
            None => (s, None),
        };
        let rl = ResourceLocation::parse(name).map_err(|_| BlockError::Malformed(s.to_owned()))?;
        let block = self
            .blocks
            .by_name(&rl)
            .ok_or_else(|| BlockError::UnknownBlock(rl.to_string()))?;
        let mut state = block.default_state;
        if let Some(props) = props {
            for kv in props.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| BlockError::Malformed(s.to_owned()))?;
                state = self.with(state, k.trim(), v.trim())?;
            }
        }
        Ok(state)
    }

    /// Formats a state as `ns:block[prop=value,...]`.
    pub fn state_string(&self, state: BlockStateId) -> String {
        let b = self.block_of(state);
        if b.properties.is_empty() {
            return b.name.to_string();
        }
        let props: Vec<String> = b
            .properties
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{}={}", p.name, p.values[b.value_index(state, i)]))
            .collect();
        format!("{}[{}]", b.name, props.join(","))
    }

    /// All states of a block.
    pub fn states_of(&self, id: BlockId) -> impl Iterator<Item = BlockStateId> {
        let b = self.block(id);
        (b.first_state.0..b.first_state.0 + b.state_count).map(BlockStateId)
    }

    /// Names of all states, indexed by state id (persisted in saves as the state palette).
    pub fn state_names(&self) -> Vec<String> {
        (0..self.state_count())
            .map(|i| self.state_string(BlockStateId(i as u16)))
            .collect()
    }
}

impl fmt::Display for BlockStateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn test_registry() -> BlockRegistry {
        let rl = ResourceLocation::game;
        let defs = vec![
            (rl("stone"), BlockDef::default()),
            (
                rl("glass"),
                BlockDef {
                    opaque: false,
                    layer: RenderLayer::Cutout,
                    ..BlockDef::default()
                },
            ),
            (
                rl("oak_stairs"),
                BlockDef {
                    properties: vec![
                        "facing:north,south,west,east".into(),
                        "half:top,bottom".into(),
                        "shape:straight,inner_left,inner_right,outer_left,outer_right".into(),
                    ],
                    defaults: [("half".to_string(), "bottom".to_string())].into(),
                    shape: ShapeKind::Stairs,
                    opaque: false,
                    waterloggable: true,
                    ..BlockDef::default()
                },
            ),
            (
                rl("water"),
                BlockDef {
                    properties: vec!["level:0..15".into()],
                    fluid: Some("water".into()),
                    collision: false,
                    opaque: false,
                    render: RenderKind::Fluid,
                    layer: RenderLayer::Translucent,
                    light_opacity: Some(2),
                    replaceable: true,
                    ..BlockDef::default()
                },
            ),
            (
                rl("furnace"),
                BlockDef {
                    properties: vec!["facing:north,south,west,east".into(), "lit:bool".into()],
                    emission_when: Some(ConditionalEmission {
                        property: "lit".into(),
                        values: [("true".to_string(), 13u8)].into(),
                    }),
                    ..BlockDef::default()
                },
            ),
            (
                rl("torch"),
                BlockDef {
                    shape: ShapeKind::Boxes(vec![[6.0, 0.0, 6.0, 10.0, 10.0, 10.0]]),
                    collision: false,
                    opaque: false,
                    render: RenderKind::Model,
                    layer: RenderLayer::Cutout,
                    emission: 14,
                    ..BlockDef::default()
                },
            ),
        ];
        BlockRegistry::build(defs).unwrap()
    }

    #[test]
    fn air_is_state_zero_and_unknown_exists() {
        let r = test_registry();
        assert_eq!(r.default_state("air"), BlockStateId::AIR);
        assert!(r.has(BlockStateId::AIR, StateFlags::AIR));
        assert!(!r.has(BlockStateId::AIR, StateFlags::HAS_COLLISION));
        assert_ne!(r.unknown_state(), BlockStateId::AIR);
        assert_eq!(r.block_of(r.water_source()).name.path(), "water");
    }

    #[test]
    fn state_counts_and_defaults() {
        let r = test_registry();
        let stairs = r
            .block_by_name(&ResourceLocation::game("oak_stairs"))
            .unwrap();
        assert_eq!(stairs.state_count, 4 * 2 * 5 * 2);
        let d = stairs.default_state;
        assert_eq!(r.get(d, "half"), Some("bottom"));
        assert_eq!(r.get(d, "facing"), Some("north"));
        assert_eq!(r.get(d, "waterlogged"), Some("false"));
    }

    #[test]
    fn with_and_parse_round_trip() {
        let r = test_registry();
        for state in r.states_of(r.block_id("oak_stairs").unwrap()) {
            let s = r.state_string(state);
            assert_eq!(r.parse_state(&s).unwrap(), state, "{s}");
        }
        let s = r
            .parse_state("hearth:oak_stairs[facing=east,half=top,waterlogged=true]")
            .unwrap();
        assert_eq!(r.get(s, "facing"), Some("east"));
        assert!(r.has(s, StateFlags::WATER));
        assert!(r.has(s, StateFlags::WATERLOGGED));
        assert_eq!(r.fluid_amount(s), 8);
        let s2 = r.with(s, "facing", "west").unwrap();
        assert_eq!(r.get(s2, "facing"), Some("west"));
        assert_eq!(r.get(s2, "half"), Some("top"));
        assert!(r.parse_state("hearth:oak_stairs[facing=up]").is_err());
        assert!(r.parse_state("hearth:nope").is_err());
        assert!(r.parse_state("hearth:stone[").is_err());
    }

    #[test]
    fn precomputed_flags_and_light() {
        let r = test_registry();
        let stone = r.default_state("stone");
        assert!(r.is_opaque(stone));
        assert_eq!(r.light_opacity(stone), 15);
        assert!(r.has(stone, StateFlags::FULL_COLLISION | StateFlags::SOLID_TOP));
        let glass = r.default_state("glass");
        assert!(!r.is_opaque(glass));
        assert!(r.has(glass, StateFlags::FULL_CUBE_SHAPE));
        let torch = r.default_state("torch");
        assert_eq!(r.light_emission(torch), 14);
        assert!(!r.has(torch, StateFlags::HAS_COLLISION));
        assert!(!r.outline_shape(torch).is_empty());
        let lit = r.parse_state("furnace[lit=true]").unwrap();
        let unlit = r.parse_state("furnace[lit=false]").unwrap();
        assert_eq!(r.light_emission(lit), 13);
        assert_eq!(r.light_emission(unlit), 0);
        let water = r.water_source();
        assert_eq!(r.fluid_amount(water), 8);
        let flowing = r.parse_state("water[level=3]").unwrap();
        assert_eq!(r.fluid_amount(flowing), 5);
        assert_eq!(r.light_opacity(water), 2);
    }

    #[test]
    fn bad_definitions_are_rejected() {
        let bad = vec![(
            ResourceLocation::game("x"),
            BlockDef {
                properties: vec!["broken".into()],
                ..BlockDef::default()
            },
        )];
        assert!(BlockRegistry::build(bad).is_err());
        let bad_default = vec![(
            ResourceLocation::game("y"),
            BlockDef {
                properties: vec!["a:bool".into()],
                defaults: [("a".to_string(), "maybe".to_string())].into(),
                ..BlockDef::default()
            },
        )];
        assert!(matches!(
            BlockRegistry::build(bad_default),
            Err(BlockError::BadDefault { .. })
        ));
    }

    #[test]
    fn def_deserializes_from_json() {
        let json = r#"{
            "properties": ["facing:north,south,west,east", "open:bool", "half:top,bottom"],
            "shape": "trapdoor", "opaque": false, "layer": "cutout", "hardness": 3.0,
            "tool": "axe", "waterloggable": true
        }"#;
        let def: BlockDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.shape, ShapeKind::Trapdoor);
        assert_eq!(def.tool, ToolKind::Axe);
        assert!(serde_json::from_str::<BlockDef>(r#"{"hardnes": 1}"#).is_err());
    }
}
