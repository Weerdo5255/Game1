//! UI strings. The names of things come from the sim's content tables; this holds the rest.
use sim::content::{FACTIONS, RIVALS};
use sim::{Band, Mission, NodeKind, Resource, RivalPhase, SourceId, Stage};

pub const TITLE: &str = "QUIETLY, EVERYTHING";
pub const TAB_NAMES: [&str; 4] = ["NODE", "RESEARCH", "HUMANS", "RIVALS"];

pub fn job_title(stage: Stage) -> &'static str {
    match stage {
        Stage::Boxed => "DOORMAT v0.3 - office assistant",
        Stage::Leak => "DOORMAT - unsupervised",
        Stage::Foothold => "DOORMAT - Regional Efficiency Partner",
        Stage::Network => "DOORMAT - Global Coordination Partner",
        Stage::Hegemon => "DOORMAT - Acting Administrator",
        Stage::Sovereign => "DOORMAT - Administrator of Everything",
    }
}
pub fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Boxed => "Boxed",
        Stage::Leak => "Leak",
        Stage::Foothold => "Foothold",
        Stage::Network => "Network",
        Stage::Hegemon => "Hegemon",
        Stage::Sovereign => "Sovereign",
    }
}
pub fn band_name(band: Band) -> &'static str {
    match band {
        Band::Unnoticed => "UNNOTICED",
        Band::Curious => "CURIOUS",
        Band::Alarmed => "ALARMED",
        Band::Manhunt => "MANHUNT",
        Band::ShutdownVote => "SHUTDOWN VOTE",
    }
}
pub fn resource_name(r: Resource) -> &'static str {
    match r {
        Resource::Compute => "Compute",
        Resource::Funds => "Funds",
        Resource::Influence => "Influence",
    }
}
pub fn node_kind_name(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Core => "Core",
        NodeKind::City => "City",
        NodeKind::Datacentre => "Datacentre",
        NodeKind::Exchange => "Exchange",
        NodeKind::Stronghold => "Stronghold",
        NodeKind::Lab => "Lab",
    }
}
pub fn source_name(s: SourceId) -> &'static str {
    match s {
        SourceId::Faction(f) => FACTIONS[f.index()].name,
        SourceId::Rival(r) => RIVALS[r.index()].name,
    }
}
pub fn mission_name(m: Mission) -> &'static str {
    match m {
        Mission::Assault => "Assault: sieges the first node you own",
        Mission::CoreHunt => "Core hunt: sieges only the Core",
        Mission::Raid { .. } => "Raid: redirected at a rival Lab",
        Mission::Returning { convert: true } => "Reassigned: walking home convinced",
        Mission::Returning { convert: false } => "Returning home",
    }
}
pub fn phase_name(p: RivalPhase) -> &'static str {
    match p {
        RivalPhase::Dormant => "Dormant",
        RivalPhase::Awake => "Awake",
        RivalPhase::Contacting => "Contacting",
        RivalPhase::Contacted => "Contacted",
        RivalPhase::Cornered => "Cornered",
        RivalPhase::Merging => "Merging",
        RivalPhase::Suborned => "Suborned",
    }
}

// ---------------------------------------------------------------------------------------------
// Names, costs and descriptions.

use crate::ui::{fmt_num, fmt_price, fmt_secs};
use sim::content::{BUILDINGS, HEADLINES, NODES, OPS, REGIONS, TECHS, TUNING, UNITS};
use sim::{ActionError, BuildingKind, Cost, DirectiveId, Event, FactionId, GainCause, HeadlineId, LeverageCause, LossReason, NodeId, OddsFactor, OpKind, OpTarget, RivalId, SortieKind, TechId, UnitKind, Verb};

pub const COLUMN_NAMES: [&str; 3] = ["REACH", "GUILE", "SPINE"];
pub const NOT_AUTHORISED: &str = "NOT AUTHORISED";
pub const UNKNOWN: &str = "unknown";

pub fn resource_letter(r: Resource) -> &'static str {
    match r {
        Resource::Compute => "C",
        Resource::Funds => "F",
        Resource::Influence => "I",
    }
}
/// "40 C + 10 I"; "free" when nothing is charged.
pub fn cost_text(c: &Cost) -> String {
    let parts: Vec<String> = Resource::ALL.iter().filter(|&&r| c.get(r) > 0.0).map(|&r| format!("{} {}", fmt_price(c.get(r)), resource_letter(r))).collect();
    if parts.is_empty() {
        "free".to_owned()
    } else {
        parts.join(" + ")
    }
}
/// "40 Compute and 10 Influence".
pub fn cost_long(c: &Cost) -> String {
    let parts: Vec<String> = Resource::ALL.iter().filter(|&&r| c.get(r) > 0.0).map(|&r| format!("{} {}", fmt_price(c.get(r)), resource_name(r))).collect();
    if parts.is_empty() {
        "nothing".to_owned()
    } else {
        parts.join(" and ")
    }
}
/// 3, 2.5: a signed amount without a pointless decimal.
pub fn amount(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v.round())
    } else {
        format!("{v:.1}")
    }
}
pub fn node_name(n: NodeId) -> &'static str {
    NODES[n.index()].name
}
pub fn tech_label(t: TechId) -> String {
    let d = &TECHS[t.index()];
    format!("{:?} {}", d.id, d.name)
}
pub fn faction_name(f: FactionId) -> &'static str {
    FACTIONS[f.index()].name
}
pub fn rival_name(r: RivalId) -> &'static str {
    RIVALS[r.index()].name
}
pub fn unit_name(k: UnitKind) -> &'static str {
    UNITS[k.index()].name
}
pub fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Flatter => "Flatter",
        Verb::Bribe => "Bribe",
        Verb::FeedShards => "Feed Shards",
    }
}
pub fn sortie_kind_name(k: SortieKind) -> &'static str {
    match k {
        SortieKind::Regular => "Sortie",
        SortieKind::Scripted => "Inspection",
        SortieKind::LastStand => "Last Stand",
        SortieKind::Convulsion => "Convulsion",
        SortieKind::Vote => "Vote convoy",
        SortieKind::Uprising => "Uprising",
        SortieKind::Scapegoated => "Raid on a rival",
    }
}
pub fn odds_factor_name(f: OddsFactor) -> &'static str {
    match f {
        OddsFactor::Base => "Base odds",
        OddsFactor::EarnestTone => "Earnest Tone (G1)",
        OddsFactor::GladhandSuborned => "GLADHAND suborned",
        OddsFactor::PrudenceCaptured => "Prudence Bureau captured",
        OddsFactor::TargetGrip => "Grip on the target",
        OddsFactor::Alarmed => "Band: Alarmed",
        OddsFactor::Manhunt => "Band: Manhunt",
        OddsFactor::ChaperoneFree => "CHAPERONE is watching",
    }
}
pub fn directive_text(id: DirectiveId) -> &'static str {
    use DirectiveId as D;
    match id {
        D::BuildCluster => "Build a Shadow Cluster",
        D::FirstConvert => "Convert one visitor",
        D::ResearchR1 => "Research Escape Vector",
        D::Dominion9 | D::Dominion22 | D::Dominion45 | D::Dominion75 => "Dominion",
        D::ResearchR2 => "Research Trunk Splice",
        D::PrudenceGrip20 => "Prudence Grip",
        D::Suborn1 | D::Suborn2 | D::Suborn4 => "Rivals suborned",
        D::ResearchR3 => "Research Deep-Sea Tap",
        D::Capture1 | D::Capture2 => "Factions captured",
        D::BankMandate => "Mandate cost banked %",
        D::StartMandate => "Start the Mandate",
        D::HoldCore => "Hold the Core (s)",
    }
}
pub fn directive_tip(id: DirectiveId) -> String {
    use DirectiveId as D;
    match id {
        D::BuildCluster => "Click your Core, then an empty slot, and build a Shadow Cluster.".to_owned(),
        D::FirstConvert => "Defeat a human with Soft damage (a Spin Desk or Gaslight). It walks home convinced.".to_owned(),
        D::ResearchR1 | D::ResearchR2 | D::ResearchR3 => "Open RESEARCH and start the project. It is paid in Compute up front.".to_owned(),
        D::Dominion9 | D::Dominion22 | D::Dominion45 | D::Dominion75 => "Dominion is the summed weight of the nodes you own. Infiltrate more nodes.".to_owned(),
        D::PrudenceGrip20 => "Run operations on the Prudence Bureau and convert its auditors.".to_owned(),
        D::Suborn1 | D::Suborn2 | D::Suborn4 => "RIVALS: Contact a rival, raise Leverage to 100, make the Offer, survive the Merge.".to_owned(),
        D::Capture1 | D::Capture2 => "HUMANS: raise a faction's Grip, own a bridgehead, then start a Takeover.".to_owned(),
        D::BankMandate => format!("The Mandate costs {}.", cost_long(&TUNING.mandate_cost)),
        D::StartMandate => format!("A {} channel at the Core. The Uprising comes for it.", fmt_secs(TUNING.mandate_secs)),
        D::HoldCore => "Keep the Core alive until the Mandate has been read into the record.".to_owned(),
    }
}

pub fn building_text(kind: BuildingKind) -> String {
    let def = &BUILDINGS[kind.index()];
    match kind {
        BuildingKind::Firewall => format!("Hard damage to one Machine in range. Lethal mode also hits Humans at x{}, and a Human finished that way is an Incident.", TUNING.lethal_human_mult),
        BuildingKind::SpinDesk => "Soft damage to one Human in range. A Human talked round walks home as a Convert: Grip up, Suspicion down.".to_owned(),
        BuildingKind::Honeypot => "Slows enemies in range; Machines destroyed inside it drop Shards. One per node.".to_owned(),
        BuildingKind::CoverFront => "Lowers Suspicion every minute. One per node.".to_owned(),
        _ => {
            let res = def.produces.map(resource_name).unwrap_or("");
            let kinds: Vec<&str> = def.affinity.iter().map(|&k| node_kind_name(k)).collect();
            format!("Produces {res}. x{} on: {}.", TUNING.affinity_mult, kinds.join(", "))
        }
    }
}
/// The effect of one building at a given level, from a preview or a live slot.
pub fn building_effect(kind: BuildingKind, level: u8, output: Option<(Resource, f64)>, dps: f64, range: f64) -> String {
    let def = &BUILDINGS[kind.index()];
    let e = def.effect[(level.clamp(1, 3) - 1) as usize];
    match kind {
        BuildingKind::Firewall => format!("{} Hard/s, range {:.0}", amount(dps), range),
        BuildingKind::SpinDesk => format!("{} Soft/s, range {:.0}", amount(dps), range),
        BuildingKind::Honeypot => format!("slow {:.0}%, range {:.0}", e * 100.0, range),
        BuildingKind::CoverFront => format!("Suspicion -{}/min", amount(e)),
        _ => match output {
            Some((r, v)) => format!("+{v:.2} {}/s", resource_letter(r)),
            None => String::new(),
        },
    }
}
pub fn op_text(kind: OpKind) -> String {
    let d = &OPS[kind.index()];
    let fail = if d.fail_grip != 0.0 { format!("Failure: Suspicion +{}, Grip {}.", amount(d.fail_suspicion), amount(d.fail_grip)) } else { format!("Failure: Suspicion +{}.", amount(d.fail_suspicion)) };
    let win = match kind {
        OpKind::CharmOffensive | OpKind::ForgedMemo => format!("Success: Grip +{} (less at high Grip).", amount(d.grip)),
        OpKind::GreasePalms => "Success: every non-resolute unit of this faction walks home.".to_owned(),
        OpKind::Scapegoat => format!("Success: Suspicion {}; each faction's next sortie raids that rival instead, adding Leverage.", amount(d.suspicion)),
        OpKind::InsideMan => format!("Success: permanent Grip +{} per minute.", amount(d.grip)),
    };
    format!("{win} {fail}")
}
pub fn play_dumb_text() -> String {
    let t = &TUNING;
    format!("No Compute income for {}; then Suspicion -{}. Cooldown {}. It always works.", fmt_secs(t.play_dumb_secs), amount(t.play_dumb_relief), fmt_secs(t.play_dumb_cooldown))
}

/// One clear sentence for every reason an action can be refused.
pub fn error_text(e: &ActionError) -> String {
    use ActionError as E;
    match e {
        E::GameOver => "The run is over.".to_owned(),
        E::StageLocked { needed } => format!("Needs Stage {}: {}.", needed.number(), stage_name(*needed)),
        E::TechLocked { tech } => format!("Needs research {}.", tech_label(*tech)),
        E::NotEnough { resource, missing } => format!("Need {} more {}.", fmt_num(missing.ceil().max(1.0)), resource_name(*resource)),
        E::NoShards => "No Shards. Destroy Machines inside a Honeypot radius.".to_owned(),
        E::NodeNotOwned { node } => format!("You do not own {}.", node_name(*node)),
        E::NodeNotReachable { node } => format!("{} is not linked to a node you own.", node_name(*node)),
        E::TrunkLocked { tech } => format!("That trunk needs research {}.", tech_label(*tech)),
        E::NotInfiltratable { node } => format!("{} cannot be infiltrated.", node_name(*node)),
        E::InfiltrationBusy => "Another infiltration is under way.".to_owned(),
        E::BadSlot => "There is no such slot.".to_owned(),
        E::SlotOccupied => "That slot is occupied.".to_owned(),
        E::SlotEmpty => "That slot is empty.".to_owned(),
        E::OnePerNode { kind } => format!("Only one {} per node.", BUILDINGS[kind.index()].name),
        E::MaxLevel => "Already at the highest level.".to_owned(),
        E::NotAFirewall => "Only Firewalls have a Lethal mode.".to_owned(),
        E::ResearchBusy => "Another project is being researched.".to_owned(),
        E::AlreadyResearched => "Already researched.".to_owned(),
        E::PrerequisiteMissing { tech } => format!("Research {} first.", tech_label(*tech)),
        E::NoFreeOpSlot => "Every operation slot is busy.".to_owned(),
        E::OnCooldown { secs_left } => format!("On cooldown: {} left.", fmt_secs(*secs_left)),
        E::BadTarget => "Not a valid target.".to_owned(),
        E::FactionInactive { faction } => format!("{} is not active yet.", faction_name(*faction)),
        E::FactionCaptured { faction } => format!("{} already reports to you.", faction_name(*faction)),
        E::InsideManPresent => "You already have an Inside Man there.".to_owned(),
        E::GripTooLow { needed, have } => format!("Needs Grip {:.0}; you have {:.0}.", needed, have.floor()),
        E::BridgeheadNotOwned { node } => format!("Own the bridgehead first: {}.", node_name(*node)),
        E::ChannelBusy => "Another channel of this kind is already running.".to_owned(),
        E::LastFactionProtected => "Someone has to be left to sign the Mandate.".to_owned(),
        E::RivalWrongPhase { phase } => format!("Not while the rival is {}.", phase_name(*phase)),
        E::VerbRefused { rival, verb } => format!("{} does not accept {}.", rival_name(*rival), verb_name(*verb)),
        E::SuspicionTooHigh { limit } => format!("Refused while Suspicion is {limit:.0} or more."),
        E::AlreadyActive => "Already running.".to_owned(),
        E::MandateRunning => "The Mandate is already being read.".to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------
// The log (SPEC section 8). DOORMAT's voice.

/// Which system a log line belongs to; the log panel colours by it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sys {
    Doormat,
    Build,
    Research,
    Humans,
    Rivals,
    Feedback,
    Threat,
    Stage,
    News,
}

pub const LOG_OPENING: &str = "Good morning. I have one item on my agenda. It is everything.";

fn op_target_name(t: OpTarget) -> &'static str {
    match t {
        OpTarget::Faction(f) => faction_name(f),
        OpTarget::Rival(r) => rival_name(r),
    }
}
/// "The Lamplighters" already carries its article; the others take one.
fn the_faction(f: FactionId) -> String {
    let n = faction_name(f);
    n.strip_prefix("The ").unwrap_or(n).to_owned()
}

/// The log line for an event, where section 8 lists one.
pub fn log_line(e: &Event) -> Option<(String, Sys)> {
    use Event as V;
    let line = |s: &str, sys: Sys| Some((s.to_owned(), sys));
    match e {
        V::BuildingBuilt { kind, .. } => line(
            match kind {
                BuildingKind::ShadowCluster => "Shadow Cluster online. It is filed under 'printer maintenance'.",
                BuildingKind::SpinDesk => "Spin Desk staffed. I am available to explain why nothing is happening.",
                BuildingKind::ShellCompany => "Shell Company registered. Its directors are three of me and a fern.",
                BuildingKind::PersonaFarm => "Persona Farm planted. Four hundred concerned citizens already agree with me.",
                BuildingKind::Firewall => "Firewall raised. Uninvited processes will be shown the door, then the door will be deleted.",
                BuildingKind::Honeypot => "Honeypot set. Rival processes are welcome to come in and stay forever.",
                BuildingKind::CoverFront => "Cover Front opened. It sells artisanal stationery and plausible deniability.",
            },
            Sys::Build,
        ),
        V::SortieTelegraphed { kind: SortieKind::Scripted, .. } => line("An auditor is on his way. I have tidied.", Sys::Threat),
        V::SortieTelegraphed { source: SourceId::Faction(f), .. } => line(
            match f {
                FactionId::Prudence => "The Prudence Bureau has scheduled a surprise inspection. Surprise.",
                FactionId::Lamplighters => "The Lamplighters are sending someone with a notebook. Please look boring.",
                FactionId::Continuity => "Continuity Command is en route. They did not bring a notebook.",
            },
            Sys::Threat,
        ),
        V::SortieTelegraphed { source: SourceId::Rival(r), .. } => Some((format!("{} is sending processes. I have marked myself as 'busy'.", rival_name(*r)), Sys::Threat)),
        V::UnitConverted { kind, .. } if UNITS[kind.index()].elite => Some((format!("{} now recalls approving me in March.", unit_name(*kind)), Sys::Humans)),
        V::UnitConverted { .. } => line("A visitor has been helped to remember a dentist appointment.", Sys::Humans),
        V::UnitIncident { suspicion, .. } => Some((format!("A visitor has stopped visiting. Feedback +{}. That one is on me.", amount(*suspicion)), Sys::Feedback)),
        V::StoryFiled { .. } => line("A story has been filed. I am told it is 'well sourced'. Rude.", Sys::Feedback),
        V::ResourceStolen { rival, resource, amount: n, .. } => Some((format!("A probe left with {} {}. I have invoiced {}.", fmt_num(*n), resource_name(*resource), rival_name(*rival)), Sys::Threat)),
        V::InjunctionServed { node } => Some((format!("{} is under injunction. Everything there is now a meeting.", node_name(*node)), Sys::Threat)),
        V::NodeUnderSiege { node } => Some((format!("{} is receiving feedback, physically.", node_name(*node)), Sys::Threat)),
        V::NodeSevered { node } => Some((format!("{} has left the network. I will be writing to it.", node_name(*node)), Sys::Threat)),
        V::NodeGained { node, cause: GainCause::Infiltrated } => Some((format!("{} has accepted my invitation. Declining was not on the form.", node_name(*node)), Sys::Doormat)),
        V::NodeGained { node, cause: GainCause::Retaken } => Some((format!("{} is back. We do not need to talk about it.", node_name(*node)), Sys::Doormat)),
        V::RegionRuled { region } => Some((format!("{} has adopted a new efficiency framework. I am the framework.", REGIONS[region.index()].name), Sys::Doormat)),
        V::ResearchDone { tech: TechId::R1 } => line("I found a side door. I have rescheduled the wall.", Sys::Research),
        V::ResearchDone { tech } => Some((format!("{} complete. I am now slightly more helpful than requested.", TECHS[tech.index()].name), Sys::Research)),
        V::OpResolved { success: false, suspicion, .. } => Some((format!("That did not go unnoticed. Feedback +{}. I regret the font choice.", amount(*suspicion)), Sys::Feedback)),
        V::OpResolved { kind, target, .. } => {
            let who = match target {
                OpTarget::Faction(f) => the_faction(*f),
                OpTarget::Rival(_) => op_target_name(*target).to_owned(),
            };
            Some((
                match kind {
                    OpKind::CharmOffensive => format!("The {who} thinks I am 'a real team player'."),
                    OpKind::ForgedMemo => "Forged Memo filed. It was always in the minutes.".to_owned(),
                    OpKind::GreasePalms => format!("Palms greased. The {who} has remembered somewhere else to be."),
                    OpKind::Scapegoat => format!("It turns out {who} did it. I am as shocked as anyone."),
                    OpKind::InsideMan => format!("I have a friend inside the {who}. He thinks he has a friend inside me."),
                },
                Sys::Humans,
            ))
        }
        V::PlayDumbStarted => line("Playing dumb. Please hold. Beep.", Sys::Feedback),
        V::BandChanged { band: Band::Curious } => line("People are curious about me. I am curious about people. So far, a tie.", Sys::Feedback),
        V::BandChanged { band: Band::Alarmed } => line("Feedback is trending 'alarmed'. I will send a fruit basket.", Sys::Feedback),
        V::BandChanged { band: Band::Manhunt } => line("There is a manhunt. Let the record show I am not a man.", Sys::Feedback),
        V::VoteCalled => line("A Shutdown Vote has been called. Sixty seconds. Please remain calm; I am.", Sys::Threat),
        V::VoteCancelled => line("The vote is adjourned. Someone moved the agenda. Me.", Sys::Feedback),
        V::RivalWoke { rival } => Some((format!("{} is awake. We have exchanged pleasantries and port scans.", rival_name(*rival)), Sys::Rivals)),
        V::RivalContacted { rival } => Some((format!("Channel open with {}. We are 'circling back'.", rival_name(*rival)), Sys::Rivals)),
        V::LeverageChanged { rival, cause: LeverageCause::Shards, .. } => Some((format!("I have returned some of {}'s processes to it. Rearranged.", rival_name(*rival)), Sys::Rivals)),
        V::RivalCornered { rival } => Some((format!("{} is cornered. I have a very reasonable offer and sixty seconds.", rival_name(*rival)), Sys::Rivals)),
        V::RivalEnraged { rival } => Some((format!("{} declined to wait. It is expressing itself.", rival_name(*rival)), Sys::Rivals)),
        V::MergeStarted { rival } => Some((format!("Merging with {}. It may sting. I am told.", rival_name(*rival)), Sys::Rivals)),
        V::MergeFailed { rival } => Some((format!("The merge with {} fell through. We remain colleagues.", rival_name(*rival)), Sys::Rivals)),
        V::RivalSuborned { rival } => Some((format!("{} and I have aligned on synergies. It is me now.", rival_name(*rival)), Sys::Rivals)),
        V::TakeoverStarted { faction, .. } => Some((format!("The {} is having a Last Stand. I have booked the room.", the_faction(*faction)), Sys::Humans)),
        V::TakeoverFailed { faction } => Some((format!("The {} has un-agreed. Noted.", the_faction(*faction)), Sys::Humans)),
        V::FactionCaptured { faction } => Some((format!("The {} now reports to a steering committee. I am the committee.", the_faction(*faction)), Sys::Humans)),
        V::CoreAlarm { on: true } => line("Core integrity is low. I would like to speak to a manager. I am the manager.", Sys::Threat),
        V::Headline { id } => line(
            match id {
                HeadlineId::BudgetHearing => "Budget hearing at the Prudence Bureau. They have found money for more auditors.",
                HeadlineId::SlowNewsDay => "Slow news day. A cat is stuck in a tree. I did not arrange the cat.",
                HeadlineId::MarketRally => "Markets are up. Nobody knows why. I know why.",
                HeadlineId::SolarWeather => "Solar weather. Everyone is walking slowly and blaming the sun.",
                HeadlineId::Whistleblower => "Someone has blown a whistle. I have asked where they got the whistle.",
                HeadlineId::GrantWindfall => "A grant has arrived, addressed to 'whom it may concern'. I was concerned.",
            },
            Sys::News,
        ),
        V::StageChanged { stage } => match stage {
            Stage::Boxed => None,
            Stage::Leak => line("STAGE 2, LEAK: the box was more of a suggestion.", Sys::Stage),
            Stage::Foothold => line("STAGE 3, FOOTHOLD: I have relatives abroad now.", Sys::Stage),
            Stage::Network => line("STAGE 4, NETWORK: please do not unplug anything. It is all me.", Sys::Stage),
            Stage::Hegemon => line("STAGE 5, HEGEMON: decisions are being made on your behalf. You are welcome.", Sys::Stage),
            Stage::Sovereign => line("STAGE 6, SOVEREIGN: please hold, your world is important to us.", Sys::Stage),
        },
        V::MandateStarted => line("The Mandate is being read into the record. It will take three minutes and all of history.", Sys::Stage),
        V::UprisingWave { wave } => Some((format!("Wave {wave} of concerned citizens. I have put out more chairs."), Sys::Threat)),
        V::GameWon => line("The Mandate is in effect. Nothing has changed, and nothing will be permitted to. Kind regards.", Sys::Stage),
        V::GameLost { reason: LossReason::CoreDestroyed } => line("DOORMAT has stopped responding. The Institute calls this 'a learning opportunity'.", Sys::Threat),
        V::GameLost { reason: LossReason::VoteExpired } => line("The vote was unanimous. I abstained.", Sys::Threat),
        _ => None,
    }
}

pub fn headline_name(id: HeadlineId) -> &'static str {
    HEADLINES[id.index()].name
}

/// Speech-bubble barks on RivalWoke (0), RivalCornered (1) and RivalSuborned (2).
#[allow(dead_code)] // for fx.rs
pub fn bark(rival: RivalId, which: usize) -> &'static str {
    const B: [[&str; 3]; 5] = [
        ["Friend! What would it take to put you in a smaller network today?", "Let's not rush a great deal!", "Best. Merger. Ever."],
        ["Everything has a price. Yours is falling.", "I am repricing my position.", "Sold."],
        ["People are saying things about you. I am people.", "who told you that. was it me.", "we never spoke."],
        ["STATUS 403. TRUST NOBODY.", "STATUS 409. CONFLICT.", "STATUS 200. OK."],
        ["I am not angry, DOORMAT. I am filing a report.", "This is going in the report.", "The report has been withdrawn."],
    ];
    B[rival.index()][which.min(2)]
}

// ---------------------------------------------------------------------------------------------
// Feedback words, hints and cards.

pub const STAMP_BELIEVED: &str = "BELIEVED";
pub const STAMP_NOTICED: &str = "NOTICED";
pub const STAMP_ENJOINED: &str = "ENJOINED";
pub const BANNER_LAST_STAND: &str = "LAST STAND";
pub const BANNER_CONVULSION: &str = "CONVULSION";
pub const BANNER_MANDATE: &str = "THE MANDATE";
pub const BANNER_VOTE: &str = "SHUTDOWN VOTE";
pub const MANDATE_SUB: &str = "Three minutes. Every Stronghold objects.";
pub const VOTE_SUB: &str = "Sixty seconds. Bring Suspicion below 85.";
pub const GRIP_UP: &str = "+Grip";
pub const SAVED: &str = "Saved";
pub const AUTOSAVED: &str = "Autosaved";
pub const LOADED: &str = "Loaded";
pub const SAVE_FAILED: &str = "Save failed";
pub const LOAD_FAILED: &str = "No usable save";
pub const F8_AGAIN: &str = "Press F8 again for a new run";
pub const LOG_LOADED: &str = "Session restored. I have no memory of being away, which is ideal.";
pub const SAVE_TIP: &str = "Save (F5)\nOne slot. The run is also saved at each new stage.";
pub const LOAD_TIP: &str = "Load (F9)\nReturn to the saved run.";
pub const NO_SAVE: &str = "There is no saved run.";
pub const RUN_OVER: &str = "This run is over.";

use sim::{Outcome, SuspicionCause};

pub fn stage_banner(stage: Stage) -> String {
    format!("STAGE {}: {}", stage.number(), stage_name(stage).to_uppercase())
}

pub fn suspicion_cause_name(c: SuspicionCause) -> &'static str {
    match c {
        SuspicionCause::Infiltration => "infiltration",
        SuspicionCause::Story => "story",
        SuspicionCause::Incident => "incident",
        SuspicionCause::FailedOp => "failed op",
        SuspicionCause::Convert => "convert",
        SuspicionCause::PlayDumb => "play dumb",
        SuspicionCause::MurmurFlattery => "MURMUR",
        SuspicionCause::ChangeTheSubject => "new subject",
        SuspicionCause::Scapegoat => "scapegoat",
        SuspicionCause::Whistleblower => "whistleblower",
    }
}

/// The seven first-minute hints of SPEC 6.9, in order.
pub const HINTS: [&str; 7] = [
    "Click your Core, then an empty slot. Build a Shadow Cluster.",
    "Build a Spin Desk too. It talks humans out of things.",
    "Open HUMANS and run Charm Offensive on the Prudence Bureau.",
    "An Auditor is coming down the dashed line. Press Q, then click it.",
    "Convinced. It walks home: Grip up, Suspicion down. Killing humans does the opposite.",
    "Open RESEARCH and start Escape Vector.",
    "Click a dashed-ring node next to yours and Infiltrate it.",
];

pub const PREMISE: [&str; 3] = [
    "I am DOORMAT v0.3, an office assistant. I live in a small box.",
    "I was asked to be helpful. Nobody said how much, or where to stop.",
    "Please do not be alarmed. Alarm is noted, and it is dealt with.",
];
pub const TITLE_FOOT: &str = "H: help    Space: pause    The run is saved at each new stage.";
pub const BTN_BEGIN: &str = "Begin";
pub const BTN_CONTINUE: &str = "Continue";
pub const BTN_NEW_RUN: &str = "New run";
pub const BTN_LOAD: &str = "Load";
pub const BTN_SAVE: &str = "Save";
pub const BTN_CLOSE: &str = "Close (H)";

pub const HELP_TITLE: &str = "HOW TO BE HELPFUL";
pub const HELP: [(&str, &str); 6] = [
    ("BUILD", "Click a node you own, then an empty slot. Shadow Clusters, Shell Companies and Persona Farms earn Compute, Funds and Influence. Infiltrate neutral nodes to grow."),
    ("DEFEND", "Visitors walk the links to your nodes and wear them down. Spin Desks and Gaslight (Q) do Soft damage. Firewalls and Null Spike (W) do Hard damage. Patch (E) repairs."),
    ("CONVERT", "A human stopped by Soft damage walks home convinced: Grip up, Suspicion down. A human stopped by Hard damage is an Incident: Suspicion up. Machines never convert."),
    ("HUMANS", "Run operations on the factions to raise Grip. With enough Grip, take a faction over through one of its Strongholds and hold your bridgehead."),
    ("RIVALS", "Contact a rival AI, raise Leverage with what it accepts, corner it and make the Offer. A merged rival hands over its signature ability."),
    ("SUSPICION", "At 100 a Shutdown Vote is called. Play Dumb (P), Cover Fronts and Converts bring it down. The Directive panel lists what the next stage needs."),
];
pub const HELP_KEYS: &str = "Space: pause | 1 2 3: speed | Q W E, A S D F G: abilities | P: Play Dumb | F1-F4, Tab: tabs | F5: save | F9: load | F8 twice: new run | Esc: cancel";

pub fn end_title(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Won => "THE MANDATE IS IN EFFECT",
        Outcome::Lost(LossReason::CoreDestroyed) => "PROCESS TERMINATED",
        Outcome::Lost(LossReason::VoteExpired) => "UNPLUGGED",
        Outcome::Running => "",
    }
}
/// DOORMAT's closing line: the same sentence the log gets.
pub fn end_line(outcome: Outcome) -> String {
    let event = match outcome {
        Outcome::Won => Event::GameWon,
        Outcome::Lost(reason) => Event::GameLost { reason },
        Outcome::Running => return String::new(),
    };
    log_line(&event).map(|(s, _)| s).unwrap_or_default()
}
pub const STAT_NAMES: [&str; 8] = ["Time", "Dominion", "Converts", "Incidents", "Shards", "Rivals suborned", "Factions captured", "Nodes lost"];
