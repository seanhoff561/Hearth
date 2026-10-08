//! Creative's inventory (Amendment P §3.1): everything the world is made of, by category, to
//! place, summon or know — every natural material and rock, every plant and animal species, every
//! item of every form and material, every building piece and workstation, every piece of
//! knowledge — searched by name. No people (Amendment E §9.1).

use hearth_content::Content;

/// The inventory's categories, in its tabs' order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Terrain,
    Plants,
    Animals,
    Items,
    Building,
    Knowledge,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Terrain,
        Category::Plants,
        Category::Animals,
        Category::Items,
        Category::Building,
        Category::Knowledge,
    ];

    /// The tab's word key.
    pub fn key(self) -> &'static str {
        match self {
            Category::Terrain => "creative.terrain",
            Category::Plants => "creative.plants",
            Category::Animals => "creative.animals",
            Category::Items => "creative.items",
            Category::Building => "creative.building",
            Category::Knowledge => "creative.knowledge",
        }
    }
}

/// One thing of the inventory: what it is (its id in its own table) and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub category: Category,
    pub id: String,
    pub name: String,
}

/// Every thing of the inventory, by category and then name.
pub fn catalog(content: &Content) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut add = |category, id: &str, name: &str| {
        out.push(Entry {
            category,
            id: id.to_owned(),
            name: name.to_owned(),
        })
    };
    for b in hearth_content::generate::natural_blocks(content) {
        add(Category::Terrain, &b.id, &b.name);
    }
    for p in content.plants.iter() {
        add(Category::Plants, &p.id, &p.name);
    }
    for a in content.animals.iter() {
        add(Category::Animals, &a.id, &a.name);
    }
    // Every item of every form and material (the registry the world's items come from).
    for i in hearth_items::Items::from_content(content).iter() {
        add(Category::Items, &i.id, &i.name);
    }
    // Each piece in each material it may be made of (its block), named as the piece is.
    for b in hearth_content::building::piece_blocks(
        &content.construction,
        &content.materials,
        &content.forms,
    ) {
        let piece = content.construction.get(b.piece.as_str());
        let material = content.materials.get(b.material.as_str());
        let (Some(piece), Some(material)) = (piece, material) else {
            continue;
        };
        let name = if piece.name.contains("{material}") {
            piece
                .name
                .replace("{material}", &material.name.to_lowercase())
        } else {
            format!("{} ({})", piece.name, material.name.to_lowercase())
        };
        add(Category::Building, &b.id, name.trim());
    }
    for w in content.workstations.iter() {
        add(Category::Building, &w.id, &w.name);
    }
    for k in content.knowledge.iter() {
        add(Category::Knowledge, &k.id, &k.name);
    }
    let order = |c: Category| Category::ALL.iter().position(|x| *x == c).unwrap_or(0);
    out.sort_by(|a, b| {
        order(a.category)
            .cmp(&order(b.category))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// The things of a category whose names hold every word of `query` (any case).
pub fn search<'a>(all: &'a [Entry], category: Category, query: &str) -> Vec<&'a Entry> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    all.iter()
        .filter(|e| e.category == category)
        .filter(|e| {
            let name = e.name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_holds_its_things_and_is_searched_by_name() {
        let content = Content::load_base();
        let all = catalog(&content);
        for c in Category::ALL {
            let n = all.iter().filter(|e| e.category == c).count();
            assert!(n > 0, "{c:?} is empty");
        }
        let count = |c| all.iter().filter(|e| e.category == c).count();
        assert_eq!(count(Category::Plants), content.plants.iter().count());
        assert_eq!(count(Category::Animals), content.animals.iter().count());
        assert_eq!(
            count(Category::Items),
            hearth_items::Items::from_content(&content).iter().count()
        );
        let deer = search(&all, Category::Animals, "red DEER");
        assert!(deer.iter().any(|e| e.id.ends_with("red_deer")), "{deer:?}");
        assert!(search(&all, Category::Animals, "no such beast").is_empty());
        let flint = search(&all, Category::Items, "flint");
        assert!(flint.len() > 1, "flint in many forms");
    }
}
