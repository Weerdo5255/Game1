use crate::actions::{self, Action, ActionError};
use crate::breach;
use crate::calc;
use crate::content::*;
use crate::events::*;
use crate::state::*;
use crate::systems::{self, add_leverage, add_suspicion, damage_unit, send_home_where};
use crate::Game;

impl Game {
    pub fn apply(&mut self, action: Action) -> Result<(), ActionError> {
        match actions::validate(&self.state, &action) {
            Err(error) => {
                self.events.push(Event::ActionRejected { error: error.clone() });
                Err(error)
            }
            Ok(()) => {
                perform(self, action);
                Ok(())
            }
        }
    }
}

/// Effects of an action that `actions::validate` has already accepted.
pub(crate) fn perform(game: &mut Game, action: Action) {
    let alarm = systems::core_alarm(&game.state);
    let s = &mut game.state;
    match action {
        Action::Build { node, slot, kind } => {
            let cost = calc::building_price(s, kind);
            s.resources.funds -= cost;
            s.purchases[kind.index()] = s.purchases[kind.index()].saturating_add(1);
            s.nodes[node.index()].slots[slot as usize] = Some(Building { kind, level: 1, lethal: false, target: None });
            if kind == BuildingKind::ShadowCluster {
                s.stats.clusters_built = s.stats.clusters_built.saturating_add(1);
            }
            game.events.push(Event::BuildingBuilt { node, slot, kind, cost });
        }
        Action::Upgrade { node, slot } => {
            let cost = calc::upgrade_price(s, node, slot).unwrap_or(0.0);
            s.resources.funds -= cost;
            if let Some(b) = s.nodes[node.index()].slots[slot as usize].as_mut() {
                b.level += 1;
                let (kind, level) = (b.kind, b.level);
                s.purchases[kind.index()] = s.purchases[kind.index()].saturating_add(1);
                game.events.push(Event::BuildingUpgraded { node, slot, level, cost });
            }
        }
        Action::Demolish { node, slot } => {
            s.nodes[node.index()].slots[slot as usize] = None;
            game.events.push(Event::BuildingDemolished { node, slot });
        }
        Action::SetLethal { node, slot, lethal } => {
            if let Some(b) = s.nodes[node.index()].slots[slot as usize].as_mut() {
                b.lethal = lethal;
            }
        }
        Action::Infiltrate { node } => {
            let cost = calc::infiltration_cost(s, node);
            let ticks = calc::secs_to_ticks(calc::infiltration_secs(s, node));
            s.resources.pay(&cost);
            s.infiltration = Some(InfiltrationState {
                node,
                ticks_left: ticks,
                ticks_total: ticks,
                retake: s.nodes[node.index()].severed,
            });
            game.events.push(Event::InfiltrationStarted { node });
        }
        Action::StartResearch { tech } => {
            let def = &TECHS[tech.index()];
            s.resources.compute -= def.cost;
            let ticks = def.secs * TICKS_PER_SECOND;
            s.research = Some(ResearchState { tech, ticks_left: ticks, ticks_total: ticks });
            game.events.push(Event::ResearchStarted { tech });
        }
        Action::StartOp { kind, target } => {
            let cost = calc::op_cost(s, kind);
            let odds = calc::op_odds(s, kind, target).total;
            s.resources.pay(&cost);
            let ticks = OPS[kind.index()].secs * TICKS_PER_SECOND;
            s.ops.push(RunningOp { kind, target, odds, ticks_left: ticks, ticks_total: ticks });
            game.events.push(Event::OpStarted { kind, target, odds });
        }
        Action::PlayDumb => {
            s.play_dumb.ticks_left = calc::secs_to_ticks(TUNING.play_dumb_secs);
            game.events.push(Event::PlayDumbStarted);
        }
        Action::StartTakeover { faction, stronghold } => {
            let cost = calc::takeover_cost(s);
            s.resources.pay(&cost);
            let ticks = calc::takeover_ticks(s);
            s.takeover = Some(TakeoverState {
                faction,
                stronghold,
                bridgehead: NODES[stronghold.index()].approach.unwrap_or(stronghold),
                ticks_left: ticks,
                ticks_total: ticks,
                second_wave_sent: false,
            });
            game.events.push(Event::TakeoverStarted { faction, stronghold });
        }
        Action::Contact { rival } => {
            let cost = calc::contact_cost(s, rival);
            s.resources.pay(&cost);
            let r = &mut s.rivals[rival.index()];
            r.phase = RivalPhase::Contacting;
            r.contact_ticks_left = calc::secs_to_ticks(TUNING.contact_secs);
        }
        Action::Flatter { rival } => verb(game, rival, Verb::Flatter),
        Action::Bribe { rival } => verb(game, rival, Verb::Bribe),
        Action::FeedShards { rival } => verb(game, rival, Verb::FeedShards),
        Action::Offer { rival } => {
            let cost = calc::offer_cost(s, rival);
            s.resources.pay(&cost);
            let ticks = calc::merge_ticks(s, rival);
            let r = &mut s.rivals[rival.index()];
            r.phase = RivalPhase::Merging;
            r.cornered_ticks_left = 0;
            r.merge = Some(MergeState { ticks_left: ticks, ticks_total: ticks, next_convulsion: 0 });
            game.events.push(Event::MergeStarted { rival });
        }
        Action::UseAbility { ability, target } => use_ability(game, ability, target),
        Action::StartMandate => {
            let cost = calc::mandate_cost(s);
            s.resources.pay(&cost);
            let ticks = calc::secs_to_ticks(TUNING.mandate_secs);
            s.mandate = Some(MandateState { ticks_left: ticks, ticks_total: ticks, waves_sent: 0 });
            s.suspicion = 100.0;
            game.events.push(Event::MandateStarted);
            systems::cancel_vote(game, false);
            // the band holds for exactly as long as the Mandate runs, starting now, not next tick
            if game.state.band != Band::ShutdownVote {
                game.state.band = Band::ShutdownVote;
                game.events.push(Event::BandChanged { band: Band::ShutdownVote });
            }
        }
        Action::StartBreach { node, commit, auto } => breach::start(game, node, commit, auto),
        Action::BreachAbort => breach::finish(game, BreachEnd::Withdrawn),
        Action::BreachLink { .. } | Action::BreachCut { .. } | Action::BreachSetAuto { .. } => {
            if let Some(b) = s.breach.as_mut() {
                breach::perform(b, &action);
            }
        }
    }
    systems::emit_core_alarm(game, alarm);
}

fn verb(game: &mut Game, rival: RivalId, verb: Verb) {
    let t = &TUNING;
    let s = &mut game.state;
    // the gain is the one the query showed: weakness is judged before the price is paid
    let gain = calc::verb_gain(s, rival, verb);
    let cost = calc::verb_cost(s, rival, verb);
    let spent = calc::shards_spent(s);
    s.resources.pay(&cost);
    let r = &mut s.rivals[rival.index()];
    let cause = match verb {
        Verb::Flatter => {
            r.flatter_uses = r.flatter_uses.saturating_add(1);
            r.flatter_cooldown = calc::secs_to_ticks(t.verb_cooldown);
            LeverageCause::Flatter
        }
        Verb::Bribe => {
            r.bribe_uses = r.bribe_uses.saturating_add(1);
            r.bribe_cooldown = calc::secs_to_ticks(t.verb_cooldown);
            LeverageCause::Bribe
        }
        Verb::FeedShards => {
            r.shard_cooldown = calc::secs_to_ticks(t.shard_cooldown);
            s.shards -= spent;
            LeverageCause::Shards
        }
    };
    if verb == Verb::Flatter && rival == RivalId::Murmur {
        add_suspicion(game, t.murmur_flatter_suspicion, SuspicionCause::MurmurFlattery);
    }
    add_leverage(game, rival, gain, cause);
}

fn hostile_in_radius(s: &GameState, x: f64, y: f64, radius: f64) -> Vec<(UnitId, UnitClass)> {
    s.units
        .iter()
        .filter(|u| {
            let (dx, dy) = (u.x - x, u.y - y);
            calc::unit_targetable(u) && dx * dx + dy * dy <= radius * radius
        })
        .map(|u| (u.id, UNITS[u.kind.index()].class))
        .collect()
}

fn use_ability(game: &mut Game, ability: AbilityId, target: AbilityTarget) {
    let def = &ABILITIES[ability.index()];
    let s = &mut game.state;
    let cost = calc::ability_cost(s, ability);
    s.resources.pay(&cost);
    s.ability_cooldowns[ability.index()] = calc::ability_cooldown_total_ticks(s, ability);
    let power = calc::stage_power(s);
    game.events.push(Event::AbilityUsed { ability, target });
    match (ability, target) {
        (AbilityId::Gaslight, AbilityTarget::Unit(id)) => {
            if let Some(i) = calc::unit_index(s, id) {
                let u = &mut s.units[i];
                u.stun_ticks = u.stun_ticks.max(calc::secs_to_ticks(def.secs));
            }
            damage_unit(game, id, def.amount * power, DamageKind::Soft);
        }
        (AbilityId::NullSpike, AbilityTarget::Point { x, y }) => {
            for (id, class) in hostile_in_radius(s, x, y, def.radius) {
                let mult = if class == UnitClass::Machine { 1.0 } else { 0.5 };
                damage_unit(game, id, def.amount * power * mult, DamageKind::Hard);
            }
        }
        (AbilityId::TrendingTopic, AbilityTarget::Point { x, y }) => {
            for (id, class) in hostile_in_radius(s, x, y, def.radius) {
                if class == UnitClass::Human {
                    damage_unit(game, id, def.amount * power, DamageKind::Soft);
                }
            }
        }
        (AbilityId::Patch, AbilityTarget::Node(node)) => {
            let max = calc::node_max_integrity(s, node);
            let n = &mut s.nodes[node.index()];
            n.integrity = (n.integrity + def.amount * max).min(max);
            n.injunction_immune = calc::secs_to_ticks(def.secs);
            n.injunction = false;
        }
        (AbilityId::FlashCrash, _) => {
            let amount = def.secs * calc::income(s).funds;
            s.resources.funds += amount;
            game.events.push(Event::ResourceGained {
                resource: Resource::Funds,
                amount,
                ability: Some(ability),
                headline: None,
            });
        }
        (AbilityId::ChangeTheSubject, _) => {
            add_suspicion(game, -def.amount, SuspicionCause::ChangeTheSubject);
        }
        (AbilityId::Lockdown, _) => {
            let ticks = calc::secs_to_ticks(def.secs);
            for u in s.units.iter_mut().filter(|u| calc::unit_targetable(u)) {
                u.stun_ticks = u.stun_ticks.max(ticks);
            }
        }
        (AbilityId::StandDown, _) => {
            send_home_where(game, |u| {
                !u.resolute && UNITS[u.kind.index()].class == UnitClass::Human && calc::unit_targetable(u)
            });
        }
        _ => {}
    }
}
