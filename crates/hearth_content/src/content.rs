//! Loading all domains into typed tables. Later packs override earlier ones entry by entry;
//! singleton files (`units`, `time`, `body/human`) are replaced whole.

use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;

use crate::diag::Report;
use crate::generate::{ItemDef, generate_items};
use crate::id::IdRef;
use crate::schema::body::{BodyParams, Garment, Illness, Injury};
use crate::schema::config::{BalanceKey, BalancePreset, TimeConfig, Units};
use crate::schema::ecosystem::Ecosystem;
use crate::schema::era::Era;
use crate::schema::fauna::Animal;
use crate::schema::flora::Plant;
use crate::schema::geology::{Deposit, Mineral, Province, Rock, Soil};
use crate::schema::humans::{Chromosome, GenePool, GeneticsSettings, Locus, Species, Trait};
use crate::schema::item::{Item, ItemForm};
use crate::schema::knowledge::Knowledge;
use crate::schema::life::{LifeTable, Moment};
use crate::schema::material::Material;
use crate::schema::mind::Routine;
use crate::schema::process::Process;
use crate::schema::psyche::{Feeling, Tendency, Value};
use crate::schema::social::{Norm, Ways};
use crate::schema::station::{ConstructionPiece, Workstation};
use crate::schema::{Entry, Status};
use crate::source::{EntryFile, SourceFile, domain_files, find_id_line, parse, singleton_files};

/// Where an entry was defined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub pack: usize,
}

/// All entries of one domain, in load order, indexed by full id (`namespace:path`).
#[derive(Debug, Clone)]
pub struct Table<T> {
    entries: Vec<T>,
    origins: Vec<Origin>,
    index: FxHashMap<String, usize>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            origins: Vec::new(),
            index: FxHashMap::default(),
        }
    }
}

impl<T> Table<T> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.index.get(id).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.index.get(id).map(|&i| &mut self.entries[i])
    }

    pub fn contains(&self, id: &IdRef) -> bool {
        self.index.contains_key(id.as_str())
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    pub fn at(&self, i: usize) -> &T {
        &self.entries[i]
    }

    pub fn origin(&self, i: usize) -> &Origin {
        &self.origins[i]
    }

    pub fn origin_of(&self, id: &str) -> Option<&Origin> {
        self.index.get(id).map(|&i| &self.origins[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter()
    }

    pub fn iter_with_origin(&self) -> impl Iterator<Item = (&T, &Origin)> {
        self.entries.iter().zip(&self.origins)
    }

    /// Inserts or overrides (same id) an entry.
    pub fn upsert(&mut self, id: String, value: T, origin: Origin) -> Option<Origin> {
        match self.index.get(&id) {
            Some(&i) => {
                self.entries[i] = value;
                Some(std::mem::replace(&mut self.origins[i], origin))
            }
            None => {
                self.index.insert(id, self.entries.len());
                self.entries.push(value);
                self.origins.push(origin);
                None
            }
        }
    }
}

impl<T: Entry> Table<T> {
    pub fn count(&self, status: Status) -> usize {
        self.entries.iter().filter(|e| e.status() == status).count()
    }
}

/// The complete loaded content.
#[derive(Debug, Clone)]
pub struct Content {
    pub packs: Vec<PathBuf>,
    pub units: Units,
    pub time: TimeConfig,
    pub body: BodyParams,
    pub materials: Table<Material>,
    pub rocks: Table<Rock>,
    pub minerals: Table<Mineral>,
    pub provinces: Table<Province>,
    pub deposits: Table<Deposit>,
    pub soils: Table<Soil>,
    pub plants: Table<Plant>,
    pub animals: Table<Animal>,
    pub ecosystems: Table<Ecosystem>,
    pub species: Table<Species>,
    pub chromosomes: Table<Chromosome>,
    pub traits: Table<Trait>,
    pub loci: Table<Locus>,
    pub gene_pools: Table<GenePool>,
    pub genetics: Table<GeneticsSettings>,
    pub tendencies: Table<Tendency>,
    pub feelings: Table<Feeling>,
    pub values: Table<Value>,
    pub routines: Table<Routine>,
    pub life_tables: Table<LifeTable>,
    pub moments: Table<Moment>,
    pub norms: Table<Norm>,
    pub ways: Table<Ways>,
    pub forms: Table<ItemForm>,
    pub explicit_items: Table<Item>,
    /// Explicit items plus every form × material combination.
    pub items: Table<ItemDef>,
    pub processes: Table<Process>,
    pub knowledge: Table<Knowledge>,
    pub workstations: Table<Workstation>,
    pub construction: Table<ConstructionPiece>,
    pub injuries: Table<Injury>,
    pub illnesses: Table<Illness>,
    pub garments: Table<Garment>,
    pub eras: Table<Era>,
    pub balance_keys: Table<BalanceKey>,
    pub balance_presets: Table<BalancePreset>,
}

fn qualify(namespace: &str, id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("{namespace}:{id}")
    }
}

fn load_table<T: Entry>(packs: &[PathBuf], report: &mut Report) -> Table<T> {
    let mut table = Table::default();
    for file in domain_files(packs, T::DOMAIN, report) {
        load_file(&file, &mut table, report);
    }
    table
}

fn load_file<T: Entry>(file: &SourceFile, table: &mut Table<T>, report: &mut Report) {
    let Some(parsed) = parse::<EntryFile<T>>(file, report) else {
        return;
    };
    if parsed.schema != T::SCHEMA {
        report.error(
            "schema-version",
            Some(file.path.clone()),
            Some(1),
            format!(
                "file declares schema {} but `{}` is at schema {}",
                parsed.schema,
                T::DOMAIN,
                T::SCHEMA
            ),
        );
        return;
    }
    let mut seen_here: Vec<String> = Vec::new();
    for mut e in parsed.entries {
        let id = qualify(&file.namespace, e.id());
        let line = find_id_line(&file.text, &id);
        if hearth_core::ResourceLocation::parse(&id).is_err() {
            report.error(
                "bad-id",
                Some(file.path.clone()),
                line,
                format!("`{id}` is not a valid id (lowercase letters, digits, _ . / -)"),
            );
            continue;
        }
        if seen_here.contains(&id) {
            report.error(
                "duplicate-id",
                Some(file.path.clone()),
                line,
                format!("`{id}` is defined twice in this file"),
            );
            continue;
        }
        seen_here.push(id.clone());
        e.set_id(id.clone());
        let origin = Origin {
            file: file.path.clone(),
            line,
            pack: file.pack,
        };
        if let Some(prev) = table.upsert(id.clone(), e, origin)
            && prev.pack == file.pack
            && prev.file != file.path
        {
            report.error(
                "duplicate-id",
                Some(file.path.clone()),
                line,
                format!("`{id}` is also defined in {}", prev.file.display()),
            );
        }
    }
}

fn load_singleton<T: DeserializeOwned>(
    packs: &[PathBuf],
    name: &str,
    report: &mut Report,
) -> Option<T> {
    let files = singleton_files(packs, name, report);
    if files.is_empty() {
        report.error(
            "missing-file",
            None,
            None,
            format!("no `{name}.ron` in any data pack"),
        );
        return None;
    }
    let mut out = None;
    for f in &files {
        if let Some(v) = parse::<T>(f, report) {
            out = Some(v);
        }
    }
    out
}

impl Content {
    /// Loads every domain from `packs` (base pack first). Returns `None` when a required file is
    /// missing or unreadable; the report lists every problem found either way.
    pub fn load(packs: &[PathBuf]) -> (Option<Content>, Report) {
        let mut r = Report::default();
        let units = load_singleton::<Units>(packs, "units", &mut r);
        let time = load_singleton::<TimeConfig>(packs, "time", &mut r);
        let body = load_singleton::<BodyParams>(packs, "body/human", &mut r);
        let materials = load_table::<Material>(packs, &mut r);
        let forms = load_table::<ItemForm>(packs, &mut r);
        let explicit_items = load_table::<Item>(packs, &mut r);
        let garments = load_table::<crate::schema::body::Garment>(packs, &mut r);
        let mut items = generate_items(&forms, &materials, &explicit_items, &garments, &mut r);
        let animals = load_table::<Animal>(packs, &mut r);
        let mut processes = load_table::<Process>(packs, &mut r);
        let mut knowledge = load_table::<Knowledge>(packs, &mut r);
        // Carcasses and the ways of working them, from each species' data.
        crate::butchery::generate(
            &animals,
            &materials,
            &mut items,
            &mut processes,
            &mut knowledge,
        );
        // Putting up and taking down each construction piece.
        let construction = load_table::<ConstructionPiece>(packs, &mut r);
        crate::building::generate(&construction, &mut processes, &mut knowledge);
        let content = Content {
            packs: packs.to_vec(),
            materials,
            rocks: load_table(packs, &mut r),
            minerals: load_table(packs, &mut r),
            provinces: load_table(packs, &mut r),
            deposits: load_table(packs, &mut r),
            soils: load_table(packs, &mut r),
            plants: load_table(packs, &mut r),
            animals,
            ecosystems: load_table(packs, &mut r),
            species: load_table(packs, &mut r),
            chromosomes: load_table(packs, &mut r),
            traits: load_table(packs, &mut r),
            loci: load_table(packs, &mut r),
            gene_pools: load_table(packs, &mut r),
            genetics: load_table(packs, &mut r),
            tendencies: load_table(packs, &mut r),
            feelings: load_table(packs, &mut r),
            values: load_table(packs, &mut r),
            routines: load_table(packs, &mut r),
            life_tables: load_table(packs, &mut r),
            moments: load_table(packs, &mut r),
            norms: load_table(packs, &mut r),
            ways: load_table(packs, &mut r),
            forms,
            explicit_items,
            items,
            processes,
            knowledge,
            workstations: load_table(packs, &mut r),
            construction,
            injuries: load_table(packs, &mut r),
            illnesses: load_table(packs, &mut r),
            garments,
            eras: load_table(packs, &mut r),
            balance_keys: load_table(packs, &mut r),
            balance_presets: load_table(packs, &mut r),
            units: match units {
                Some(u) => u,
                None => return (None, r),
            },
            time: match time {
                Some(t) => t,
                None => return (None, r),
            },
            body: match body {
                Some(b) => b,
                None => return (None, r),
            },
        };
        for (name, schema) in [
            ("units", content.units.schema),
            ("time", content.time.schema),
            ("body/human", content.body.schema),
        ] {
            if schema != 1 {
                r.error(
                    "schema-version",
                    None,
                    None,
                    format!("`{name}` declares schema {schema}; this build reads schema 1"),
                );
            }
        }
        (Some(content), r)
    }

    /// The repository's base data directory (`<repo>/data`), for tools and tests.
    pub fn base_pack_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    /// Loads just the base pack; panics with the report if it is broken (tests, tools).
    pub fn load_base() -> Content {
        let (c, r) = Content::load(&[Content::base_pack_dir()]);
        match c {
            Some(c) if r.errors() == 0 => c,
            _ => {
                let msgs: Vec<String> = r.sorted().iter().map(|d| d.to_string()).collect();
                panic!("base content failed to load:\n{}", msgs.join("\n"))
            }
        }
    }

    /// `(domain, id)` of every entry marked `uncertain: true`.
    pub fn uncertain_entries(&self) -> Vec<(&'static str, String)> {
        fn u<T: Entry>(t: &Table<T>, out: &mut Vec<(&'static str, String)>) {
            out.extend(
                t.iter()
                    .filter(|e| e.uncertain())
                    .map(|e| (T::DOMAIN, e.id().to_owned())),
            );
        }
        let mut out = Vec::new();
        u(&self.materials, &mut out);
        u(&self.rocks, &mut out);
        u(&self.minerals, &mut out);
        u(&self.provinces, &mut out);
        u(&self.deposits, &mut out);
        u(&self.soils, &mut out);
        u(&self.plants, &mut out);
        u(&self.animals, &mut out);
        u(&self.ecosystems, &mut out);
        u(&self.species, &mut out);
        u(&self.traits, &mut out);
        u(&self.loci, &mut out);
        u(&self.gene_pools, &mut out);
        u(&self.tendencies, &mut out);
        u(&self.feelings, &mut out);
        u(&self.values, &mut out);
        u(&self.routines, &mut out);
        u(&self.life_tables, &mut out);
        u(&self.moments, &mut out);
        u(&self.norms, &mut out);
        u(&self.ways, &mut out);
        u(&self.forms, &mut out);
        u(&self.processes, &mut out);
        u(&self.knowledge, &mut out);
        u(&self.workstations, &mut out);
        u(&self.construction, &mut out);
        u(&self.garments, &mut out);
        u(&self.injuries, &mut out);
        u(&self.illnesses, &mut out);
        u(&self.eras, &mut out);
        out
    }

    /// Implemented / planned counts per domain for the progress table.
    pub fn status_counts(&self) -> Vec<(&'static str, usize, usize)> {
        fn c<T: Entry>(name: &'static str, t: &Table<T>) -> (&'static str, usize, usize) {
            (name, t.count(Status::Implemented), t.count(Status::Planned))
        }
        vec![
            c("Materials", &self.materials),
            c("Rock types", &self.rocks),
            c("Minerals", &self.minerals),
            c("Geological provinces", &self.provinces),
            c("Deposit models", &self.deposits),
            c("Soils", &self.soils),
            c("Plant species", &self.plants),
            c("Animal species", &self.animals),
            c("Ecosystems", &self.ecosystems),
            c("Species of person", &self.species),
            c("Heritable traits", &self.traits),
            c("Named loci", &self.loci),
            c("Gene pools", &self.gene_pools),
            c("Behaviour tendencies", &self.tendencies),
            c("Feelings", &self.feelings),
            c("Values", &self.values),
            c("Routines", &self.routines),
            c("Life tables", &self.life_tables),
            c("Moments of childhood", &self.moments),
            c("Norms", &self.norms),
            c("Ways with strangers and quarrels", &self.ways),
            c("Item forms", &self.forms),
            c("Processes", &self.processes),
            c("Knowledge nodes", &self.knowledge),
            c("Workstations", &self.workstations),
            c("Construction pieces", &self.construction),
            c("Garments", &self.garments),
            c("Injuries", &self.injuries),
            c("Illnesses", &self.illnesses),
            c("Eras", &self.eras),
        ]
    }
}
