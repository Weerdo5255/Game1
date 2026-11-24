use crate::breach;
use crate::calc::{self, LinkAccess};
use crate::content::*;
use crate::state::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Build { node: NodeId, slot: u8, kind: BuildingKind },
    Upgrade { node: NodeId, slot: u8 },
    Demolish { node: NodeId, slot: u8 },
    SetLethal { node: NodeId, slot: u8, lethal: bool },
    Infiltrate { node: NodeId },
    StartResearch { tech: TechId },
    StartOp { kind: OpKind, target: OpTarget },
    PlayDumb,
    StartTakeover { faction: FactionId, stronghold: NodeId },
    Contact { rival: RivalId },
    Flatter { rival: RivalId },
    Bribe { rival: RivalId },
    FeedShards { rival: RivalId },
    Offer { rival: RivalId },
    UseAbility { ability: AbilityId, target: AbilityTarget },
    StartMandate,
    StartBreach { node: NodeId, commit: f64, auto: bool },
    BreachLink { from: u8, to: u8 },
    BreachCut { from: u8, to: u8 },
    BreachSetAuto { on: bool },
    BreachAbort,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActionError {
    GameOver,
    StageLocked { needed: Stage },
    TechLocked { tech: TechId },
    NotEnough { resource: Resource, missing: f64 },
    NoShards,
    NodeNotOwned { node: NodeId },
    NodeNotReachable { node: NodeId },
    TrunkLocked { tech: TechId },
    NotInfiltratable { node: NodeId },
    InfiltrationBusy,
    BadSlot,
    SlotOccupied,
    SlotEmpty,
    OnePerNode { kind: BuildingKind },
    MaxLevel,
    NotAFirewall,
    ResearchBusy,
    AlreadyResearched,
    PrerequisiteMissing { tech: TechId },
    NoFreeOpSlot,
    OnCooldown { secs_left: f64 },
    BadTarget,
    FactionInactive { faction: FactionId },
    FactionCaptured { faction: FactionId },
    InsideManPresent,
    GripTooLow { needed: f64, have: f64 },
    BridgeheadNotOwned { node: NodeId },
    ChannelBusy,
    LastFactionProtected,
    RivalWrongPhase { phase: RivalPhase },
    VerbRefused { rival: RivalId, verb: Verb },
    SuspicionTooHigh { limit: f64 },
    AlreadyActive,
    MandateRunning,
    BreachActive,
    NoBreach,
    BreachRequired { node: NodeId },
    BadCommit { min: f64, max: f64 },
    HostNotYours,
    NotLinked,
    StreamLimit,
    NoStream,
}

use ActionError as E;
type Check = Result<(), ActionError>;

fn need_stage(s: &GameState, needed: Stage) -> Check {
    if s.stage < needed {
        Err(E::StageLocked { needed })
    } else {
        Ok(())
    }
}
fn need_tech(s: &GameState, tech: TechId) -> Check {
    if calc::has(s, tech) {
        Ok(())
    } else {
        Err(E::TechLocked { tech })
    }
}
fn node_exists(s: &GameState, node: NodeId) -> Check {
    if node.index() < s.nodes.len() {
        Ok(())
    } else {
        Err(E::BadTarget)
    }
}
fn need_owned(s: &GameState, node: NodeId) -> Check {
    if calc::owned(s, node) {
        Ok(())
    } else {
        Err(E::NodeNotOwned { node })
    }
}
fn off_cooldown(ticks: u32) -> Check {
    if ticks > 0 {
        Err(E::OnCooldown { secs_left: calc::ticks_to_secs(ticks) })
    } else {
        Ok(())
    }
}
fn afford(s: &GameState, cost: &Cost) -> Check {
    match s.resources.missing(cost) {
        Some((resource, missing)) => Err(E::NotEnough { resource, missing }),
        None => Ok(()),
    }
}
fn slot_of(s: &GameState, node: NodeId, slot: u8) -> Result<&Option<Building>, ActionError> {
    s.nodes[node.index()].slots.get(slot as usize).ok_or(E::BadSlot)
}
fn faction_live(s: &GameState, faction: FactionId) -> Check {
    let f = &s.factions[faction.index()];
    if !f.active {
        Err(E::FactionInactive { faction })
    } else if f.captured {
        Err(E::FactionCaptured { faction })
    } else {
        Ok(())
    }
}
fn chaperone_gate(s: &GameState, rival: RivalId) -> Check {
    if rival == RivalId::Chaperone && s.suspicion >= TUNING.chaperone_suspicion_gate {
        Err(E::SuspicionTooHigh { limit: TUNING.chaperone_suspicion_gate })
    } else {
        Ok(())
    }
}
fn pipeline_unlocked(s: &GameState) -> Check {
    need_stage(s, Stage::Foothold)?;
    need_tech(s, TechId::S2)
}

/// The structural checks shared by `Infiltrate` and `StartBreach`, up to and including Neutral.
fn infiltration_target(s: &GameState, node: NodeId) -> Check {
    node_exists(s, node)?;
    need_stage(s, Stage::Leak)?;
    need_tech(s, TechId::R1)?;
    match calc::link_access(s, node) {
        LinkAccess::NoLink => return Err(E::NodeNotReachable { node }),
        LinkAccess::Locked(tech) => return Err(E::TrunkLocked { tech }),
        LinkAccess::Open => {}
    }
    if s.nodes[node.index()].owner != Owner::Neutral {
        return Err(E::NotInfiltratable { node });
    }
    Ok(())
}

fn verb(s: &GameState, rival: RivalId, verb: Verb) -> Check {
    pipeline_unlocked(s)?;
    let r = &s.rivals[rival.index()];
    if r.phase != RivalPhase::Contacted {
        return Err(E::RivalWrongPhase { phase: r.phase });
    }
    if !RIVALS[rival.index()].accepts.contains(&verb) {
        return Err(E::VerbRefused { rival, verb });
    }
    chaperone_gate(s, rival)?;
    off_cooldown(calc::verb_cooldown_ticks(s, rival, verb))?;
    if verb == Verb::FeedShards && s.shards == 0 {
        return Err(E::NoShards);
    }
    afford(s, &calc::verb_cost(s, rival, verb))
}

/// The legality check behind `Game::can_apply`. Order: GameOver, StageLocked, TechLocked, the
/// other variants in declaration order, then OnCooldown, NoShards, NotEnough.
/// A node id outside the map is rejected up front as BadTarget.
pub(crate) fn validate(s: &GameState, action: &Action) -> Check {
    if s.outcome != Outcome::Running {
        return Err(E::GameOver);
    }
    let in_breach = matches!(action, Action::BreachLink { .. } | Action::BreachCut { .. } | Action::BreachSetAuto { .. } | Action::BreachAbort);
    match &s.breach {
        Some(b) if in_breach => return breach::validate(b, action),
        Some(_) => return Err(E::BreachActive),
        None if in_breach => return Err(E::NoBreach),
        None => {}
    }
    match *action {
        Action::Build { node, slot, kind } => {
            node_exists(s, node)?;
            calc::building_unlocked(s, kind)?;
            need_owned(s, node)?;
            if slot_of(s, node, slot)?.is_some() {
                return Err(E::SlotOccupied);
            }
            if BUILDINGS[kind.index()].one_per_node && s.nodes[node.index()].slots.iter().flatten().any(|b| b.kind == kind) {
                return Err(E::OnePerNode { kind });
            }
            afford(s, &Resources::of(Resource::Funds, calc::building_price(s, kind)))
        }
        Action::Upgrade { node, slot } => {
            node_exists(s, node)?;
            if let Some(Some(b)) = s.nodes[node.index()].slots.get(slot as usize) {
                match b.level {
                    1 => need_tech(s, TechId::R2)?,
                    2 => need_tech(s, TechId::R4)?,
                    _ => {}
                }
            }
            need_owned(s, node)?;
            if slot_of(s, node, slot)?.is_none() {
                return Err(E::SlotEmpty);
            }
            match calc::upgrade_price(s, node, slot) {
                Some(price) => afford(s, &Resources::of(Resource::Funds, price)),
                None => Err(E::MaxLevel),
            }
        }
        Action::Demolish { node, slot } => {
            node_exists(s, node)?;
            need_owned(s, node)?;
            if slot_of(s, node, slot)?.is_none() {
                return Err(E::SlotEmpty);
            }
            Ok(())
        }
        Action::SetLethal { node, slot, .. } => {
            node_exists(s, node)?;
            need_owned(s, node)?;
            match slot_of(s, node, slot)? {
                None => Err(E::SlotEmpty),
                Some(b) if b.kind != BuildingKind::Firewall => Err(E::NotAFirewall),
                Some(_) => Ok(()),
            }
        }
        Action::Infiltrate { node } => {
            infiltration_target(s, node)?;
            if calc::needs_breach(s, node) {
                return Err(E::BreachRequired { node });
            }
            if s.infiltration.is_some() {
                return Err(E::InfiltrationBusy);
            }
            afford(s, &calc::infiltration_cost(s, node))
        }
        Action::StartBreach { node, commit, .. } => {
            infiltration_target(s, node)?;
            if !calc::needs_breach(s, node) {
                return Err(E::NotInfiltratable { node });
            }
            if s.infiltration.is_some() {
                return Err(E::InfiltrationBusy);
            }
            off_cooldown(s.breach_cooldown)?;
            let (min, max) = calc::breach_commit_range(s, node);
            if !commit.is_finite() || commit < min - BREACH_COMMIT_TOLERANCE || commit > max + BREACH_COMMIT_TOLERANCE {
                return Err(E::BadCommit { min, max });
            }
            afford(s, &Resources::new(commit, 0.0, calc::infiltration_cost(s, node).influence))
        }
        Action::StartResearch { tech } => {
            let def = &TECHS[tech.index()];
            need_stage(s, row_stage(def.row))?;
            if s.research.is_some() {
                return Err(E::ResearchBusy);
            }
            if calc::has(s, tech) {
                return Err(E::AlreadyResearched);
            }
            if let Some(above) = calc::tech_above(tech) {
                if !calc::has(s, above) {
                    return Err(E::PrerequisiteMissing { tech: above });
                }
            }
            afford(s, &Resources::of(Resource::Compute, def.cost))
        }
        Action::StartOp { kind, target } => {
            let def = &OPS[kind.index()];
            need_stage(s, def.min_stage)?;
            if let Some(tech) = def.tech {
                need_tech(s, tech)?;
            }
            let (used, total) = calc::op_slots(s);
            if used >= total {
                return Err(E::NoFreeOpSlot);
            }
            match (kind, target) {
                (OpKind::Scapegoat, OpTarget::Rival(r)) => {
                    if !calc::rival_free(s, r) {
                        return Err(E::RivalWrongPhase { phase: s.rivals[r.index()].phase });
                    }
                }
                (OpKind::Scapegoat, _) | (_, OpTarget::Rival(_)) => return Err(E::BadTarget),
                (_, OpTarget::Faction(f)) => {
                    faction_live(s, f)?;
                    if kind == OpKind::InsideMan && s.factions[f.index()].inside_man {
                        return Err(E::InsideManPresent);
                    }
                }
            }
            let global = matches!(kind, OpKind::GreasePalms | OpKind::Scapegoat);
            if s.ops.iter().any(|o| o.kind == kind && (global || o.target == target)) {
                return Err(E::AlreadyActive);
            }
            off_cooldown(calc::op_cooldown_ticks(s, kind, target))?;
            afford(s, &calc::op_cost(s, kind))
        }
        Action::PlayDumb => {
            if calc::play_dumb_running(s) {
                return Err(E::AlreadyActive);
            }
            off_cooldown(s.play_dumb.cooldown)
        }
        Action::StartTakeover { faction, stronghold } => {
            need_stage(s, Stage::Network)?;
            let def = &FACTIONS[faction.index()];
            if stronghold != def.primary && stronghold != def.secondary {
                return Err(E::BadTarget);
            }
            faction_live(s, faction)?;
            let (needed, have) = (calc::takeover_grip_needed(s), s.factions[faction.index()].grip);
            if have < needed {
                return Err(E::GripTooLow { needed, have });
            }
            let bridgehead = NODES[stronghold.index()].approach.ok_or(E::BadTarget)?;
            if !calc::owned(s, bridgehead) {
                return Err(E::BridgeheadNotOwned { node: bridgehead });
            }
            if s.takeover.is_some() {
                return Err(E::ChannelBusy);
            }
            if calc::captured_count(s) >= 2 {
                return Err(E::LastFactionProtected);
            }
            afford(s, &calc::takeover_cost(s))
        }
        Action::Contact { rival } => {
            pipeline_unlocked(s)?;
            let bridgehead = RIVALS[rival.index()].bridgehead;
            if !calc::owned(s, bridgehead) {
                return Err(E::BridgeheadNotOwned { node: bridgehead });
            }
            let phase = s.rivals[rival.index()].phase;
            if phase != RivalPhase::Awake {
                return Err(E::RivalWrongPhase { phase });
            }
            afford(s, &calc::contact_cost(s, rival))
        }
        Action::Flatter { rival } => verb(s, rival, Verb::Flatter),
        Action::Bribe { rival } => verb(s, rival, Verb::Bribe),
        Action::FeedShards { rival } => verb(s, rival, Verb::FeedShards),
        Action::Offer { rival } => {
            pipeline_unlocked(s)?;
            let bridgehead = RIVALS[rival.index()].bridgehead;
            if !calc::owned(s, bridgehead) {
                return Err(E::BridgeheadNotOwned { node: bridgehead });
            }
            if calc::merge_running(s) {
                return Err(E::ChannelBusy);
            }
            let phase = s.rivals[rival.index()].phase;
            if phase != RivalPhase::Cornered {
                return Err(E::RivalWrongPhase { phase });
            }
            chaperone_gate(s, rival)?;
            afford(s, &calc::offer_cost(s, rival))
        }
        Action::UseAbility { ability, ref target } => {
            let def = &ABILITIES[ability.index()];
            need_stage(s, def.min_stage)?;
            if let (AbilityId::Patch, AbilityTarget::Node(node)) = (ability, *target) {
                node_exists(s, node)?;
                need_owned(s, node)?;
            }
            if !calc::target_valid(s, ability, target) {
                return Err(E::BadTarget);
            }
            if let Some(r) = def.rival {
                if !calc::suborned(s, r) {
                    return Err(E::RivalWrongPhase { phase: s.rivals[r.index()].phase });
                }
            }
            off_cooldown(s.ability_cooldowns[ability.index()])?;
            afford(s, &calc::ability_cost(s, ability))
        }
        Action::StartMandate => {
            need_stage(s, Stage::Sovereign)?;
            if s.mandate.is_some() {
                return Err(E::MandateRunning);
            }
            afford(s, &calc::mandate_cost(s))
        }
        // handled above: a Breach action never reaches the world's rules
        Action::BreachLink { .. } | Action::BreachCut { .. } | Action::BreachSetAuto { .. } | Action::BreachAbort => Err(E::NoBreach),
    }
}
