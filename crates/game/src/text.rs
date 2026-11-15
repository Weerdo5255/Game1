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
