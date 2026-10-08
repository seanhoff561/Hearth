//! What a person carries (v2 §10.2): a thing in each hand or one in both, garments worn by layer
//! with what hangs from their attachment points, a load on the back, and something dragged
//! behind. The load slows and tires as it does people: about a quarter of the body's mass is
//! carried in comfort, half only with great effort; more is lifted for a moment or dragged.

use hearth_content::schema::body::ClothingLayer;
use serde::{Deserialize, Serialize};

use crate::registry::Items;
use crate::stack::Stack;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hand {
    Left,
    Right,
}

/// The most one hand carries (kg): a full basket by its rim, a large stone.
pub const ONE_HAND_KG: f32 = 12.0;
/// Carried in both arms: up to half the body's mass.
pub const BOTH_HANDS_SHARE: f32 = 0.5;
/// Dragged: up to three times the body's mass, slowly.
pub const DRAG_SHARE: f32 = 3.0;

/// A worn garment and what hangs from its attachment points (one entry per point, in its
/// order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worn {
    pub stack: Stack,
    #[serde(default)]
    pub hung: Vec<Option<Stack>>,
}

/// Why something could not be taken up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The hand (or both) is full.
    HandsFull,
    /// Too heavy to carry: it can be dragged.
    DragIt,
    /// Too heavy to move at all.
    Immovable,
    /// Not that sort of thing (not a garment to wear, not something that hangs there).
    Wrong,
    /// The place is taken (a garment on that layer and region, a point already used).
    Taken,
    /// Unknown to the content.
    Unknown,
}

/// The load, against the body.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Load {
    /// Carried: in the hands, worn, hanging, on the back (kg).
    pub carried_kg: f32,
    /// Dragged behind (kg).
    pub dragged_kg: f32,
    /// Carried as a share of the body's mass.
    pub share: f32,
}

impl Load {
    /// The walking speed the load leaves (a factor): full to a fifth of the body's mass,
    /// falling to 55 % at over half.
    pub fn walk(&self) -> f32 {
        1.0 - 0.45 * ((self.share - 0.2) / 0.35).clamp(0.0, 1.0)
    }

    /// Whether the body can still run (not above two fifths of its mass) and sprint (a
    /// quarter).
    pub fn can_jog(&self) -> bool {
        self.share <= 0.4 && self.dragged_kg == 0.0
    }

    pub fn can_sprint(&self) -> bool {
        self.share <= 0.25 && self.dragged_kg == 0.0
    }

    /// The fastest a dragged load moves over ground of friction `mu` (m/s): a person pulls
    /// with about 120 W for long, so a 100 kg log on grass goes a quarter of a metre a second.
    pub fn drag_speed(&self, mu: f32) -> Option<f32> {
        (self.dragged_kg > 0.0).then(|| 120.0 / (mu.max(0.02) * self.dragged_kg * 9.81))
    }

    /// The extra work of the load at `speed_m_s` up a grade of `grade` (rise over run) for a
    /// body of `body_kg` (W): Pandolf's equation's load terms, and dragging at a quarter's
    /// efficiency over ground of friction `mu`.
    pub fn work_w(&self, body_kg: f32, speed_m_s: f32, grade: f32, mu: f32) -> f32 {
        let w = body_kg.max(1.0);
        let l = self.carried_kg;
        let v = speed_m_s.max(0.0);
        let g = (grade * 100.0).max(0.0);
        let standing = 2.0 * (w + l) * (l / w).powi(2);
        let moving = 1.2 * l * (1.5 * v * v + 0.35 * v * g);
        let drag = mu * self.dragged_kg * 9.81 * v / 0.25;
        standing + moving + drag
    }
}

/// What a person carries.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Carry {
    pub left: Option<Stack>,
    pub right: Option<Stack>,
    /// The right hand's load is held in both arms.
    #[serde(default)]
    pub both: bool,
    /// On the back: a back basket, a bundle tied on.
    #[serde(default)]
    pub back: Option<Stack>,
    #[serde(default)]
    pub worn: Vec<Worn>,
    /// Dragged behind.
    #[serde(default)]
    pub dragging: Option<Stack>,
}

impl Carry {
    /// Dressed in `garments` (their items), as a new person starts.
    pub fn dressed(items: &Items, garments: impl IntoIterator<Item = Stack>) -> Self {
        let mut c = Carry::default();
        for g in garments {
            let _ = c.wear(items, g);
        }
        c
    }

    pub fn load(&self, items: &Items, body_kg: f32) -> Load {
        let mass = |s: &Option<Stack>| s.as_ref().map_or(0.0, |s| s.mass(items));
        let worn: f32 = self
            .worn
            .iter()
            .map(|w| w.stack.mass(items) + w.hung.iter().map(mass).sum::<f32>())
            .sum();
        let carried = mass(&self.left) + mass(&self.right) + mass(&self.back) + worn;
        Load {
            carried_kg: carried,
            dragged_kg: mass(&self.dragging),
            share: carried / body_kg.max(1.0),
        }
    }

    /// Whether the hands are free (for climbing, swimming strongly, using both).
    pub fn hands_free(&self) -> bool {
        self.left.is_none() && self.right.is_none() && self.dragging.is_none()
    }

    /// Takes something up in a hand, or in both arms if it is too big or heavy for one.
    /// Gives it back, and why, if it cannot be held.
    #[allow(clippy::result_large_err)]
    pub fn hold(
        &mut self,
        items: &Items,
        stack: Stack,
        hand: Hand,
        body_kg: f32,
    ) -> Result<(), (Stack, Refusal)> {
        let Some(kind) = stack.kind(items) else {
            return Err((stack, Refusal::Unknown));
        };
        let mass = stack.mass(items);
        if mass > DRAG_SHARE * body_kg {
            return Err((stack, Refusal::Immovable));
        }
        if mass > BOTH_HANDS_SHARE * body_kg {
            return Err((stack, Refusal::DragIt));
        }
        let two_handed = mass > ONE_HAND_KG || kind.has_tag("two_handed");
        if two_handed {
            if self.left.is_some() || self.right.is_some() || self.dragging.is_some() {
                return Err((stack, Refusal::HandsFull));
            }
            self.right = Some(stack);
            self.both = true;
            return Ok(());
        }
        let slot = match hand {
            Hand::Left => &mut self.left,
            Hand::Right => &mut self.right,
        };
        if slot.is_some() || self.both || self.dragging.is_some() {
            return Err((stack, Refusal::HandsFull));
        }
        *slot = Some(stack);
        Ok(())
    }

    /// Lets go of what a hand holds (both arms' load from either).
    pub fn release(&mut self, hand: Hand) -> Option<Stack> {
        if self.both {
            self.both = false;
            return self.right.take();
        }
        match hand {
            Hand::Left => self.left.take(),
            Hand::Right => self.right.take(),
        }
    }

    /// Starts dragging something heavy (both hands on it, or a cord).
    #[allow(clippy::result_large_err)]
    pub fn drag(
        &mut self,
        items: &Items,
        stack: Stack,
        body_kg: f32,
    ) -> Result<(), (Stack, Refusal)> {
        if stack.kind(items).is_none() {
            return Err((stack, Refusal::Unknown));
        }
        if stack.mass(items) > DRAG_SHARE * body_kg {
            return Err((stack, Refusal::Immovable));
        }
        if self.left.is_some() || self.right.is_some() || self.dragging.is_some() {
            return Err((stack, Refusal::HandsFull));
        }
        self.dragging = Some(stack);
        Ok(())
    }

    /// Puts on a garment (one per layer for each region it covers; one belt; one thing on the
    /// head, the hands, the feet).
    #[allow(clippy::result_large_err)]
    pub fn wear(&mut self, items: &Items, stack: Stack) -> Result<(), (Stack, Refusal)> {
        let Some(wear) = stack.kind(items).and_then(|k| k.wear.clone()) else {
            return Err((stack, Refusal::Wrong));
        };
        let clash = self.worn.iter().any(|w| {
            w.stack
                .kind(items)
                .and_then(|k| k.wear.as_ref())
                .is_some_and(|o| {
                    o.layer == wear.layer
                        && (matches!(wear.layer, ClothingLayer::Belt | ClothingLayer::Back)
                            || o.regions.iter().any(|r| wear.regions.contains(r)))
                })
        });
        if clash {
            return Err((stack, Refusal::Taken));
        }
        let points = wear.attachments.len();
        self.worn.push(Worn {
            stack,
            hung: vec![None; points],
        });
        Ok(())
    }

    /// Takes off a worn garment, with what hangs from it (each comes back).
    pub fn take_off(&mut self, i: usize) -> Option<(Stack, Vec<Stack>)> {
        if i >= self.worn.len() {
            return None;
        }
        let w = self.worn.remove(i);
        Some((w.stack, w.hung.into_iter().flatten().collect()))
    }

    /// Hangs something from a worn garment's attachment point.
    #[allow(clippy::result_large_err)]
    pub fn hang(
        &mut self,
        items: &Items,
        stack: Stack,
        worn: usize,
        point: usize,
    ) -> Result<(), (Stack, Refusal)> {
        let Some(kind) = stack.kind(items) else {
            return Err((stack, Refusal::Unknown));
        };
        let Some(w) = self.worn.get(worn) else {
            return Err((stack, Refusal::Wrong));
        };
        let Some(kind_of_point) = w
            .stack
            .kind(items)
            .and_then(|k| k.wear.as_ref())
            .and_then(|wear| wear.attachments.get(point))
        else {
            return Err((stack, Refusal::Wrong));
        };
        if !kind.hangs_from(kind_of_point) {
            return Err((stack, Refusal::Wrong));
        }
        let slot = &mut self.worn[worn].hung[point];
        if slot.is_some() {
            return Err((stack, Refusal::Taken));
        }
        *slot = Some(stack);
        Ok(())
    }

    /// Takes down what hangs from a point.
    pub fn unhang(&mut self, worn: usize, point: usize) -> Option<Stack> {
        self.worn.get_mut(worn)?.hung.get_mut(point)?.take()
    }

    /// Puts something away where it goes: into a container on the back or hanging, onto a
    /// free attachment point it hangs from, or into a free hand. Gives back what will not go.
    pub fn stow(&mut self, items: &Items, stack: Stack, body_kg: f32) -> Result<(), Stack> {
        let mut stack = stack;
        // Into the containers carried.
        let mut containers: Vec<&mut Stack> = Vec::new();
        if let Some(b) = &mut self.back {
            containers.push(b);
        }
        for w in &mut self.worn {
            for h in w.hung.iter_mut().flatten() {
                containers.push(h);
            }
        }
        for c in containers {
            let Some(spec) = c.kind(items).and_then(|k| k.container) else {
                continue;
            };
            let Some(inside) = c.contents_mut(items) else {
                continue;
            };
            match inside.put_anywhere(items, &spec, stack) {
                Ok(()) => return Ok(()),
                Err(rest) => stack = rest,
            }
        }
        // Onto a free attachment point.
        for wi in 0..self.worn.len() {
            for pi in 0..self.worn[wi].hung.len() {
                if self.worn[wi].hung[pi].is_none() {
                    match self.hang(items, stack, wi, pi) {
                        Ok(()) => return Ok(()),
                        Err((s, _)) => stack = s,
                    }
                }
            }
        }
        // Onto a handful of the same (grain into the hand that holds grain).
        if let Some(kind) = stack.kind(items) {
            let per = kind.per_cell();
            for held in [&mut self.right, &mut self.left].into_iter().flatten() {
                if per > 1
                    && held.joins(&stack)
                    && held.count + stack.count <= per
                    && held.mass(items) + stack.mass(items) <= ONE_HAND_KG
                {
                    held.absorb(stack);
                    return Ok(());
                }
            }
        }
        // Into a hand.
        for hand in [Hand::Right, Hand::Left] {
            match self.hold(items, stack, hand, body_kg) {
                Ok(()) => return Ok(()),
                Err((s, _)) => stack = s,
            }
        }
        Err(stack)
    }

    /// Everything carried, worn and dragged, as stacks.
    pub fn into_stacks(self) -> Vec<Stack> {
        let mut out: Vec<Stack> = Vec::new();
        out.extend(self.left);
        out.extend(self.right);
        out.extend(self.back);
        out.extend(self.dragging);
        for w in self.worn {
            out.push(w.stack);
            out.extend(w.hung.into_iter().flatten());
        }
        out
    }

    /// The garments worn (their content ids), for the body's warmth.
    pub fn garments(&self, items: &Items) -> Vec<String> {
        self.worn
            .iter()
            .filter_map(|w| {
                w.stack
                    .kind(items)?
                    .wear
                    .as_ref()
                    .map(|g| g.garment.clone())
            })
            .collect()
    }
}

/// Where a carried thing starts: a hand, the back, a worn garment, an attachment point, or
/// what is dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Root {
    Hand(Hand),
    Back,
    Worn(usize),
    /// A worn garment's attachment point.
    Hung(usize, usize),
    Dragging,
}

/// Where a carried thing is: its root, then the places in containers within containers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub root: Root,
    #[serde(default)]
    pub inside: Vec<usize>,
}

impl Path {
    pub fn at(root: Root) -> Self {
        Self {
            root,
            inside: Vec::new(),
        }
    }

    /// The `i`th thing inside the container here.
    pub fn inner(&self, i: usize) -> Self {
        let mut p = self.clone();
        p.inside.push(i);
        p
    }
}

/// Where a thing goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// A hand, the back, a worn place or an attachment point (wearing, for `Worn`).
    Root(Root),
    /// A cell of the container at `container`.
    Cell {
        container: Path,
        x: u8,
        y: u8,
        turned: bool,
    },
    /// Wherever it goes in the containers carried, else a free hand.
    Stow,
}

impl Carry {
    fn root(&self, r: Root) -> Option<&Stack> {
        match r {
            Root::Hand(Hand::Left) => self.left.as_ref(),
            Root::Hand(Hand::Right) => self.right.as_ref(),
            Root::Back => self.back.as_ref(),
            Root::Worn(i) => self.worn.get(i).map(|w| &w.stack),
            Root::Hung(i, p) => self.worn.get(i)?.hung.get(p)?.as_ref(),
            Root::Dragging => self.dragging.as_ref(),
        }
    }

    fn root_mut(&mut self, r: Root) -> Option<&mut Stack> {
        match r {
            Root::Hand(Hand::Left) => self.left.as_mut(),
            Root::Hand(Hand::Right) => self.right.as_mut(),
            Root::Back => self.back.as_mut(),
            Root::Worn(i) => self.worn.get_mut(i).map(|w| &mut w.stack),
            Root::Hung(i, p) => self.worn.get_mut(i)?.hung.get_mut(p)?.as_mut(),
            Root::Dragging => self.dragging.as_mut(),
        }
    }

    /// The thing at a path.
    pub fn get(&self, p: &Path) -> Option<&Stack> {
        let mut s = self.root(p.root)?;
        for &i in &p.inside {
            s = &s.contents()?.items.get(i)?.stack;
        }
        Some(s)
    }

    /// The thing at a path, to change it (wear an edge, let it go off).
    pub fn get_mut(&mut self, p: &Path) -> Option<&mut Stack> {
        let mut s = self.root_mut(p.root)?;
        for &i in &p.inside {
            s = &mut s.inside.as_mut()?.items.get_mut(i)?.stack;
        }
        Some(s)
    }

    /// Every carried thing, in containers too (garments worn included).
    pub fn for_each_mut(&mut self, f: &mut dyn FnMut(&mut Stack)) {
        fn walk(s: &mut Stack, f: &mut dyn FnMut(&mut Stack)) {
            f(s);
            if let Some(c) = s.inside.as_mut() {
                for p in &mut c.items {
                    walk(&mut p.stack, f);
                }
            }
        }
        for s in [
            &mut self.left,
            &mut self.right,
            &mut self.back,
            &mut self.dragging,
        ]
        .into_iter()
        .flatten()
        {
            walk(s, f);
        }
        for w in &mut self.worn {
            walk(&mut w.stack, f);
            for s in w.hung.iter_mut().flatten() {
                walk(s, f);
            }
        }
    }

    /// Where the first carried thing that `wanted` (in a container too, the hands first, then
    /// the back, what is worn and what hangs from it) is.
    pub fn find(&self, wanted: &dyn Fn(&Stack) -> bool) -> Option<Path> {
        fn walk(s: &Stack, at: Path, wanted: &dyn Fn(&Stack) -> bool) -> Option<Path> {
            if wanted(s) {
                return Some(at);
            }
            let c = s.contents()?;
            c.items
                .iter()
                .enumerate()
                .find_map(|(i, p)| walk(&p.stack, at.inner(i), wanted))
        }
        let mut roots = vec![Root::Hand(Hand::Right), Root::Hand(Hand::Left), Root::Back];
        for (i, w) in self.worn.iter().enumerate() {
            roots.push(Root::Worn(i));
            roots.extend((0..w.hung.len()).map(|p| Root::Hung(i, p)));
        }
        roots
            .into_iter()
            .find_map(|r| walk(self.root(r)?, Path::at(r), wanted))
    }

    /// Drops every carried thing (in containers too) for which `keep` is false: an ember gone
    /// cold, a rotted carcass.
    pub fn retain(&mut self, keep: &mut dyn FnMut(&Stack) -> bool) {
        fn inner(s: &mut Stack, keep: &mut dyn FnMut(&Stack) -> bool) {
            if let Some(c) = s.inside.as_mut() {
                c.items.retain(|p| keep(&p.stack));
                for p in &mut c.items {
                    inner(&mut p.stack, keep);
                }
            }
        }
        for slot in [
            &mut self.left,
            &mut self.right,
            &mut self.back,
            &mut self.dragging,
        ] {
            if slot.as_ref().is_some_and(|s| !keep(s)) {
                *slot = None;
            }
            if let Some(s) = slot.as_mut() {
                inner(s, keep);
            }
        }
        if self.right.is_none() {
            self.both = false;
        }
        for w in &mut self.worn {
            for h in &mut w.hung {
                if h.as_ref().is_some_and(|s| !keep(s)) {
                    *h = None;
                }
                if let Some(s) = h.as_mut() {
                    inner(s, keep);
                }
            }
            inner(&mut w.stack, keep);
        }
    }

    /// The container at a path, opened, with its spec.
    pub fn container_mut(
        &mut self,
        items: &Items,
        p: &Path,
    ) -> Option<(
        hearth_content::schema::item::ContainerSpec,
        &mut crate::Container,
    )> {
        let mut s = self.root_mut(p.root)?;
        for &i in &p.inside {
            s = &mut s.inside.as_mut()?.items.get_mut(i)?.stack;
        }
        let spec = s.kind(items)?.container?;
        Some((spec, s.contents_mut(items)?))
    }

    /// Takes the thing at a path (or `count` of a stack).
    pub fn take(&mut self, items: &Items, p: &Path, count: Option<u16>) -> Option<Stack> {
        if let Some((&last, outer)) = p.inside.split_last() {
            let parent = Path {
                root: p.root,
                inside: outer.to_vec(),
            };
            let (_, c) = self.container_mut(items, &parent)?;
            return match count {
                Some(n) => c.take_some(last, n),
                None => c.take(last),
            };
        }
        if let Some(n) = count {
            // Part of a stack held, hanging or on the back.
            let s = self.root_mut(p.root)?;
            if n < s.count {
                s.count -= n;
                let mut part = s.clone();
                part.count = n;
                part.inside = None;
                return Some(part);
            }
        }
        match p.root {
            Root::Hand(h) => self.release(h),
            Root::Back => self.back.take(),
            Root::Worn(i) => {
                // A garment comes off only with nothing hanging from it.
                if self.worn.get(i)?.hung.iter().any(Option::is_some) {
                    return None;
                }
                self.take_off(i).map(|(s, _)| s)
            }
            Root::Hung(i, pt) => self.unhang(i, pt),
            Root::Dragging => self.dragging.take(),
        }
    }

    /// Puts a thing at a target; gives it back if it will not go.
    pub fn put(
        &mut self,
        items: &Items,
        stack: Stack,
        to: &Target,
        body_kg: f32,
    ) -> Result<(), Stack> {
        match to {
            Target::Root(Root::Hand(h)) => self.hold(items, stack, *h, body_kg).map_err(|(s, _)| s),
            Target::Root(Root::Back) => {
                let back = stack
                    .kind(items)
                    .and_then(|k| k.container)
                    .is_some_and(|c| c.back);
                if self.back.is_some() || !back {
                    return Err(stack);
                }
                self.back = Some(stack);
                Ok(())
            }
            Target::Root(Root::Worn(_)) => self.wear(items, stack).map_err(|(s, _)| s),
            Target::Root(Root::Hung(i, p)) => self.hang(items, stack, *i, *p).map_err(|(s, _)| s),
            Target::Root(Root::Dragging) => self.drag(items, stack, body_kg).map_err(|(s, _)| s),
            Target::Cell {
                container,
                x,
                y,
                turned,
            } => {
                let Some((spec, c)) = self.container_mut(items, container) else {
                    return Err(stack);
                };
                c.put(items, &spec, stack, *x, *y, *turned)
                    .map_err(|(s, _)| s)
            }
            Target::Stow => self.stow(items, stack, body_kg),
        }
    }

    /// Moves a thing (or `count` of a stack) from a path to a target: all of it or nothing.
    pub fn shift(
        &mut self,
        items: &Items,
        from: &Path,
        count: Option<u16>,
        to: &Target,
        body_kg: f32,
    ) -> bool {
        // A container cannot go into itself or into what it holds.
        if let Target::Cell { container, .. } = to
            && container.root == from.root
            && container.inside.starts_with(&from.inside)
        {
            return false;
        }
        let before = self.clone();
        let Some(stack) = self.take(items, from, count) else {
            return false;
        };
        if self.put(items, stack, to, body_kg).is_ok() {
            return true;
        }
        // All or nothing: as it was.
        *self = before;
        false
    }
}
