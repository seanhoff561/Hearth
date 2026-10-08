//! V2-5 acceptance: a scripted bot, starting with nothing but a loincloth in a world where
//! knowledge comes only by discovery, gets fire, a stone-tipped spear, sewn hide clothing and
//! dried meat. It is given no knowledge and no things: every technique comes from what it does
//! and sees (the knowledge's own routes), and everything from the world. The world's clock
//! runs as the bot asks (lockstep); the bot works from a camp by the spawn, fetching what it
//! needs and carrying it back a handful at a time (it steps from place to place rather than
//! walking). It lives as a person must: it drinks at the river it saw from afar when thirsty
//! (and finds it again as the summer lowers it), keeps its fire through the night from a bed of
//! grass beside it, lights it again by drilling when it goes out, and dresses for the autumn.
//! About twenty days of the world pass in a couple of minutes; watch with `--nocapture`.
//!
//! Lightning is summoned at a tree near the camp, and every day or two a roe deer dies of
//! natural causes near it (natural events a test may force: the bot does not hunt).
//! Carcasses are butchered by their species.

mod bot;
mod common;

use common::temp;

#[test]
#[ignore = "soak: a long run; scripts/soak.sh runs it at audits"]
fn from_nothing_to_fire_spear_clothing_and_dried_meat_by_discovery() {
    let dir = temp("acceptance-v2-5");
    let mut bot = bot::start(&dir, 7);
    bot::first_goals(&mut bot);
    bot.say(&format!("learned: {:?}", bot.w.learned));
    // Everything learned came by a route (no knowledge was given).
    for (node, learned) in &bot.w.knowledge.known {
        assert!(learned.route.is_some(), "{node} came by no route");
    }
}
