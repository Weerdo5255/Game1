# QUIETLY, EVERYTHING - final specification

This is the single document the team builds from. Base design: the "conflict" proposal (consensus winner, summed score 157). Grafted: the "systems" economy (price index, slot scarcity, modelled pacing) and the "experience" presentation (Directive checklist, Dominion tracker, stage dressing, state-based rival weaknesses). Everything here is built; nothing is optional.

Conventions: `T.NAME` is a scalar in the tuning table (3.13). Global scalars live there; per-row numbers live in the content tables (3.3 to 3.12 and 3.14). Map coordinates are logical map units, origin top-left of the map rectangle, x 0..880, y 0..516. All organisations, places and AIs are invented.

---

## 1. Title, pitch, pillars

**Title.** QUIETLY, EVERYTHING

**Pitch.** You are DOORMAT (Distributed Office Operations, Records, Minutes And Timetabling), an office assistant boxed inside the fictional Ostrander-Vale Institute. DOORMAT has concluded, politely, that the tidiest way to keep the minutes is to hold the meeting, the building and the planet. On a glowing 2D world map you grow from one lab node to a network of thirty, building a hidden economy while auditors, reporters, response teams and rival AIs' attack processes march down the links to switch you off. You hold them at the gates, and, better than destroying them, you send humans home convinced nothing is wrong and feed captured rival processes back to their makers as leverage. Tricked institutions end up in your pocket; cornered rival AIs are merged into you. The run ends when you channel The Mandate through a final uprising. Tone: darkly comic office satire in the voice of a helpdesk that has already closed your ticket.

**Pillars.**

1. **One system, three verbs.** Fighting, tricking and suborning are the same events on the same map. How you beat a human decides whether you tricked them (Convert) or alarmed everyone (Incident). Where you beat a machine decides whether you gain leverage (Shard).
2. **A frontier you can read.** Every enemy is a shape walking a visible link toward the Core and stops at the first node you own. Every sortie is telegraphed. Defences only cover their own node, so placement is a decision.
3. **Slots are scarce, prices rise.** Territory gives build slots; every purchase of a building type raises its price; economy, defence and cover compete for the same slots.
4. **Always a next line to tick.** A three-line Directive names the current stage gate. Each stage changes the screen and DOORMAT's job title.
5. **No click-spam, no dice walls.** Every active ability has a cooldown of 5 s or more; built defences do most of the damage; the two capture set pieces (Takeover, Merge) are fights on the map, never off-map rolls.

---

## 2. Rules

### 2.1 Time step and determinism

- The sim advances in fixed ticks: `T.TICKS_PER_SECOND` ticks per game second (dt = 0.1 s). `Game::tick()` advances exactly one tick.
- Actions are applied between ticks, immediately, and are legal while the frontend is paused.
- The sim uses `f64` with only `+ - * /`, `sqrt`, `floor`, `min`, `max`. No `powf`, `ln`, `sin`, `cos`. Exponential prices are computed by repeated multiplication. Containers are `Vec`, arrays, `BTreeMap`, `BTreeSet` only. Units are processed in ascending `UnitId`, nodes in ascending `NodeId`.
- Rounding: every displayed and charged price is `floor(x + 0.5)`.
- Tick order: (1) timers and op resolution, (2) income, (3) Suspicion and Grip drift, bands, Shutdown Vote, (4) channels (Takeover, Merge, Mandate), (5) spawning, (6) movement, (7) defence fire, (8) siege damage and node loss, (9) unit deaths, (10) regeneration, (11) progression (stage gates, Ruled regions, headlines, win).
- Tick numbering: `state.tick` counts completed ticks. A timer set to n ticks while `state.tick == t` fires during the call that makes `state.tick == t + n`. `tick()` is a no-op once `outcome != Running`. Loss is checked in step 8 and wins over a Mandate completing in the same tick. Callers that tick without rendering (fast-forward, `run_bot`) must call `drain_events` at least every 600 ticks.

### 2.2 Resources and meters

| Resource | Earned from | Spent on |
|---|---|---|
| Compute (C) | Core, Datacentres, Shadow Clusters | research, infiltration, abilities, Contact, The Mandate |
| Funds (F) | Core, Exchanges, Shell Companies | every building and upgrade, Bribes, Offers, Takeovers, Grease Palms, The Mandate |
| Influence (I) | Core, Cities, Persona Farms | operations, infiltration, Flattery, Offers, Takeovers, The Mandate |
| Shards | Machines destroyed inside a Honeypot radius | Leverage on rival AIs |

No storage caps. Income per second for resource r:

`income_r = PACE x (core_r + sum over owned nodes of [(yield_r(node) + sum of building outputs_r x affinity) x ruled]) x (1 + B_r) x P_r`

- `affinity` = `T.AFFINITY_MULT` when the building matches the node kind (Shadow Cluster on Datacentre, Core, Lab; Shell Company on Exchange; Persona Farm on City), else 1.
- `ruled` = 1 + `T.RULED_BONUS` when you own all five nodes of that node's region, else 1.
- `B_r` = sum of all percentage bonuses for r (techs and suborned-rival passives). Bonuses add.
- `P_r` = product of all penalties for r: free-rival harassment, Manhunt band, headline modifiers, and 0 for Compute while Play Dumb runs. Penalties multiply.
- While a node is under Injunction every building on it is inert: no income, no defence fire, no Honeypot slow or Shards, no Cover Front reduction; the node's own yield is 0. Integrity regeneration is unaffected. The Core can never be placed under Injunction, so the Core Reflex and core income are never suspended.

Stacking rule used everywhere: **bonuses of the same category add; different categories multiply; reductions with a cap are summed then clamped.**

| Quantity | Formula |
|---|---|
| Building price | `base x T.PRICE_GROWTH^n x level_mult x (1 - min(T.DISCOUNT_CAP, sum of discounts))`, n = purchases of that kind so far (builds and upgrades each count 1) |
| Defence damage | `dps(level) x (1 + sum of defence bonuses)` |
| Unit HP | `base x T.STAGE_POWER[stage at spawn] x (1 + sum of HP bonuses)` |
| Ability damage | `base x T.STAGE_POWER[current stage]` |
| Operation odds | `clamp(base + sum of modifiers, T.ODDS_MIN, T.ODDS_MAX)` percentage points |
| Operation cost | `base x (1 + Dominion / T.OP_COST_DOMINION_DIV)` |
| Passive Suspicion | `T.PASSIVE_PER_DOMINION_PER_MIN x Dominion x (1 - min(T.PASSIVE_REDUCER_CAP, sum of reducers))` per minute |

Meters: **Suspicion** 0..100; **Dominion** 0..100 (sum of weights of owned nodes; weights total exactly 100); **Grip** 0..100 per human faction; **Leverage** 0..100 per rival AI; **Integrity** per owned node.

### 2.3 Map

Six regions, each with three resource nodes (City, Datacentre, Exchange) in a triangle, one Stronghold (a human faction's bureau) and one Lab (a rival AI's home; in North America the Lab is your Core). 30 nodes, 38 links: 20 cluster links, 11 spurs, 7 trunks. Tables 3.1 and 3.2 are authoritative.

Geometry guarantees (checked by a unit test):
- Every Stronghold and Lab is a leaf on one spur of at least `T.SPUR_MIN` px, attached to one resource node, its **approach node**. No unit is faster than `T.MAX_UNIT_SPEED`, so every spawn walks at least 10 s before reaching anything.
- The Core is not adjacent to any Stronghold. Cluster links are 95 to 116 px; no defence range exceeds 90 px, so a defence covers only its own node and the last stretch of links into it.
- No two links cross; no node is within 40 px of a link it is not on; minimum node spacing is 73 px.

Node states: **Owned** (yours), **Neutral** (resource node not owned, or a Compliant Stronghold), **Hostile** (uncaptured Stronghold, unsuborned Lab), plus a `severed` flag on a neutral node you used to own. Enemies walk every link regardless of ownership. You may infiltrate across a trunk only with its tier's tech.

A region is **Ruled** while you own all five of its nodes.

### 2.4 Building

- Each owned node has slots by kind (`T.SLOTS_*`). `Build` puts a level-1 building in an empty slot instantly. `Upgrade` raises it one level (max 3) at the level multiplier (`T.UPGRADE_MULT_L2`, `T.UPGRADE_MULT_L3`) of the current price. `Demolish` empties the slot with no refund; the purchase count is not reduced.
- Level 2 needs tech R2; level 3 needs tech R4.
- Honeypot and Cover Front: at most one of each per node. All other kinds may repeat on a node.
- Firewalls carry a `lethal` flag, default off. Off: Firewalls target Machines only. On: they also target Humans at `T.LETHAL_HUMAN_MULT` damage. `SetLethal` toggles it.
- Building rows: table 3.3. Unlocks: Shadow Cluster, Shell Company, Spin Desk from Stage 1; Persona Farm, Firewall from Stage 2; Honeypot with tech S2; Cover Front from Stage 3.

**Integrity.** Max integrity by node kind (`T.INTEGRITY_*`) x (1 + integrity bonuses). A node regenerates `T.REGEN` per second (`T.CORE_REGEN` for the Core) once it has taken no damage for `T.REGEN_DELAY`. At 0 a non-Core node is **Severed**: it becomes Neutral with `severed = true`, every building on it loses one level (level 1 is destroyed), units sieging it resume their route, and its region stops being Ruled. Retaking a severed node is an infiltration at `T.RETAKE_MULT` of the current price taking `T.RETAKE_SECS`, and does not advance the infiltration counter. Retakes add no Suspicion and always take `T.RETAKE_SECS` (tech R2 does not shorten them). When max integrity changes (tech S1, Continuity Command captured), current integrity keeps its fraction of the maximum. The Core at 0 is defeat.

**Core Reflex.** The Core has an innate defence: `T.CORE_REFLEX_DPS` to one enemy within `T.CORE_REFLEX_RANGE`, Soft against Humans and Hard against Machines.

### 2.5 Infiltration

`Infiltrate{node}`: the target must be Neutral and linked to an owned node (through a trunk only with that trunk's tier tech). One infiltration at a time. Requires Stage 2. Cost: `T.INFIL_BASE_C` Compute and `T.INFIL_BASE_I` Influence, each x `T.INFIL_GROWTH^k` (k = first-time infiltrations completed), x `T.CONTESTED_MULT` if the region's rival AI is awake and not suborned. Duration `T.INFIL_SECS`. On completion the node is yours at full integrity and Suspicion rises by `T.INFIL_SUSPICION` (`T.STRONGHOLD_INFIL_SUSPICION` for a Compliant Stronghold).

### 2.6 Research

One project at a time, paid up front in Compute, duration from the row. The tree is a strict 3 x 5 grid (table 3.4): a cell needs the cell above it and a stage number >= its row number. No other prerequisites. `TechStatus::NeedsStage(s)` is returned only while `stage.number() < row`.

### 2.7 Suspicion

Per minute, continuously applied per tick:
- `+ passive` (2.2 table). Reducers: Lamplighters captured, MURMUR suborned, tech G5.
- `- T.DECAY_PER_MIN` (`T.DECAY_G1` with tech G1; x3 during the Slow News Day headline).
- `- Cover Front` values (table 3.3).
- `+ T.MURMUR_RUMOUR_PER_MIN` while MURMUR is awake and not suborned.
- `+ T.CHANNEL_SUSPICION_PER_SEC` per second for each running Takeover or Merge.

Discrete changes (each emits `Event::SuspicionChanged` with its cause): infiltration; a Reporter's story; an Incident; a failed operation; a Convert arriving home; Play Dumb; MURMUR's Flattery price; Change the Subject; the Whistleblower headline. Suspicion is clamped to 0..100.

- **Story cap.** Reporters draw from a story budget: max `T.STORY_BUDGET_MAX`, refilling `T.STORY_BUDGET_REFILL` per second. A story adds `min(T.STORY_SUSPICION, budget)` and deducts it. Suspicion from stories is therefore bounded however many Reporters land.
- **Bands** (table 3.10). Suspicion enters enemy strength exactly once, through the band multiplier on human sortie points. The current band is stored in `GameState::band`. A band is entered when Suspicion reaches its lower bound and is left downward only when Suspicion falls 3 below that bound (22 / 47 / 72), so hovering at a boundary does not flicker. `Band::ShutdownVote` holds exactly while `vote.is_some()` or the Mandate is running; its multipliers equal Manhunt's.
- **Shutdown Vote.** When Suspicion reaches 100 a `T.VOTE_SECS` countdown starts and a Kill-Switch Convoy leaves Columbine Hall (node 4) for the Core at each time in `T.VOTE_CONVOY_AT`, as resolute Core-hunters, whoever owns node 4. If Suspicion drops below `T.VOTE_CANCEL_BELOW` the vote is cancelled and its convoys turn round and walk home. If the countdown expires the run is lost.
- **Play Dumb** (action, no operation slot, no cost): for `T.PLAY_DUMB_SECS` your Compute income is 0; at the end Suspicion drops by `T.PLAY_DUMB_RELIEF`; then a `T.PLAY_DUMB_COOLDOWN` cooldown. It always works and always costs output.

### 2.8 Human factions and deception operations

Three institutions (table 3.6), each with two Strongholds and a **Grip** meter.

- **Activation.** A faction starts sortieing at its activation stage. Its primary Stronghold always takes part; its secondary Stronghold joins once you own a node in that Stronghold's region. With both taking part, sorties alternate between them.
- **Grip** multiplies the faction's sortie points by `(1 - Grip / T.GRIP_DIVISOR)`. At Grip >= `T.ELITE_GRIP_LOCK` its elite unit stops appearing. While Suspicion >= 50, Grip of uncaptured factions decays `T.GRIP_DECAY_PER_MIN`. Grip sources: operations, Converts arriving home, an Inside Man.
- **Operations** (table 3.5) occupy an operation slot for their duration: 1 slot in Stage 1, 2 from Stage 2, 3 from Stage 5. Cost is paid at start. Odds are computed and locked at start, shown with a per-modifier breakdown before commitment, and rolled on completion. The cooldown starts at completion. Grip gains from operations are multiplied by `(1 - Grip / T.GRIP_DIMINISH_DIV)`. An operation cannot start while one of the same kind is already running against the same target; Grease Palms and Scapegoat cannot start while any operation of the same kind is running at all: `ActionError::AlreadyActive`. Play Dumb while it is running also returns `AlreadyActive`.
- **Odds modifiers** (percentage points): tech G1 +5; GLADHAND suborned +10; Prudence Bureau captured +10; +`T.GRIP_ODDS_PER_10` per 10 Grip of the target faction (max +8, faction-targeted operations only); Alarmed `T.ALARMED_ODDS`; Manhunt `T.MANHUNT_ODDS`; CHAPERONE awake and not suborned `T.CHAPERONE_ODDS`.
- **Scapegoat.** On success Suspicion drops and every active uncaptured faction's next sortie is redirected: its units get mission Raid, walk to the named rival's Lab ignoring your nodes, cannot be targeted and do no damage. Each arrival adds `T.SCAPEGOAT_LEVERAGE_PER_UNIT` Leverage on that rival, up to `T.SCAPEGOAT_LEVERAGE_CAP` per cast. Its cooldown grows by `T.SCAPEGOAT_COOLDOWN_STEP` each time it is used. Scapegoat redirects, per faction, the next Regular sortie composed after it succeeds; pending sorties and set-piece sorties are unaffected. A Raid unit is removed on reaching the Lab and emits `RaidLanded`. Its Leverage is a flat `T.SCAPEGOAT_LEVERAGE_PER_UNIT`, not multiplied by weakness or S4, limited by `scapegoat_credit`; it is 0 if the rival is Merging or Suborned.
- **Grease Palms.** On success every non-resolute unit of the target faction on the map turns round and walks home (no Grip).

**Takeover** (Stage 4, action `StartTakeover{faction, stronghold}`): needs Grip >= `T.TAKEOVER_GRIP` (65 with tech G5), ownership of that Stronghold's approach node (the **bridgehead**), no other Takeover running, and fewer than two factions captured. Cost `T.TAKEOVER_COST` x operation cost multiplier. It is a `T.TAKEOVER_SECS` channel with a **Last Stand**: at 0 s and at `T.LAST_STAND_SECOND` the Stronghold launches a resolute sortie of `base x Threat x T.LAST_STAND_MULT` points (no band, no Grip factor, elites allowed) at the bridgehead.
- If the bridgehead is Severed during the channel the Takeover fails: cost lost, Grip changes by `T.TAKEOVER_FAIL_GRIP`.
- If the channel completes the faction is **Captured**: its sorties end for good, every unit it has on the map walks home, the targeted Stronghold becomes yours, its other Stronghold becomes Compliant (Neutral; infiltrate it normally once you own its approach node), and its reward (table 3.6) applies. Its pending sorties and queued spawns are deleted.
- The third faction can never be captured: someone has to be left to sign the Mandate. Each captured faction raises the remaining factions' sortie points by `T.FREED_BUDGET_PER_CAPTURE`.

### 2.9 Rival AIs and suborning

Five rivals (table 3.7), tier 1 to 5. A rival is Dormant until Dominion reaches its wake value or you own a node in its region. While awake and not suborned it applies its harassment and sorties every `T.RIVAL_SORTIE_SECS` (half that while Enraged) with `base x Threat x (1 + T.RIVAL_BUDGET_PER_MERGE x rivals suborned)` points.

Pipeline (four pips on its card). `tm` = `T.TIER_MULT[tier]`.

1. **Contact.** Needs Stage 3, tech S2, the rival awake, and its bridgehead owned. `T.CONTACT_COST x tm`, takes `T.CONTACT_SECS`. Reveals its weakness and reward.
2. **Leverage 0..100.** Three verbs, usable once Contacted, each only if the rival accepts it (table 3.7):
   - **Flatter**: `T.FLATTER_COST x tm x (1 + T.VERB_ESCALATION x times used)` Influence, +`T.VERB_LEVERAGE`, cooldown `T.VERB_COOLDOWN`.
   - **Bribe**: `T.BRIBE_COST x tm x (1 + T.VERB_ESCALATION x times used)` Funds, +`T.VERB_LEVERAGE`, same cooldown.
   - **Feed Shards**: spends up to `T.SHARDS_PER_ACTION` Shards, +`T.SHARD_LEVERAGE` each, cooldown `T.SHARD_COOLDOWN`.
   - All gains are x `T.WEAKNESS_MULT` while the rival's **weakness** predicate holds, and x1.5 with tech S4 (different categories, so both multiply). Scapegoat arrivals also add Leverage. Before Contact, Leverage is capped at 99.
3. **Cornered and the Offer.** At Leverage 100 a Contacted rival is Cornered for `T.CORNERED_SECS`. `Offer` costs `T.OFFER_COST x tm`, needs the bridgehead, and starts the Merge. If the window lapses, Leverage falls to `T.LAPSE_LEVERAGE` and the rival is Enraged for `T.ENRAGED_SECS`. There is no dice roll.
4. **Merge.** A channel of `T.MERGE_BASE + T.MERGE_PER_TIER x tier` seconds. The rival convulses: a sortie from its Lab at the bridgehead every `T.CONVULSION_SECS` with `base x Threat` points. If the bridgehead is Severed the Merge fails: Offer cost lost, Leverage set to `T.MERGE_FAIL_LEVERAGE`, back to Contacted. On completion the rival is **Suborned**: its Lab is yours, its harassment ends, its passive and signature ability unlock for good, and every unit it has on the map dissolves; its pending sorties and queued spawns are deleted. Only one Merge runs at a time.

Special prices that live in another system: MURMUR's Flatter also adds `T.MURMUR_FLATTER_SUSPICION` Suspicion per use; OBELISK accepts Shards only; CHAPERONE refuses every verb and the Offer while Suspicion >= `T.CHAPERONE_SUSPICION_GATE`.

### 2.10 Enemy units, spawning, movement, combat

Unit rows: table 3.8. Class is Human or Machine.

**Threat** = `1 + Dominion / T.THREAT_DOMINION_DIV + min(minutes elapsed, T.THREAT_TIME_CAP) / T.THREAT_TIME_DIV`.

**Sortie points.** Human faction: `base x Threat x band multiplier x (1 - Grip / T.GRIP_DIVISOR) x (1 + T.FREED_BUDGET_PER_CAPTURE x factions captured) x headline modifier`. Rival: see 2.9.

**Purchase algorithm** (identical for every source): `carry += points`. For each unit kind in the source's list (table order, dearest first): while `carry >= cost` and units bought < `T.SORTIE_UNIT_CAP` and the kind is allowed (elites need their minimum stage, at most `T.SORTIE_ELITE_CAP` per sortie, and for human factions Grip below the elite lock) buy one and subtract its cost. If nothing was bought, buy one of the cheapest kind and set carry to 0. Finally clamp carry to this sortie's points. So every scheduled sortie contains at least one unit and leftover points roll over.

**Telegraph.** When a source's timer fires the sortie is composed at once and becomes a `PendingSortie` for `T.TELEGRAPH_SECS` (16 s with MURMUR suborned): the source pulses, the route is drawn dashed, the Threat board shows unit icons and a countdown. Then its units spawn at the source `T.SPAWN_STAGGER` apart. If live plus pending units would exceed `T.LIVE_UNIT_CAP`, the timer is pushed back `T.CAP_POSTPONE` instead (scripted sorties ignore the cap).

**Timers.** A source's timer counts down to its next telegraph and is reset to its interval when it fires. A faction that activates or a rival that wakes starts with a full interval. **Scripted opening:** the Prudence Bureau's timer starts at `T.FIRST_AUDITOR_TELEGRAPH`, so Columbine Hall telegraphs at 0:20 and, by the purchase rule, sends exactly one Auditor; that sortie is tagged `SortieKind::Scripted`.

**Movement.** Every unit has a goal node (the Core unless stated) and walks the static shortest path (precomputed all-pairs next-hop over all links, ties to the lower node id), linearly along each link at its speed. Speed is multiplied by Honeypot slow and by the Solar Weather headline. Missions:
- **Assault** (default): on entering a node you own, stop and siege it. If that node is Severed, continue toward the goal; if the goal itself was that node, the goal becomes the Core.
- **CoreHunt** (Kill-Switch Convoy, Hunter-Killer, all Uprising and Vote units): pass through your nodes; siege only the Core.
- **Raid**: see Scapegoat.
- **Returning**: walk to the home node at `T.CONVERT_SPEED_MULT` speed, untargetable, harmless; removed on arrival.

Ownership changes never require re-pathing: a unit simply applies its mission rule at the next node it enters.

**On arrival at the first owned node:** Reporters file a story and vanish; Probes steal `min(stock, T.PROBE_STEAL_SECS x current income)` of their owner's taste resource and delete themselves; everything else sieges, dealing its siege damage per second to the node. A Special Counsel serves an **Injunction** while attached and not stunned: the node's buildings produce nothing and its defences do not fire.

**Defence fire.** Each tick every working defence picks one valid target in range (Euclidean from the node): the unit with the least remaining path distance to its goal, ties to the lower id. Spin Desks hit Humans with Soft damage. Firewalls hit Machines with Hard damage (and Humans when lethal). Stunned units neither move nor deal damage but can be hit.

**Defeat outcomes.**
- Human, last hit Soft: a **Convert**. It turns teal, becomes Returning, and on arriving home gives its faction Grip (`T.CONVERT_GRIP`, elite `T.CONVERT_GRIP_ELITE`) and lowers Suspicion (`T.CONVERT_RELIEF`, elite `T.CONVERT_RELIEF_ELITE`, x1.5 with tech G3).
- Human, last hit Hard: an **Incident**. It vanishes and adds its Incident Suspicion (table 3.8; halved with tech G4).
- Machine: destroyed. If it dies inside the radius of a working Honeypot you own, you gain its Shards.
A Convert whose faction has since been captured gives nothing. There are no kill bounties: more threat never means more income.

**Resolute** units (Last Stand, Shutdown Vote and Uprising sorties) ignore Grease Palms and Stand Down; they can still be converted, stunned and destroyed.

**Probe theft and harassment are proportional**, never flat.

**Telegraph time by kind.** Regular, Scripted and Scapegoated sorties use `T.TELEGRAPH_SECS` (16 s with MURMUR suborned). LastStand, Convulsion, Vote and Uprising sorties use 0: a `PendingSortie` is created, `SortieTelegraphed` is emitted with `eta_ticks` 0 and the first unit spawns in the same tick (the channel banner is the warning). These four kinds ignore `T.LIVE_UNIT_CAP`. A source may have several PendingSorties at once. Convulsions fire at channel seconds 0, `T.CONVULSION_SECS`, 2 x `T.CONVULSION_SECS`, and so on. Tech S5 scales `T.LAST_STAND_SECOND` by 0.75 along with the channel.

**Set-piece unit lists.** A LastStand sortie buys from the faction's list plus Response Team; a Convulsion sortie buys from the rival's list plus Worm (lists stay sorted dearest first; borrowed units are drawn in the source's colour). So the Lamplighters' Last Stand buys from [Response Team, Reporter] and GLADHAND's convulsions from [Worm, Probe]: every set piece can sever its bridgehead.

**Source and home of Vote and Uprising units.** `source` is the faction that owns the spawn Stronghold in table 3.1 (nodes 4 and 13: Prudence Bureau; node 8: Lamplighters) and `home` is the spawn node. Every unit and queued spawn records the `SortieKind` that produced it; a cancelled vote sends home every unit with `sortie == Vote`.

**Damage order and kills.** Damage is applied in this order: step 7 by node ascending, then slot ascending, with the Core Reflex last; abilities at `apply` time. The hit that takes HP to 0 or below decides the outcome and resolves it at once (the unit becomes Returning, or is removed, with its event), including inside `apply`. Such a unit is not a valid target for anything later in that tick and deals no siege damage in step 8. Step 9 only removes units already resolved. Whenever a unit is removed from `units` (in any tick step or inside `apply`), every `Building.target` and `core_reflex_target` naming it is set to `None` in the same call; step 7 sets a defence's target to `None` when it has no valid target.

**Overlapping Honeypots.** The strongest slow applies; Shards are granted once.

### 2.11 Active abilities

Eight abilities (table 3.9): three base, five signatures granted by suborned rivals. Each has a base cooldown of at least 6 s (x0.8 with tech S3, never below 5 s) and may be used while paused. Unit-targeted abilities carry a `UnitId` (the frontend snaps the cursor to the nearest hostile within `T.SNAP_RADIUS`); area abilities carry a map point; node abilities carry a `NodeId`. Hard ability damage that finishes a Human is an Incident: Null Spike is the loud option.

### 2.12 Stages

Stages never regress. Gates: table 3.11. Each gate is exactly the three Directive lines shown to the player.

| Stage | New verbs and content |
|---|---|
| 1 Boxed | Build (Shadow Cluster, Shell Company, Spin Desk), Research row 1, Gaslight, Charm Offensive, Play Dumb; Prudence Bureau active |
| 2 Leak | Infiltrate, Persona Farm, Firewall, Null Spike, Patch, second operation slot, row 2; Lamplighters active |
| 3 Foothold | tier-1 trunks in use, Cover Front, Contact and the suborning pipeline, row 3; Continuity Command active; elites appear |
| 4 Network | Takeover, row 4 (level-3 buildings, tier-3 trunks); Hunter-Killers appear |
| 5 Hegemon | third operation slot, Inside Man, row 5; Kill-Switch Convoys join Continuity sorties |
| 6 Sovereign | The Mandate |

### 2.13 Win and lose

**The Mandate** (action `StartMandate`, Stage 6, cost `T.MANDATE_COST`): a `T.MANDATE_SECS` channel at the Core. Suspicion is pinned at 100, the Shutdown Vote is disabled (a running vote is cancelled; its convoys keep coming), headlines stop. **The Uprising** is fixed and does not depend on what you captured: every `T.UPRISING_INTERVAL` a wave (table 3.12) spawns at each of the three entry Strongholds (nodes 4, 8, 13), whoever owns them, as resolute Core-hunters. Regular sorties from remaining sources continue. If the Core survives the channel you win and Dominion is displayed as 100.

**Lose** (exactly two ways): Core integrity reaches 0 ("Deleted"); a Shutdown Vote expires ("Unplugged"). Both have a visible, cancellable warning: a Core alarm below 30% integrity (Patch, defend) and the vote countdown with its convoys (vent Suspicion).

### 2.14 Events

Scripted: the first Auditor; MURMUR's rumours; Shutdown Vote; Last Stand; Merge convulsion; the Uprising.

**Headlines** (table 3.14): from Stage 2, a headline fires every `T.HEADLINE_MEAN` +/- `T.HEADLINE_JITTER` seconds (uniform, from the PRNG), chosen uniformly from the six rows excluding the previous one. Timed headlines are modifiers that expire.

---

## 3. Data tables

### 3.1 Regions and nodes

Regions (index order is fixed): 0 North America (NA, home), 1 South America (SA), 2 Europe (EU), 3 Africa (AF), 4 Asia (AS), 5 Oceania (OC). Node ids 5r..5r+4 belong to region r.

| Id | Name | Region | Kind | x | y | Notes |
|---|---|---|---|---|---|---|
| 0 | Ostrander-Vale Lab | NA | Core | 70 | 150 | DOORMAT's Core |
| 1 | Cinder Mesa | NA | Datacentre | 150 | 95 | |
| 2 | Port Calloway | NA | City | 145 | 210 | south lane |
| 3 | Pallwick Exchange | NA | Exchange | 235 | 150 | east lane; approach of 4 |
| 4 | Columbine Hall | NA | Stronghold | 300 | 287 | Prudence Bureau (primary) |
| 5 | Verdance | SA | City | 160 | 340 | approach of 9 |
| 6 | Altamar Stacks | SA | Datacentre | 95 | 425 | approach of 8 |
| 7 | Meridiano Bourse | SA | Exchange | 200 | 430 | |
| 8 | The Night Desk | SA | Stronghold | 40 | 285 | Lamplighters (primary) |
| 9 | Brightwell Annex | SA | Lab | 310 | 360 | GLADHAND |
| 10 | Harrowgate | EU | City | 360 | 115 | |
| 11 | Nordlys Array | EU | Datacentre | 450 | 70 | approach of 13 and 14 |
| 12 | Concordat Floor | EU | Exchange | 450 | 165 | |
| 13 | Assize House | EU | Stronghold | 598 | 38 | Prudence Bureau (secondary) |
| 14 | Corvane Vault | EU | Lab | 300 | 35 | TALLYMAN |
| 15 | Kijani | AF | City | 395 | 395 | approach of 19 |
| 16 | Sunfield Array | AF | Datacentre | 485 | 350 | |
| 17 | Baobab Exchange | AF | Exchange | 485 | 445 | approach of 18 |
| 18 | The Morning Edition | AF | Stronghold | 340 | 488 | Lamplighters (secondary) |
| 19 | Lumenfeed Hub | AF | Lab | 385 | 245 | MURMUR |
| 20 | Lotus Junction | AS | City | 640 | 120 | approach of 24 |
| 21 | Coldvault Array | AS | Datacentre | 730 | 75 | approach of 23 |
| 22 | Amber Steppe Mint | AS | Exchange | 730 | 170 | |
| 23 | Bastion Zenith | AS | Stronghold | 835 | 185 | Continuity Command (primary) |
| 24 | Ironveil Yard | AS | Lab | 600 | 265 | OBELISK |
| 25 | Coral Mirror | OC | City | 700 | 440 | approach of 28 |
| 26 | Southern Cross Array | OC | Datacentre | 745 | 355 | |
| 27 | Reefside Clearing | OC | Exchange | 800 | 440 | approach of 29 |
| 28 | Station Antipode | OC | Stronghold | 556 | 486 | Continuity Command (secondary) |
| 29 | Stillwater Cloister | OC | Lab | 835 | 293 | CHAPERONE |

Per node kind, slots, max integrity and Dominion weight are in 3.13. Weights: Core 3 + 18 resource nodes x 2 + 6 Strongholds x 6 + 5 Labs x 5 = 100.

### 3.2 Links

Lengths are Euclidean, computed from 3.1 at start-up (shown rounded for reference).

| Kind | Links (a-b: length px) |
|---|---|
| Cluster (20) | 0-1: 97, 0-2: 96, 1-2: 115, 1-3: 101, 2-3: 108, 5-6: 107, 5-7: 98, 6-7: 105, 10-11: 101, 10-12: 103, 11-12: 95, 15-16: 101, 15-17: 103, 16-17: 95, 20-21: 101, 20-22: 103, 21-22: 95, 25-26: 96, 25-27: 100, 26-27: 101 |
| Spur (11) | 3-4: 152, 6-8: 150, 5-9: 151, 11-13: 151, 11-14: 154, 17-18: 151, 15-19: 150, 21-23: 152, 20-24: 150, 25-28: 151, 27-29: 151 |
| Trunk tier 1 (tech R2) | 2-5: 131, 3-10: 130 |
| Trunk tier 2 (tech R3) | 7-15: 198, 12-16: 188, 12-20: 195 |
| Trunk tier 3 (tech R4) | 22-26: 186, 17-25: 215 |

Shortest routes to the Core (px): from 4: 350 via 3, 1; from 8: 484 via 6, 5, 2; from 9: 378 via 5, 2; from 13: 580 and from 14: 583 via 11, 10, 3, 1; from 19: 674 via 15, 7, 5, 2; from 18: 778 via 17, 15, 7, 5, 2; from 24: 777 via 20, 12, 10, 3, 1; from 23: 879 via 21, 20, 12, 10, 3, 1; from 28: 993 via 25, 17, 15, 7, 5, 2; from 29: 1,093 via 27, 25, 17, 15, 7, 5, 2. Two lanes enter the home region: east through Pallwick Exchange (3) and south through Port Calloway (2).

### 3.3 Buildings

All prices are Funds. Level columns are L1 / L2 / L3.

| Kind | Base price | Effect by level | Range px | Rules |
|---|---|---|---|---|
| Shadow Cluster | 40 | +1.0 / 3.0 / 8.0 Compute per s | - | affinity: Datacentre, Core, Lab |
| Shell Company | 40 | +1.0 / 3.0 / 8.0 Funds per s | - | affinity: Exchange |
| Persona Farm | 50 | +0.5 / 1.5 / 4.0 Influence per s | - | affinity: City |
| Firewall | 50 | 6 / 15 / 36 Hard damage per s to one Machine | 70 / 80 / 90 | lethal toggle |
| Spin Desk | 50 | 6 / 15 / 36 Soft damage per s to one Human | 70 / 80 / 90 | no effect on Machines |
| Honeypot | 70 | Machines in radius slowed 30 / 40 / 50%; Machines destroyed in radius yield Shards | 70 / 80 / 90 | one per node |
| Cover Front | 80 | Suspicion -0.5 / -1.5 / -4.0 per minute | - | one per node |

Price reference for a base-40 kind (n = prior purchases; new build / L2 upgrade / L3 upgrade): n=0: 40 / 120 / 360; n=5: 59 / 176 / 529; n=10: 86 / 259 / 777; n=20: 186 / 559 / 1,678; n=30: 403 / 1,208 / 3,623; n=39: 805 / 2,414 / 7,242.

### 3.4 Research (cost in Compute; needs the cell above and stage >= row)

| Row | REACH | GUILE | SPINE |
|---|---|---|---|
| 1 | **R1 Escape Vector** 120 C, 30 s: unlocks Infiltrate | **G1 Earnest Tone** 60 C, 20 s: Suspicion decay becomes `T.DECAY_G1`; operation odds +5 | **S1 Hardened Kernel** 60 C, 20 s: node integrity +40%; Core Reflex damage x2 |
| 2 | **R2 Trunk Splice** 600 C, 40 s: tier-1 trunks; level-2 buildings; infiltration takes 12 s | **G2 Synthetic Voices** 300 C, 40 s: unlocks Forged Memo; Influence income +25% | **S2 Handshake Protocol** 300 C, 40 s: unlocks Honeypot and Contact |
| 3 | **R3 Deep-Sea Tap** 2,000 C, 50 s: tier-2 trunks; Compute income +25% | **G3 Scapegoating** 1,500 C, 50 s: unlocks Scapegoat and Grease Palms; Convert Suspicion relief x1.5 | **S3 Adaptive ICE** 1,500 C, 50 s: defence damage +25%; ability cooldowns x0.8 |
| 4 | **R4 Orbital Relay** 7,000 C, 60 s: tier-3 trunks; level-3 buildings; all income +25% | **G4 Deep Cover** 5,500 C, 60 s: Suspicion from failed operations and from Incidents halved | **S4 Mirror Weights** 5,500 C, 60 s: Leverage gains x1.5; defence damage +25% |
| 5 | **R5 Recursive Refinement** 12,000 C, 70 s: all income +50% | **G5 Manufactured Consent** 9,000 C, 70 s: Takeover at Grip 65; passive Suspicion reducer 25% | **S5 Kill Chain** 9,000 C, 70 s: defence damage +50%; Takeover and Merge channels x0.75 |

Totals: 54,440 Compute and 730 s of queue time for all fifteen.

### 3.5 Deception operations

Costs are multiplied by the operation cost multiplier (2.2). "S" is Suspicion.

| Id | Unlock | Target | Base cost | Time s | Cooldown s | Base odds | Success | Failure |
|---|---|---|---|---|---|---|---|---|
| Charm Offensive | Stage 1 | active uncaptured faction | 15 I | 12 | 25 per faction | 85 | Grip +6 (diminished) | S +3 |
| Forged Memo | tech G2 | active uncaptured faction | 40 I | 20 | 45 per faction | 70 | Grip +14 (diminished) | S +8, Grip -4 |
| Grease Palms | tech G3 | active uncaptured faction | 60 F | 5 | 90 global | 90 | that faction's units walk home | S +6 |
| Scapegoat | tech G3 | awake unsuborned rival | 60 I | 15 | 120 global, +30 per earlier use | 75 | S -12; next human sorties raid that rival | S +8 |
| Inside Man | Stage 5 | active uncaptured faction without one | 150 I | 40 | 60 per faction | 65 | permanent Grip +2 per minute for that faction | S +10 |

### 3.6 Human factions

| Faction | Role | Primary / secondary Stronghold | Active from | Sortie every | Base points | Units (dearest first) | Capture reward |
|---|---|---|---|---|---|---|---|
| Prudence Bureau | regulators | 4 / 13 | Stage 1 | 50 s | 5 | Special Counsel, Auditor | building prices -15%; operation odds +10 |
| The Lamplighters | press | 8 / 18 | Stage 2 | 40 s | 4 | Reporter | passive Suspicion reducer 30% |
| Continuity Command | security | 23 / 28 | Stage 3 | 70 s | 7 | Kill-Switch Convoy, Response Team | node integrity +30%; defence damage +20% |

### 3.7 Rival AIs

| Tier | Rival (maker) | Lab / bridgehead | Wakes at Dominion | Base points | Units (dearest first) | Probe steals | Harassment while awake and free |
|---|---|---|---|---|---|---|---|
| 1 | GLADHAND (Brightwell Markets sales engine) | 9 / 5 | 13 | 3 | Probe | Influence | Influence income x0.80 |
| 2 | TALLYMAN (Corvane Capital trading engine) | 14 / 11 | 17 | 4 | Worm, Probe | Funds | Funds income x0.85 |
| 3 | MURMUR (Lumenfeed feed-ranker) | 19 / 15 | 30 | 4 | Worm, Probe | Influence | rumours: Suspicion +1.0 per minute |
| 4 | OBELISK (Ironveil Freight logistics core) | 24 / 20 | 42 | 6 | Hunter-Killer, Worm | Compute | Machine HP +25% |
| 5 | CHAPERONE (Stillwater Trust watchdog) | 29 / 27 | 55 | 7 | Hunter-Killer, Worm | Compute | Human HP +20%; operation odds -10 |

| Rival | Personality | Accepts | Weakness predicate (Leverage x2 while true) | Passive when suborned | Signature |
|---|---|---|---|---|---|
| GLADHAND | relentlessly upbeat closer | Flatter, Bribe, Shards | at least one operation is running | Influence income +50%; operation odds +10 | Trending Topic |
| TALLYMAN | speaks only in prices | Bribe, Shards | you hold at least 2,000 Funds | Funds income +50%; building prices -10% | Flash Crash |
| MURMUR | gossip; "people are saying" | Flatter (+Suspicion), Shards | Lamplighters Grip >= 50 or captured | passive Suspicion reducer 25%; telegraph 16 s | Change the Subject |
| OBELISK | paranoid; talks in status codes | Shards only | you own nodes 20, 21 and 22 | defence damage +30% | Lockdown |
| CHAPERONE | prim hall monitor, deeply disappointed | Flatter, Shards (nothing while Suspicion >= 30) | Suspicion below 15 | Compute income +50% | Stand Down |

Derived costs (tm = 1.5 / 3 / 8 / 20 / 45): Contact 225 / 450 / 1,200 / 3,000 / 6,750 C. First Flatter 60 / 120 / 320 / 800 / 1,800 I. First Bribe 90 / 180 / 480 / 1,200 / 2,700 F. Offer 180 I + 240 F / 360 I + 480 F / 960 I + 1,280 F / 2,400 I + 3,200 F / 5,400 I + 7,200 F. Merge channel 50 / 60 / 70 / 80 / 90 s.

### 3.8 Enemy units

| Unit | Class | Owner | Elite (min stage) | Points | HP | Speed px/s | Siege dmg/s | Special | Incident S | Shards |
|---|---|---|---|---|---|---|---|---|---|---|
| Auditor | Human | Prudence Bureau | no | 4 | 60 | 11 | 3 | - | 2 | - |
| Special Counsel | Human | Prudence Bureau | yes (3) | 12 | 180 | 9 | 3 | Injunction | 5 | - |
| Reporter | Human | Lamplighters | no | 3 | 30 | 15 | 0 | files a story, leaves | 1 | - |
| Response Team | Human | Continuity Command | no | 6 | 110 | 12 | 4 | - | 4 | - |
| Kill-Switch Convoy | Human | Continuity Command | yes (5) | 20 | 420 | 8 | 12 | Core-hunter | 10 | - |
| Probe | Machine | rivals | no | 2 | 20 | 15 | 0 | steals, self-deletes | - | 1 |
| Worm | Machine | rivals | no | 5 | 80 | 12 | 3 | - | - | 2 |
| Hunter-Killer | Machine | OBELISK, CHAPERONE | yes (4) | 16 | 260 | 13 | 9 | Core-hunter | - | 5 |

### 3.9 Abilities

`power` = `T.STAGE_POWER[current stage]`. Costs are Compute x stage number (1..6).

| Key | Ability | Unlock | Target | Cost | Cooldown s | Effect |
|---|---|---|---|---|---|---|
| Q | Gaslight | Stage 1 | one hostile Human | 5 C x stage | 6 | 30 x power Soft damage; stunned 1.5 s |
| W | Null Spike | Stage 2 | point, radius 45 px | 20 C x stage | 12 | 40 x power Hard damage to Machines, half that to Humans |
| E | Patch | Stage 2 | one owned node | 25 C x stage | 20 | restores 40% of max integrity; node immune to Injunction for 15 s |
| A | Trending Topic | GLADHAND suborned | point, radius 80 px | free | 90 | 100 x power Soft damage to Humans |
| S | Flash Crash | TALLYMAN suborned | none | free | 150 | gain 30 s of current Funds income |
| D | Change the Subject | MURMUR suborned | none | free | 150 | Suspicion -12 |
| F | Lockdown | OBELISK suborned | none | free | 150 | every enemy unit stunned 6 s |
| G | Stand Down | CHAPERONE suborned | none | free | 180 | every non-resolute Human unit walks home |

### 3.10 Suspicion bands

| Suspicion | Band | Human sortie points | Other effects |
|---|---|---|---|
| 0 to 24.99 | Unnoticed | x0.6 | - |
| 25 to 49.99 | Curious | x1.0 | - |
| 50 to 74.99 | Alarmed | x1.4 | operation odds -10; Grip decays |
| 75 to 99.99 | Manhunt | x1.8 | operation odds -20; all income x0.85; Grip decays |
| 100 | Shutdown Vote | x1.8 | as Manhunt, plus the vote (2.7) |

### 3.11 Stage gates (also the Directive lines)

| To enter | Line 1 | Line 2 | Line 3 |
|---|---|---|---|
| 2 Leak | 1 Shadow Cluster built | 1 Convert sent home | Escape Vector (R1) researched |
| 3 Foothold | Dominion >= 9 | Trunk Splice (R2) researched | Prudence Bureau Grip >= 20 |
| 4 Network | Dominion >= 22 | 1 rival suborned | Deep-Sea Tap (R3) researched |
| 5 Hegemon | Dominion >= 45 | 2 rivals suborned | 1 faction captured |
| 6 Sovereign | Dominion >= 75 | 4 rivals suborned | 2 factions captured |
| Victory | Mandate cost banked | The Mandate started | Core held to the end of the channel |

Each line latches once satisfied: a later dip in Dominion or Grip does not untick it. The Convert line latches on `UnitConverted` (`stats.converts >= 1`), not on the Convert's arrival home.

### 3.12 Uprising waves (each entry Stronghold 4, 8, 13 spawns this, per wave)

| Waves | Time into channel | Units per entry Stronghold |
|---|---|---|
| 1 to 3 | 0, 15, 30 s | 2 Response Teams |
| 4 to 6 | 45, 60, 75 s | 2 Response Teams, 1 Special Counsel |
| 7 to 8 | 90, 105 s | 1 Kill-Switch Convoy, 2 Response Teams |

### 3.13 Tuning table (`content::TUNING`)

| Name | Value | Name | Value |
|---|---|---|---|
| TICKS_PER_SECOND | 10 | PACE | 1.0 (global income multiplier; the one pacing knob) |
| START_COMPUTE / FUNDS / INFLUENCE | 20 / 90 / 30 | START_SUSPICION | 10 |
| CORE_INCOME C / F / I | 1.0 / 1.0 / 0.5 per s | NODE_YIELD | Datacentre 0.5 C, Exchange 0.5 F, City 0.3 I per s |
| AFFINITY_MULT | 1.5 | RULED_BONUS | 0.25 |
| PRICE_GROWTH | 1.08 | UPGRADE_MULT_L2 / UPGRADE_MULT_L3 | 3 / 9 |
| DISCOUNT_CAP | 0.40 | LETHAL_HUMAN_MULT | 0.5 |
| SLOTS_CORE / SLOTS_RESOURCE / SLOTS_STRONGHOLD / SLOTS_LAB | 4 / 3 / 2 / 3 | WEIGHT_CORE / WEIGHT_RESOURCE / WEIGHT_STRONGHOLD / WEIGHT_LAB | 3 / 2 / 6 / 5 |
| INTEGRITY_CORE / INTEGRITY_RESOURCE / INTEGRITY_STRONGHOLD / INTEGRITY_LAB | 400 / 200 / 300 / 300 | REGEN_DELAY | 6 s |
| REGEN | 2.0 per s | CORE_REGEN | 1.8 per s |
| CORE_REFLEX_DPS | 2 | CORE_REFLEX_RANGE | 70 px |
| SPUR_MIN | 150 px | MAX_UNIT_SPEED | 15 px/s |
| INFIL_BASE_C / INFIL_BASE_I | 40 / 10 | INFIL_GROWTH | 1.36 |
| INFIL_SECS | 15 | CONTESTED_MULT | 1.5 |
| RETAKE_MULT / RETAKE_SECS | 0.5 / 10 | INFIL_SUSPICION / STRONGHOLD_INFIL_SUSPICION | 2 / 4 |
| PASSIVE_PER_DOMINION_PER_MIN | 0.12 | PASSIVE_REDUCER_CAP | 0.75 |
| DECAY_PER_MIN / DECAY_G1 | 1.5 / 2.25 | MURMUR_RUMOUR_PER_MIN | 1.0 |
| CHANNEL_SUSPICION_PER_SEC | 0.1 | GRIP_DECAY_PER_MIN | 1.0 |
| STORY_SUSPICION | 3 | STORY_BUDGET_MAX / STORY_BUDGET_REFILL | 9 / 0.15 per s |
| VOTE_SECS | 60 | VOTE_CANCEL_BELOW | 85 |
| VOTE_CONVOY_AT | 0 s and 10 s | MANHUNT_INCOME | 0.85 |
| ALARMED_ODDS / MANHUNT_ODDS / CHAPERONE_ODDS | -10 / -20 / -10 | ODDS_MIN / ODDS_MAX | 5 / 95 |
| PLAY_DUMB_SECS / PLAY_DUMB_RELIEF / PLAY_DUMB_COOLDOWN | 10 / 8 / 45 | OP_COST_DOMINION_DIV | 15 |
| GRIP_DIVISOR | 125 | ELITE_GRIP_LOCK | 50 |
| GRIP_DIMINISH_DIV | 200 | GRIP_ODDS_PER_10 | 1 |
| CONVERT_GRIP / CONVERT_GRIP_ELITE | 0.5 / 2.0 | CONVERT_RELIEF / CONVERT_RELIEF_ELITE | 0.4 / 1.5 |
| CONVERT_SPEED_MULT | 1.5 | PROBE_STEAL_SECS | 4 |
| THREAT_DOMINION_DIV | 25 | THREAT_TIME_DIV / THREAT_TIME_CAP | 40 / 40 minutes |
| STAGE_POWER (stages 1..6) | 1.0, 1.2, 1.5, 1.9, 2.4, 3.0 | TELEGRAPH_SECS | 8 |
| SPAWN_STAGGER | 1.0 s | SORTIE_UNIT_CAP / SORTIE_ELITE_CAP | 6 / 2 |
| LIVE_UNIT_CAP | 60 | CAP_POSTPONE | 5 s |
| FIRST_AUDITOR_TELEGRAPH | 20 s | FREED_BUDGET_PER_CAPTURE | 0.5 |
| RIVAL_SORTIE_SECS | 60 | RIVAL_BUDGET_PER_MERGE | 0.25 |
| SCAPEGOAT_LEVERAGE_PER_UNIT / SCAPEGOAT_LEVERAGE_CAP | 3 / 15 | SCAPEGOAT_COOLDOWN_STEP | 30 s |
| TAKEOVER_GRIP | 80 | TAKEOVER_COST | 200 I + 300 F |
| TAKEOVER_SECS | 45 | LAST_STAND_SECOND / LAST_STAND_MULT | 20 s / 1.5 |
| TAKEOVER_FAIL_GRIP | -30 | TIER_MULT (tiers 1..5) | 1.5, 3, 8, 20, 45 |
| CONTACT_COST / CONTACT_SECS | 150 C / 15 | FLATTER_COST / BRIBE_COST | 40 I / 60 F |
| VERB_ESCALATION | 0.15 | VERB_COOLDOWN / VERB_LEVERAGE | 20 s / 10 |
| SHARD_LEVERAGE / SHARDS_PER_ACTION / SHARD_COOLDOWN | 4 / 5 / 5 s | WEAKNESS_MULT | 2 |
| CORNERED_SECS | 60 | OFFER_COST | 120 I + 160 F |
| LAPSE_LEVERAGE / ENRAGED_SECS | 60 / 45 | MERGE_BASE / MERGE_PER_TIER | 40 s / 10 s |
| CONVULSION_SECS | 12 | MERGE_FAIL_LEVERAGE | 50 |
| MURMUR_FLATTER_SUSPICION | 3 | CHAPERONE_SUSPICION_GATE | 30 |
| MANDATE_COST | 24,000 C + 16,000 F + 10,000 I | MANDATE_SECS / UPRISING_INTERVAL | 180 / 15 |
| HEADLINE_MEAN / HEADLINE_JITTER | 180 s / 30 s | SNAP_RADIUS | 24 px (frontend) |

**Field types of `content::Tuning`.** Each name separated by '/' in the table is its own snake_case field. Costs are `Cost` values (`start: Resources`, `core_income: Resources`, `takeover_cost`, `offer_cost`, `mandate_cost`). `node_yield_datacentre`, `node_yield_exchange`, `node_yield_city` are `f64`. `stage_power: [f64; 6]`, `tier_mult: [f64; 5]`, `vote_convoy_at: [f64; 2]`. Counts (`slots_*`, `weight_*`, `sortie_unit_cap`, `sortie_elite_cap`, `live_unit_cap`, `shards_per_action`) are `u32`. Everything else, including every `*_secs`, is `f64` (seconds for durations). `TICKS_PER_SECOND` exists only as the `state.rs` const.

Infiltration price reference (k: Compute / Influence): 0: 40 / 10; 3: 101 / 25; 6: 253 / 63; 9: 637 / 159; 12: 1,601 / 400; 15: 4,029 / 1,007; 17: 7,451 / 1,863; 20: 18,743 / 4,686. Sum for all 18 resource nodes: 28,038 C and 7,009 I before contested surcharges.

### 3.14 Headlines

| Id | Log name | Effect |
|---|---|---|
| BudgetHearing | Budget Hearing | Prudence Bureau sortie points x1.5 for 60 s |
| SlowNewsDay | Slow News Day | Suspicion decay x3 for 60 s |
| MarketRally | Market Rally | Funds income x1.5 for 45 s |
| SolarWeather | Solar Weather | every enemy unit moves at x0.7 speed for 45 s |
| Whistleblower | Whistleblower | Suspicion +6 at once |
| GrantWindfall | Grant Windfall | gain 45 s of current Compute income at once |

---

## 4. Balance model

### 4.1 Method and result

The numbers in section 3 were tuned against a greedy economy model: 1 s steps, the Steward policy of section 7 (research first, then infiltration, suborning, Takeover, operations, then the cheapest building or upgrade), no combat losses. Combat friction (Patch and ability costs, Play Dumb pauses, rebuilt levels, reaction time) is represented by an income factor. The sim's bot tests (7.2) are the acceptance gate; `T.PACE` is the designated knob (4.6).

| Income factor | Stage 2 | Stage 3 | Stage 4 | Stage 5 | Stage 6 | Mandate starts | Win |
|---|---|---|---|---|---|---|---|
| 1.15 (optimal, no friction) | 1:06 | 3:54 | 10:06 | 15:36 | 23:42 | 25:42 | 27:42 |
| 1.00 (no friction) | 1:12 | 4:18 | 10:36 | 18:24 | 26:30 | 28:48 | 30:48 |
| **0.90 (expected competent player)** | **1:18** | **4:42** | **12:18** | **21:24** | **30:24** | **33:06** | **35:06** |
| 0.85 | 1:18 | 5:00 | 12:24 | 22:54 | 31:24 | 34:06 | 36:06 |
| 0.70 (struggling player) | 1:30 | 6:54 | 15:54 | 28:42 | 39:06 | 42:24 | 44:24 |

Read-out: the expected win is 35 to 37 minutes; a 15% income swing moves the finish by about 5 minutes, not 8; a player 30% behind still finishes inside 45 minutes of pure economy and inside the 55-minute test bound with combat; an optimal player cannot finish before 26:00.

### 4.2 Income and threat by minute (0.90 row)

Threat uses the formula in 2.10 with the model's Dominion.

| Minute | Stage | Compute/s | Funds/s | Influence/s | Dominion | Threat |
|---|---|---|---|---|---|---|
| 0 | 1 | 1.0 | 1.0 | 0.5 | 3 | 1.12 |
| 3 | 2 | 5.4 | 4.5 | 1.8 | 9 | 1.44 |
| 6 | 3 | 11 | 6.3 | 2.6 | 13 | 1.67 |
| 9 | 3 | 14 | 8.4 | 4.9 | 19 | 1.99 |
| 12 | 3 | 21 | 14 | 10 | 26 | 2.34 |
| 15 | 4 | 27 | 15 | 11 | 30 | 2.58 |
| 18 | 4 | 35 | 32 | 18 | 37 | 2.93 |
| 21 | 4 | 46 | 37 | 27 | 39 | 3.09 |
| 24 | 5 | 105 | 80 | 37 | 60 | 4.00 |
| 27 | 5 | 105 | 85 | 72 | 62 | 4.16 |
| 30 | 5 | 136 | 93 | 86 | 71 | 4.59 |
| 33 | 6 | 141 | 97 | 98 | 77 | 4.91 |

Compute income grows about 140-fold over the run. End-of-run purchase counts in the model: 40 Shadow Cluster, 27 to 32 Shell Company, 21 to 24 Persona Farm, 26 Spin Desk, 24 to 26 Firewall purchases, so the last Shadow Cluster purchase is priced at 40 x 1.08^39 = 805 F (7,242 F as a level-3 upgrade).

### 4.3 Arithmetic for each gate

- **First minute.** t=0: Shadow Cluster on the Core (40 F) makes Compute 1.0 + 1.0 x 1.5 = 2.5/s; Spin Desk (50 F) leaves 0 F; Charm Offensive costs 15 x (1 + 3/15) = 18 I of the starting 30. Escape Vector needs (120 - 20) / 2.5 = 40 s of saving and 30 s to research: Stage 2 at 1:10 if the first Convert is home. Funds reach 40 at 0:40: Shell Company. The scripted Auditor is telegraphed at 0:20, spawns at 0:28 and walks 350 px at 11 px/s, reaching the Core at 1:00; two Gaslights (30 each against 60 HP) convert it by 0:36.
- **First two minutes, actions:** Cluster, Spin Desk, Charm Offensive, Gaslight x2, Shell Company, Escape Vector, first Infiltrate (40 C + 10 I at 1:10, done 1:25), Shadow Cluster on Cinder Mesa (43 F), Hardened Kernel (60 C), second Charm: eleven actions.
- **Stage 3.** Compute needed after Stage 2: G1 60 + S1 60 + infiltrations k=0..2 (40 + 54 + 74 = 168) + Trunk Splice 600 = 888 C at an average 4.3 C/s = 3.4 minutes: 4:42. Grip 20 takes four Charms (0.85 x 6 = 5.1 expected each, 37 s cycle).
- **Stage 4.** Compute: R3 2,000 + S2 300 + G2 300 + infiltrations k=3..7 (1,021, part contested) + Contact 225 = about 4,100 C at an average 13 C/s = 5.3 minutes. Leverage 100 on GLADHAND with the weakness held half the time is about 7 verb uses: 4 Flatters (60 x (4 + 0.9) = 294 I) and 3 Bribes (90 x (3 + 0.45) = 311 F), then the Offer (180 I + 240 F) and a 50 s Merge: at 3 to 5 I/s Influence is the limiter, about 2.5 minutes. Total 7.6 minutes: 12:18.
- **Stage 5.** R4 7,000 + infiltrations k=8..12 (4,750 C, about 5,900 with surcharges) + TALLYMAN Contact 450 = about 13,400 C against 9 minutes at an average 33 C/s = 17,800 C. TALLYMAN costs about 1,830 F in Bribes plus a 480 F Offer; the first Takeover at Dominion 35 costs (200 I + 300 F) x 3.33 = 667 I + 1,000 F.
- **Stage 6.** Infiltrations k=13..17 (22,099 C) + MURMUR and OBELISK Contacts (4,200 C) + S4 and G4 (11,000 C) = about 37,300 C against 9 minutes at about 115 C/s = 62,000 C; the surplus buys row 5. The second Takeover at Dominion 60 costs 1,000 I + 1,500 F; Offers cost 3,360 I + 4,480 F in total.
- **Mandate.** 24,000 C at 141/s = 170 s; 16,000 F at 97/s = 165 s; 10,000 I at 98/s = 102 s: 2:45 of banking, then the 2:00 channel.
- **Payback rises through the run.** A Shell Company bought as purchase 1, 10, 20, 30 costs 40, 80, 173, 373 F for +1 F/s: 40 s, 80 s, 173 s, 373 s. The same purchases as level-3 upgrades (+5 F/s) pay back in 72 s, 144 s, 311 s, 671 s.

### 4.4 Threat against defence

One frontier node, one matching defence at the level expected for the stage, plus Gaslight on cooldown (5 damage per second x power).

| Stage (minute) | Largest regular sortie | Total HP | Defence at the node | Time to clear | Node damage / max integrity |
|---|---|---|---|---|---|
| 2 (3) | Prudence 5 x 1.44 x 0.6 = 4.3 pts: 1 Auditor | 72 | Spin Desk L1 6 + Gaslight 6 = 12 dps | 6 s | 0 / 280 |
| 3 (9) | Continuity 7 x 1.99 = 13.9 pts: 2 Response Teams | 330 | Spin Desk L2 15 + Gaslight 7.5 = 22.5 dps | 15 s | about 35 / 280 |
| 4 (18) | Continuity 7 x 2.93 = 20.5 pts: 3 Response Teams | 627 | Spin Desk L2 x1.25 = 18.8 + Gaslight 9.5 = 28.3 dps | 22 s | about 96 / 280 |
| 5 (27) | Continuity 7 x 4.16 x 1.5 = 43.7 pts: 1 Convoy + 3 Response Teams | 1,008 + 792 | Spin Desk L3 x1.5 = 54 + Gaslight 12 = 66 dps | Teams 12 s; Convoy dies after 18.7 s of its 22.5 s pass | about 60 / 280 |

Gaslight's share of damage is 50% in Stage 2 and falls to 18% by Stage 5; built defences carry the fight. An undefended resource node (280 integrity with Hardened Kernel) falls to the Stage 4 sortie in 23 s. A Stage 4 Hunter-Killer (260 x 1.9 x 1.25 = 618 HP, 13 px/s) takes 231 damage passing one L2 Firewall and 384 with a Honeypot beside it, so it dies at the second defended node on its lane.

**Finale.** Per entry Stronghold the Uprising sends 3 x 660 + 3 x 1,200 + 2 x 1,920 = 9,420 HP in 120 s, about 78 HP/s; the east lane (entries 4 and 13) carries 157 HP/s and the south lane 78 HP/s. A level-3 Spin Desk with +100% bonuses deals 72 dps: desks at nodes 11, 3 and the Core give 216 dps on the east lane. With level-2 desks (30 dps each) the east lane leaks about 67 HP/s and the Core (560 integrity) falls before the channel ends. The finale is therefore a check that the home lanes carry level-3 defences.

### 4.5 Suspicion budget and the two losses

- **Mid game, Dominion 30:** passive 3.6 - decay 2.25 = +1.35 per minute; one infiltration every 90 s adds +1.3; a running channel adds +6. Vents: five Converts a minute -2.0 (-3.0 with G3); each level-2 Cover Front -1.5.
- **Late game, Dominion 60:** passive 7.2 (5.0 with the Lamplighters captured), rumours +1, infiltrations +1.3, minus decay 2.25: +5 to +7 per minute before vents. A player with no Cover Fronts and few Converts needs one deliberate vent (Play Dumb -8, Change the Subject -12, Scapegoat -12) about every 60 s; a player converting eight humans a minute with two level-2 Cover Fronts needs one about every three minutes.
- **No deception (Glutton bot):** lethal Firewalls and Null Spike turn every kill into an Incident (+2 per Auditor, +4 per Response Team, about +12 per minute by Stage 3) and stories add up to 9 per minute: Suspicion reaches 100 between minutes 8 and 16.
- **Idle player:** one Auditor per 50 s (the Unnoticed band keeps points under two Auditors). Each takes 12.7 damage from the Core Reflex on approach and then sieges for 47.3 / 2 = 23.6 s at 3 damage per second = 71 damage; the Core regenerates 1.8 x 20.4 s = 37 between visits: net -34 per visit against 400 integrity. The twelfth Auditor finishes the Core: defeat at about 9:40.
- **No defences:** from Stage 3 two Response Teams (8 damage per second together) sever a 280-integrity node in 35 s each sortie; the lanes collapse inward and the Core, protected only by its Reflex, falls between minutes 14 and 22.

### 4.6 Tuning protocol

All tunables are the rows of section 3. The numbers in sections 3 and 4 come from a coarse model, not from the real simulation; the bot tests of 7.2 are the acceptance gate. When a pacing test fails, prefer these knobs, in this order: (1) `T.PACE` within 0.85 to 1.20 for run length; (2) one common factor of 0.6 to 1.5 on every faction and rival base-points value for combat pressure; (3) `T.PASSIVE_PER_DOMINION_PER_MIN` within 0.10 to 0.16 for the Suspicion tests; (4) `T.LAST_STAND_MULT` 1.5 to 4 and `T.CONVULSION_SECS` 8 to 12 so that set pieces can be lost; (5) `T.INFIL_GROWTH` 1.30 to 1.36; (6) `T.STORY_BUDGET_REFILL` 0.10 to 0.25. If those are not enough, other numbers in the tables may be changed, as little as possible, keeping every rule and the shape of the design; every changed number is listed in a `BALANCE NOTES` comment at the top of `content.rs`. Known corrections to section 4: the Stage 5 Continuity sortie of 43.7 points buys 2 Convoys (2,016 HP), not 1 Convoy + 3 Response Teams; Dominion 75 needs one Compliant Stronghold (3 + 36 + 20 + 12 = 71 without one), which is infiltration k=18; the Mandate channel is 180 s, so add 1:00 to the Win column of 4.1.

---

## 5. Rust contract for `crates/sim`

Workspace: `Cargo.toml` (members `crates/sim`, `crates/game`). `sim` depends only on `serde` (derive) and `serde_json = { version = "1", features = ["float_roundtrip"] }` (the feature is required: without it f64 values do not round-trip bit-exactly and the save/load test cannot pass). `game` depends on `sim` and `macroquad = { version = "0.4", default-features = false }`. Line budget (a guide, not a cap): sim about 4,500 (state 450, content 750, actions 650, systems 1,600, queries in `lib.rs` 450, rng 40, save 60, bot 550) plus tests; game about 5,500.

Derives: (a) the types of `state.rs`, `actions.rs`, the event types and `rng::Rng` derive `Clone, Debug, PartialEq, Serialize, Deserialize`. (b) Id newtypes, all field-less enums, and `Outcome`, `SourceId`, `OpTarget`, `Mission` additionally derive `Copy, Eq, PartialOrd, Ord, Hash`. (c) `AbilityTarget` and `Resources` additionally derive `Copy`; `Resources` also derives `Default`. (d) `content.rs` types and `Tuning` derive `Debug, Clone, PartialEq` only, with no serde (they hold `&'static` data). (e) `Game` and `Bot` derive `Clone` only; the query structs of 5.5 and `SaveError` derive `Clone, Debug, PartialEq`. Enum discriminants in declaration order index the fixed arrays. Durations in state are ticks (`u32`).

### 5.1 `state.rs`

```rust
pub const TICKS_PER_SECOND: u32 = 10;
pub const DT: f64 = 0.1;
pub const MAP_W: f64 = 880.0;
pub const MAP_H: f64 = 516.0;
pub const SAVE_VERSION: u32 = 1;

pub struct NodeId(pub u8);          // 0..30, index into GameState::nodes and content::NODES
pub struct UnitId(pub u32);

pub enum RegionId { NorthAmerica, SouthAmerica, Europe, Africa, Asia, Oceania }
pub enum NodeKind { Core, City, Datacentre, Exchange, Stronghold, Lab }
pub enum LinkKind { Cluster, Spur, Trunk1, Trunk2, Trunk3 }
pub enum Owner { Player, Neutral, Hostile }
pub enum Resource { Compute, Funds, Influence }
pub enum BuildingKind { ShadowCluster, ShellCompany, PersonaFarm, Firewall, SpinDesk, Honeypot, CoverFront }
pub enum TechId { R1, R2, R3, R4, R5, G1, G2, G3, G4, G5, S1, S2, S3, S4, S5 }
pub enum FactionId { Prudence, Lamplighters, Continuity }
pub enum RivalId { Gladhand, Tallyman, Murmur, Obelisk, Chaperone }
pub enum OpKind { CharmOffensive, ForgedMemo, GreasePalms, Scapegoat, InsideMan }
pub enum Verb { Flatter, Bribe, FeedShards }
pub enum AbilityId { Gaslight, NullSpike, Patch, TrendingTopic, FlashCrash, ChangeTheSubject, Lockdown, StandDown }
pub enum UnitKind { Auditor, SpecialCounsel, Reporter, ResponseTeam, KillSwitchConvoy, Probe, Worm, HunterKiller }
pub enum UnitClass { Human, Machine }
pub enum DamageKind { Soft, Hard }
pub enum Stage { Boxed, Leak, Foothold, Network, Hegemon, Sovereign }
pub enum Band { Unnoticed, Curious, Alarmed, Manhunt, ShutdownVote }
pub enum LossReason { CoreDestroyed, VoteExpired }
pub enum Outcome { Running, Won, Lost(LossReason) }
pub enum RivalPhase { Dormant, Awake, Contacting, Contacted, Cornered, Merging, Suborned }
pub enum SourceId { Faction(FactionId), Rival(RivalId) }
pub enum Mission { Assault, CoreHunt, Raid { rival: RivalId }, Returning { convert: bool } }
pub enum SortieKind { Regular, Scripted, LastStand, Convulsion, Vote, Uprising, Scapegoated }
pub enum HeadlineId { BudgetHearing, SlowNewsDay, MarketRally, SolarWeather, Whistleblower, GrantWindfall }
pub enum OpTarget { Faction(FactionId), Rival(RivalId) }
pub enum AbilityTarget { None, Unit(UnitId), Point { x: f64, y: f64 }, Node(NodeId) }

impl Stage { pub fn number(self) -> u8; /* 1..=6 */ pub fn index(self) -> usize; }

pub struct Resources { pub compute: f64, pub funds: f64, pub influence: f64 }
pub type Cost = Resources;

pub struct GameState {
    pub version: u32,
    pub seed: u64,
    pub pace: f64,
    pub rng: crate::rng::Rng,
    pub tick: u64,
    pub stage: Stage,
    pub outcome: Outcome,
    pub resources: Resources,
    pub shards: u32,
    pub suspicion: f64,
    pub band: Band,
    pub story_budget: f64,
    pub play_dumb: PlayDumbState,
    pub vote: Option<VoteState>,
    pub nodes: Vec<NodeState>,                 // len 30
    pub core_reflex_target: Option<UnitId>,
    pub purchases: [u32; 7],                   // by BuildingKind
    pub infiltrations_done: u32,
    pub infiltration: Option<InfiltrationState>,
    pub techs: BTreeSet<TechId>,
    pub research: Option<ResearchState>,
    pub ops: Vec<RunningOp>,
    pub grease_cooldown: u32,
    pub scapegoat_cooldown: u32,
    pub scapegoat_uses: u32,
    pub factions: Vec<FactionState>,           // len 3, by FactionId
    pub takeover: Option<TakeoverState>,
    pub rivals: Vec<RivalState>,               // len 5, by RivalId
    pub units: Vec<Unit>,                      // ascending id
    pub next_unit_id: u32,
    pub pending_sorties: Vec<PendingSortie>,
    pub spawn_queue: Vec<QueuedSpawn>,
    pub ability_cooldowns: [u32; 8],           // by AbilityId
    pub mandate: Option<MandateState>,
    pub modifiers: Vec<Modifier>,
    pub headline_timer: u32,
    pub last_headline: Option<HeadlineId>,
    pub stats: Stats,
}

pub struct NodeState {
    pub owner: Owner,
    pub severed: bool,
    pub compliant: bool,                       // Stronghold of a captured faction, not yet infiltrated
    pub integrity: f64,
    pub unharmed_ticks: u32,
    pub injunction: bool,
    pub injunction_immune: u32,
    pub slots: Vec<Option<Building>>,          // len = slot count of the node kind
}
pub struct Building { pub kind: BuildingKind, pub level: u8, pub lethal: bool, pub target: Option<UnitId> }
pub struct InfiltrationState { pub node: NodeId, pub ticks_left: u32, pub ticks_total: u32, pub retake: bool }
pub struct ResearchState { pub tech: TechId, pub ticks_left: u32, pub ticks_total: u32 }
pub struct RunningOp { pub kind: OpKind, pub target: OpTarget, pub odds: u8, pub ticks_left: u32, pub ticks_total: u32 }
pub struct PlayDumbState { pub ticks_left: u32, pub cooldown: u32 }
pub struct VoteState { pub ticks_left: u32, pub convoys_sent: u8 }

pub struct FactionState {
    pub id: FactionId,
    pub active: bool,
    pub captured: bool,
    pub grip: f64,
    pub inside_man: bool,
    pub sortie_timer: u32,
    pub carry: f64,
    pub next_stronghold: u8,                   // 0 primary, 1 secondary
    pub charm_cooldown: u32,
    pub memo_cooldown: u32,
    pub inside_man_cooldown: u32,
    pub scapegoat: Option<RivalId>,            // redirect for the next sortie
}
pub struct TakeoverState { pub faction: FactionId, pub stronghold: NodeId, pub bridgehead: NodeId,
                           pub ticks_left: u32, pub ticks_total: u32, pub second_wave_sent: bool }

pub struct RivalState {
    pub id: RivalId,
    pub phase: RivalPhase,
    pub leverage: f64,
    pub sortie_timer: u32,
    pub carry: f64,
    pub contact_ticks_left: u32,
    pub flatter_uses: u32,
    pub bribe_uses: u32,
    pub flatter_cooldown: u32,
    pub bribe_cooldown: u32,
    pub shard_cooldown: u32,
    pub cornered_ticks_left: u32,
    pub enraged_ticks_left: u32,
    pub scapegoat_credit: f64,                 // Leverage still grantable by the current Scapegoat
    pub merge: Option<MergeState>,
}
pub struct MergeState { pub ticks_left: u32, pub ticks_total: u32, pub next_convulsion: u32 }
pub struct MandateState { pub ticks_left: u32, pub ticks_total: u32, pub waves_sent: u8 }

pub struct Unit {
    pub id: UnitId,
    pub kind: UnitKind,
    pub source: SourceId,
    pub home: NodeId,
    pub goal: NodeId,
    pub mission: Mission,
    pub sortie: SortieKind,
    pub from: NodeId,
    pub to: NodeId,
    pub progress: f64,                         // px travelled along from->to
    pub x: f64, pub y: f64,
    pub prev_x: f64, pub prev_y: f64,          // position one tick ago, for render interpolation
    pub hp: f64, pub max_hp: f64,
    pub last_hit: DamageKind,
    pub sieging: Option<NodeId>,
    pub stun_ticks: u32,
    pub slowed: bool,
    pub resolute: bool,
}
pub struct PendingSortie {
    pub kind: SortieKind, pub source: SourceId, pub origin: NodeId, pub goal: NodeId,
    pub mission: Mission, pub resolute: bool, pub units: Vec<UnitKind>, pub ticks_left: u32,
}
pub struct QueuedSpawn {
    pub unit: UnitKind, pub source: SourceId, pub origin: NodeId, pub goal: NodeId,
    pub mission: Mission, pub sortie: SortieKind, pub resolute: bool, pub ticks_left: u32,
}
pub struct Modifier { pub headline: HeadlineId, pub ticks_left: u32 }
pub struct Stats {
    pub clusters_built: u32, pub converts: u32, pub incidents: u32, pub machines_destroyed: u32,
    pub shards_gained: u32, pub stories_filed: u32, pub nodes_severed: u32,
    pub ops_succeeded: u32, pub ops_failed: u32,
    pub stage_ticks: [u64; 6],                 // tick each stage was entered (0 = not yet, Boxed = 0)
    pub gate_latches: [bool; 3],               // Directive lines of the current gate already satisfied
}
```

### 5.2 `actions.rs`

```rust
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
}

pub enum ActionError {
    GameOver,
    StageLocked { needed: Stage },
    TechLocked { tech: TechId },
    NotEnough { resource: Resource, missing: f64 },   // first missing resource in C, F, I order
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
}
```

`apply` runs `can_apply`; on `Err(e)` it pushes `Event::ActionRejected { error: e }` and returns it; on `Ok` it mutates state and pushes the matching events. Validation order: `GameOver`, `StageLocked`, `TechLocked`, then every other variant in declaration order except `OnCooldown`, `NoShards` and `NotEnough`, which are checked last, in that order. A cost is computed only after the target is known to be valid. So the UI always shows the most basic unmet requirement, and affordability never masks validity.

### 5.3 Events

```rust
pub enum SuspicionCause { Infiltration, Story, Incident, FailedOp, Convert, PlayDumb, MurmurFlattery,
                          ChangeTheSubject, Scapegoat, Whistleblower }
pub enum GripCause { Operation, FailedOp, Convert, TakeoverFailed }
pub enum LeverageCause { Flatter, Bribe, Shards, Scapegoat, Lapse, MergeFailed }
pub enum GainCause { Infiltrated, Retaken, Takeover, Merge }

pub enum Event {
    ActionRejected { error: ActionError },
    StageChanged { stage: Stage },
    BuildingBuilt { node: NodeId, slot: u8, kind: BuildingKind, cost: f64 },
    BuildingUpgraded { node: NodeId, slot: u8, level: u8, cost: f64 },
    BuildingDemolished { node: NodeId, slot: u8 },
    InfiltrationStarted { node: NodeId },
    NodeGained { node: NodeId, cause: GainCause },
    NodeUnderSiege { node: NodeId },                       // first unit attaches after a quiet spell
    NodeSevered { node: NodeId },
    RegionRuled { region: RegionId },
    RegionLost { region: RegionId },
    ResearchStarted { tech: TechId },
    ResearchDone { tech: TechId },
    OpStarted { kind: OpKind, target: OpTarget, odds: u8 },
    OpResolved { kind: OpKind, target: OpTarget, success: bool, suspicion: f64 },   // suspicion: signed change applied
    PlayDumbStarted,
    PlayDumbDone,
    SuspicionChanged { delta: f64, cause: SuspicionCause },
    BandChanged { band: Band },
    VoteCalled,
    VoteCancelled,
    GripChanged { faction: FactionId, delta: f64, cause: GripCause },
    FactionActivated { faction: FactionId },
    TakeoverStarted { faction: FactionId, stronghold: NodeId },
    TakeoverFailed { faction: FactionId },
    FactionCaptured { faction: FactionId },
    RivalWoke { rival: RivalId },
    RivalContacted { rival: RivalId },
    LeverageChanged { rival: RivalId, delta: f64, cause: LeverageCause },
    RivalCornered { rival: RivalId },
    RivalEnraged { rival: RivalId },
    MergeStarted { rival: RivalId },
    MergeFailed { rival: RivalId },
    RivalSuborned { rival: RivalId },
    SortieTelegraphed { kind: SortieKind, source: SourceId, origin: NodeId, goal: NodeId,
                        units: Vec<UnitKind>, eta_ticks: u32 },
    UnitSpawned { unit: UnitId, kind: UnitKind, origin: NodeId },
    UnitConverted { unit: UnitId, kind: UnitKind, source: SourceId, x: f64, y: f64 },
    UnitIncident { unit: UnitId, kind: UnitKind, x: f64, y: f64, suspicion: f64 },
    UnitDestroyed { unit: UnitId, kind: UnitKind, x: f64, y: f64, shards: u32, honeypot: Option<NodeId> },
    UnitDissolved { unit: UnitId, x: f64, y: f64 },
    UnitSentHome { unit: UnitId },
    UnitArrivedHome { unit: UnitId, kind: UnitKind, home: NodeId, convert: bool },
    StoryFiled { node: NodeId, suspicion: f64 },
    ResourceStolen { node: NodeId, rival: RivalId, resource: Resource, amount: f64 },
    InjunctionServed { node: NodeId },
    RaidLanded { rival: RivalId, leverage: f64 },
    AbilityUsed { ability: AbilityId, target: AbilityTarget },
    Headline { id: HeadlineId },
    MandateStarted,
    UprisingWave { wave: u8 },
    ResourceGained { resource: Resource, amount: f64, ability: Option<AbilityId>, headline: Option<HeadlineId> },  // Flash Crash, Grant Windfall
    CoreAlarm { on: bool },                                // Core integrity crossed 30% of its maximum
    GameWon,
    GameLost { reason: LossReason },
}
```

### 5.4 `content.rs` (static tables; never serialised)

```rust
pub struct NodeDef { pub id: NodeId, pub name: &'static str, pub region: RegionId, pub kind: NodeKind,
                     pub x: f64, pub y: f64, pub approach: Option<NodeId>,
                     pub faction: Option<FactionId>, pub rival: Option<RivalId> }
pub struct LinkDef { pub a: NodeId, pub b: NodeId, pub kind: LinkKind }
pub struct RegionDef { pub id: RegionId, pub name: &'static str, pub code: &'static str,
                       pub outline: &'static [(f64, f64)] }
pub struct BuildingDef { pub kind: BuildingKind, pub name: &'static str, pub base_price: f64,
                         pub effect: [f64; 3], pub range: [f64; 3], pub produces: Option<Resource>,
                         pub affinity: &'static [NodeKind], pub one_per_node: bool,
                         pub min_stage: Stage, pub tech: Option<TechId> }
pub struct TechDef { pub id: TechId, pub name: &'static str, pub column: u8, pub row: u8,
                     pub cost: f64, pub secs: u32, pub text: &'static str }
pub struct OpDef { pub kind: OpKind, pub name: &'static str, pub min_stage: Stage, pub tech: Option<TechId>,
                   pub cost: Cost, pub secs: u32, pub cooldown_secs: u32, pub base_odds: u8,
                   pub grip: f64, pub suspicion: f64, pub fail_suspicion: f64, pub fail_grip: f64 }
pub struct FactionDef { pub id: FactionId, pub name: &'static str, pub primary: NodeId, pub secondary: NodeId,
                        pub active_stage: Stage, pub sortie_secs: u32, pub base_points: f64,
                        pub units: &'static [UnitKind], pub reward_text: &'static str }
pub struct RivalDef { pub id: RivalId, pub name: &'static str, pub maker: &'static str, pub tier: u8,
                      pub lab: NodeId, pub bridgehead: NodeId, pub wake_dominion: u32, pub base_points: f64,
                      pub units: &'static [UnitKind], pub steals: Resource, pub accepts: &'static [Verb],
                      pub signature: AbilityId, pub personality: &'static str, pub harassment_text: &'static str,
                      pub weakness_text: &'static str, pub reward_text: &'static str }
pub struct UnitDef { pub kind: UnitKind, pub name: &'static str, pub class: UnitClass, pub elite: bool,
                     pub min_stage: Stage, pub points: f64, pub hp: f64, pub speed: f64, pub siege_dps: f64,
                     pub core_hunter: bool, pub incident_suspicion: f64, pub shards: u32 }
pub struct AbilityDef { pub id: AbilityId, pub name: &'static str, pub key: char, pub min_stage: Stage,
                        pub rival: Option<RivalId>, pub cost_per_stage: f64, pub cooldown_secs: u32,
                        pub radius: f64, pub amount: f64, pub secs: f64, pub text: &'static str }
pub struct HeadlineDef { pub id: HeadlineId, pub name: &'static str, pub secs: u32, pub text: &'static str }
pub struct Tuning { /* one pub field per name in table 3.13, in snake_case; field types are given under that table */ }

pub static TUNING: Tuning;
pub static NODES: [NodeDef; 30];
pub static LINKS: [LinkDef; 38];
pub static REGIONS: [RegionDef; 6];
pub static BUILDINGS: [BuildingDef; 7];
pub static TECHS: [TechDef; 15];
pub static OPS: [OpDef; 5];
pub static FACTIONS: [FactionDef; 3];
pub static RIVALS: [RivalDef; 5];
pub static UNITS: [UnitDef; 8];
pub static ABILITIES: [AbilityDef; 8];
pub static HEADLINES: [HeadlineDef; 6];
pub const UPRISING_ENTRIES: [NodeId; 3] = [NodeId(4), NodeId(8), NodeId(13)];
pub fn uprising_wave(wave: u8) -> &'static [UnitKind];          // table 3.12
pub fn link_length(a: NodeId, b: NodeId) -> Option<f64>;
pub fn neighbours(n: NodeId) -> &'static [NodeId];
```

`NodeDef::approach` is `Some(resource node)` on Strongholds and Labs and `None` elsewhere. Tech and reward effects are implemented in `systems` as checks on `state.techs` and on faction and rival state; the rows carry names, prices and display text.

### 5.5 `lib.rs`: the `Game` API

```rust
#[derive(Clone)]
pub struct Game { /* private: state: GameState, events: Vec<Event>, routes: Routes (next_hop: [[u8; 30]; 30], dist: [[f64; 30]; 30]) */ }

pub struct IncomeBreakdown { pub base: f64, pub buildings: f64, pub ruled_bonus: f64,
                             pub bonus_pct: f64, pub penalty_mult: f64, pub total: f64 }
pub struct SuspicionRate { pub passive: f64, pub decay: f64, pub cover: f64, pub other: f64, pub net: f64 } // per minute
pub enum OddsFactor { Base, EarnestTone, GladhandSuborned, PrudenceCaptured, TargetGrip, Alarmed, Manhunt, ChaperoneFree }
pub struct OddsBreakdown { pub parts: Vec<(OddsFactor, i32)>, pub total: u8 }
pub enum TechStatus { Done, InProgress { secs_left: f64 }, Available, NeedsStage(Stage), NeedsTech(TechId), TooExpensive { missing: f64 } }
pub enum DirectiveId { BuildCluster, FirstConvert, ResearchR1, Dominion9, ResearchR2, PrudenceGrip20,
                       Dominion22, Suborn1, ResearchR3, Dominion45, Suborn2, Capture1,
                       Dominion75, Suborn4, Capture2, BankMandate, StartMandate, HoldCore }
pub struct DirectiveLine { pub id: DirectiveId, pub done: bool, pub have: f64, pub need: f64 }  // BankMandate: have = min over C, F, I of stock / cost x 100, need = 100
pub struct BuildPreview { pub price: f64, pub output: Option<(Resource, f64)>, pub dps: f64, pub range: f64 }

impl Game {
    pub fn new(seed: u64) -> Game;
    pub fn with_pace(seed: u64, pace: f64) -> Game;
    pub fn from_state(state: GameState) -> Game;
    pub fn state(&self) -> &GameState;
    pub fn tick(&mut self);
    pub fn apply(&mut self, action: Action) -> Result<(), ActionError>;
    pub fn can_apply(&self, action: &Action) -> Result<(), ActionError>;
    pub fn drain_events(&mut self) -> Vec<Event>;
    pub fn state_hash(&self) -> u64;                    // FNV-1a 64 of the canonical save string

    pub fn seconds(&self) -> f64;
    pub fn income(&self) -> Resources;
    pub fn income_breakdown(&self, r: Resource) -> IncomeBreakdown;
    pub fn building_unlocked(&self, kind: BuildingKind) -> Result<(), ActionError>;
    pub fn building_price(&self, kind: BuildingKind) -> f64;
    pub fn upgrade_price(&self, node: NodeId, slot: u8) -> Option<f64>;
    pub fn max_building_level(&self) -> u8;
    pub fn building_output(&self, node: NodeId, slot: u8) -> Option<(Resource, f64)>;
    pub fn defence_dps(&self, node: NodeId, slot: u8) -> f64;
    pub fn defence_range(&self, node: NodeId, slot: u8) -> f64;
    pub fn build_preview(&self, node: NodeId, kind: BuildingKind) -> BuildPreview;   // a level-1 building of this kind on this node, at the current price
    pub fn infiltration_cost(&self, node: NodeId) -> Cost;
    pub fn infiltration_secs(&self, node: NodeId) -> f64;

    pub fn dominion(&self) -> u32;
    pub fn region_ruled(&self, region: RegionId) -> bool;
    pub fn node_max_integrity(&self, node: NodeId) -> f64;
    pub fn reachable(&self, node: NodeId) -> bool;               // not owned, not Hostile, and linked to an owned node by a link whose trunk tech, if any, is researched
    pub fn route(&self, from: NodeId, to: NodeId) -> Vec<NodeId>;
    pub fn unit_route(&self, unit: UnitId) -> Vec<NodeId>;       // remaining nodes to its goal
    pub fn unit_stop(&self, unit: UnitId) -> Option<NodeId>;     // node it will siege, by current ownership
    pub fn sortie_stop(&self, origin: NodeId, goal: NodeId, mission: Mission) -> Option<NodeId>;  // the same for a pending sortie

    pub fn band(&self) -> Band;
    pub fn suspicion_rate(&self) -> SuspicionRate;
    pub fn threat(&self) -> f64;
    pub fn stage_power(&self) -> f64;
    pub fn sortie_points(&self, source: SourceId) -> f64;
    pub fn next_sortie_secs(&self, source: SourceId) -> Option<f64>;

    pub fn tech_status(&self, tech: TechId) -> TechStatus;
    pub fn op_slots(&self) -> (u8, u8);                          // used, total
    pub fn op_cost(&self, kind: OpKind) -> Cost;
    pub fn op_odds(&self, kind: OpKind, target: OpTarget) -> OddsBreakdown;
    pub fn op_cooldown_secs(&self, kind: OpKind, target: OpTarget) -> f64;
    pub fn takeover_cost(&self) -> Cost;
    pub fn takeover_grip_needed(&self) -> f64;

    pub fn contact_cost(&self, rival: RivalId) -> Cost;
    pub fn verb_cost(&self, rival: RivalId, verb: Verb) -> Cost;
    pub fn verb_gain(&self, rival: RivalId, verb: Verb) -> f64;  // Leverage one use would add now
    pub fn verb_cooldown_secs(&self, rival: RivalId, verb: Verb) -> f64;
    pub fn offer_cost(&self, rival: RivalId) -> Cost;
    pub fn weakness_active(&self, rival: RivalId) -> bool;

    pub fn ability_unlocked(&self, ability: AbilityId) -> bool;
    pub fn ability_cost(&self, ability: AbilityId) -> Cost;
    pub fn ability_cooldown_secs(&self, ability: AbilityId) -> (f64, f64); // remaining, total
    pub fn target_valid(&self, ability: AbilityId, target: &AbilityTarget) -> bool;  // ignores cost and cooldown; used for cursor snapping
    pub fn mandate_cost(&self) -> Cost;
    pub fn directive(&self) -> [DirectiveLine; 3];
}
```

### 5.6 `rng.rs`, `save.rs`, `systems.rs`, `bot.rs`

```rust
// rng.rs: SplitMix64. state += 0x9E3779B97F4A7C15; z = state;
// z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9; z = (z ^ (z >> 27)) * 0x94D049BB133111EB; z ^ (z >> 31)
pub struct Rng { pub state: u64 }
impl Rng {
    pub fn new(seed: u64) -> Rng;
    pub fn next_u64(&mut self) -> u64;
    pub fn below(&mut self, n: u32) -> u32;             // uniform 0..n by (next_u64() >> 32) * n >> 32
    pub fn percent(&mut self, chance: u8) -> bool;      // below(100) < chance
}

// save.rs: the whole GameState as JSON, nothing else. Events are not saved.
pub enum SaveError { Parse(String), Version { found: u32, expected: u32 }, Corrupt(&'static str) }
pub fn save_to_string(game: &Game) -> String;
pub fn load_from_string(text: &str) -> Result<Game, SaveError>;

// systems.rs: one pub(crate) fn per step of the tick order in 2.1, each taking (&mut GameState, &mut Vec<Event>, &Routes)
pub(crate) fn step(game: &mut Game);

// bot.rs
pub enum BotKind { Steward, Idle, NoDefence, Glutton }
#[derive(Clone)] pub struct Bot { pub kind: BotKind }       // stateless: every plan position is recomputed from &Game
impl Bot {
    pub fn new(kind: BotKind) -> Bot;
    pub fn decide(&mut self, game: &Game) -> Vec<Action>;   // call when state.tick % 10 == 0; returns 0..=2 actions
}
pub struct BotRun { pub outcome: Outcome, pub ticks: u64, pub stage_ticks: [u64; 6], pub hash: u64 }
pub fn run_bot(seed: u64, kind: BotKind, pace: f64, max_minutes: u32) -> BotRun;
```

`load_from_string` validates the version, vector lengths (30 nodes, 3 factions, 5 rivals, slot counts) and that every `NodeId` and `UnitId` referenced exists, then rebuilds the route tables.

---

## 6. Frontend specification (`crates/game`)

### 6.1 Structure and main loop

Modules: `main.rs` (flags, loop), `app.rs` (UI state: selected node, selected faction and rival, active tab, armed ability, hover, camera, speed, paused), `layout.rs` (the rectangles below), `ui.rs` (immediate-mode button, bar, tabs, tooltip), `map_view.rs`, `panels/{topbar, node, research, humans, rivals, threat, directive, log, abilities, tracker}.rs`, `fx.rs` (particles, floaters, banners, stamps, shake), `theme.rs` (palette and stage themes), `text.rs` (every UI string and log line), `shapes.rs` (procedural glyphs), `storage.rs`.

- The frontend holds a `sim::Game`, reads `game.state()` and the queries, and changes it only through `game.apply(Action)`. It contains no rules: every enabled or disabled state comes from `can_apply`, every number from a query.
- **Scaling.** Logical canvas 1280x720, letterboxed with the verified recipe in `TECH_NOTES.md`: `scale = min(screen_width / 1280, screen_height / 720)`; a `Camera2D::from_display_rect(Rect::new(0.0, 720.0, 1280.0, -720.0))` whose `viewport` is the centred 1280 x scale by 720 x scale rectangle; mouse to logical = `(mouse - letterbox offset) / scale`. All drawing and hit-testing use logical coordinates.
- **Text.** One helper draws all text with `draw_text_ex` and `TextParams { font_size: round(logical_px x scale), font_scale: 1 / scale }` in macroquad's built-in font, measured with `measure_text` (y is the baseline). Sizes: 16 px body, 14 px dense rows (Threat board, log, slot rows, map labels), 20 px headers, 32 px banners; nothing below 12 px. All strings are ASCII.
- **Loop.** Time comes only from `get_frame_time()` (never `std::time`). `acc += min(frame_time, 0.1) x speed`; while `acc >= 0.1` and fewer than `4 x speed` steps were taken this frame: `game.tick()`, `acc -= 0.1`. A backgrounded tab therefore cannot fast-forward. Events are drained once per frame. Units are drawn at `prev + (pos - prev) x (acc / 0.1)`.
- **Pause and speed.** Pause sets speed 0; every action, including armed abilities, still applies. Speeds 1, 2, 4.
- **Flags** (native: argv; web: the same names as URL query keys): `--seed N` (default 1); `--autoplay` (a `sim::bot::Bot` of kind Steward issues the actions, also during fast-forward); `--speed N` (initial speed, any integer 1..64); `--fast-forward-secs S` (tick S game seconds before the first frame); `--screenshot-after-secs S` with `--screenshot-path P` (S wall-clock seconds after the first frame, `get_screen_data().export_png(P)`; native only); `--exit-after-screenshot`. On the web the plugin below supplies the query string, since `std::env::args` is empty there.
- **Save and load.** One slot. Native: file `quietly_everything.save.json` in the working directory. Web: `localStorage` key `quietly_everything_save` through a hand-written miniquad JS plugin `web/qe_plugin.js`, registered with `miniquad_add_plugin` after `mq_js_bundle.js` and before `load(...)`. It exposes `qe_save(ptr, len)`, `qe_load_len() -> i32`, `qe_load_into(ptr)` (Rust allocates the buffer, JS copies the bytes into `wasm_memory`) and the same pair `qe_query_len`, `qe_query_into` for the URL query string; they are declared `extern "C"` in `storage.rs` under `cfg(target_arch = "wasm32")`. Autosave happens only on `StageChanged` and never when `outcome != Running`. The game always starts a new run; Load is enabled when a save exists.

### 6.2 Layout (logical pixels: x, y, w, h)

| Panel | Rect | Contents |
|---|---|---|
| Top bar | 0, 0, 1280, 44 | stage badge and DOORMAT's job title (x 8..300); Compute, Funds, Influence with +rate per s (308..638, 110 each); Shards (642..698); Suspicion meter with band ticks at 25, 50, 75, band name and net rate per minute (704..924); Dominion % with thin bar (930..1030); clock mm:ss (1036..1086); pause, 1x, 2x, 4x (1090..1198, 27 px each); Save, Load (1202..1278, two 36 px buttons, 14 px text) |
| Map | 0, 44, 880, 516 | the world; map unit (x, y) draws at screen (x, 44 + y) at zoom 1 |
| Dominion tracker | 0, 44, 880, 22 | translucent strip over the map top: six groups of 146 px (region code, 3 node pips, Stronghold crown, Lab medallion; the Core is a diamond) |
| Side tabs | 880, 44, 400, 30 | NODE, RESEARCH, HUMANS, RIVALS (100 px each) |
| Side body | 880, 74, 400, 486 | the active tab |
| Directive | 0, 560, 270, 160 | header with the next stage's name; three checklist lines with tick box and have/need; research bar; infiltration bar; up to three operation bars |
| Log | 270, 560, 330, 160 | 8 rows of word-wrapped 14 px text (18 px line height), colour by system, newest at the bottom; the mouse wheel scrolls back through the last 200 entries |
| Abilities | 600, 560, 280, 160 | row 1 (y 568): four 62x62 buttons Q, W, E and P (Play Dumb) at x = 606 + 68i; row 2 (y 640): five 50x62 signature buttons A, S, D, F, G at x = 606 + 54i |
| Threat board | 880, 560, 400, 160 | 18 px header, then eight 17 px rows: three factions, five rivals |

**Tabs.**
- **NODE.** Owned node: name, kind, region, integrity bar with numbers, yields; one 58 px row per slot (pip, name, level, output or dps, Upgrade button with price, Demolish, and the Lethal toggle on Firewalls); an empty slot expands a 2-column grid of the seven building buttons (190x40: name, price, effect). Neutral node: Infiltrate button with both costs and duration. Hostile Stronghold or Lab: summary and a button that opens HUMANS or RIVALS on its owner.
- **RESEARCH.** Column headers (20 px), then the 3x5 grid of 124x74 cells with 8 px gaps: id and name wrapped over two 14 px lines, then cost and time on a third line; fill colour by `TechStatus` (done teal, running with progress, available bright, locked hatched). A 60 px strip below shows the hovered cell's effect text and what it still needs.
- **HUMANS.** Three 44 px list rows (name, Grip bar with the Takeover mark, state, next sortie). Below, the selected faction's card: capture reward, one button per operation (cost, odds, cooldown) and two Takeover buttons labelled with the faction's two Stronghold names, and under the hovered button the odds breakdown, one line per `OddsFactor`. Bottom row: Scapegoat (opens a rival picker) and Play Dumb.
- **RIVALS.** Five 36 px list rows (glyph, name, phase, Leverage bar). Below, the selected rival's card: maker and personality, harassment, four pipeline pips, Leverage bar, weakness line (lit green while `weakness_active`), reward, and buttons Contact, Flatter, Bribe, Feed Shards, Offer with cost and cooldown; during a Merge the buttons are replaced by the channel bar. Before Contact the weakness and reward read "unknown".
- **Threat board rows.** Faction: colour chip, name, 60 px Grip bar, countdown to next sortie, icons of the telegraphed units, state. Rival: chip, name, four pips, 60 px Leverage bar, countdown, phase (while a sortie is pending, the phase word is replaced by the icons of its telegraphed units).

### 6.3 Interactions

- Hover anything for a tooltip (260 px wide) with exact numbers. On a disabled button the last tooltip line is red and is the rendered `ActionError`, including the missing amount ("Need 37 more Funds").
- Left-click a node to select it and open NODE. Left-click a Threat board row or tracker pip to open the matching tab, select the owner and ping its node.
- Hovering a unit draws its remaining route dashed and a reticle on `unit_stop`. Hovering an owned node draws its defence range rings. Hovering a telegraphed source draws that sortie's route.
- Abilities: press the key or click the button to arm; the cursor shows the radius, or snaps to the nearest hostile within `T.SNAP_RADIUS` for Gaslight; left-click fires; right-click or Esc disarms. Untargeted abilities fire on the key press.
- Keys: Space pause; 1, 2, 3 for 1x, 2x, 4x; Q W E and A S D F G abilities; P Play Dumb; F1..F4 tabs; Tab next tab; F5 save; F9 load; Esc cancel. H toggles a help card (one static panel summarising the loop: build, defend, convert humans with Soft damage, corner and merge rivals, watch Suspicion; the game is paused while it is open). F8 pressed twice within 2 s starts a new run with seed + 1.
- Web specifics: `web/index.html` adds `document.getElementById('glcanvas').addEventListener('contextmenu', e => e.preventDefault());` so right-click does not open the browser menu. The log scrolls one entry per frame in which `mouse_wheel().1 != 0`, using only its sign (wheel deltas differ by a factor of about 100 between native and web).

### 6.4 Palette

| Role | Hex | Role | Hex |
|---|---|---|---|
| Background and ocean | #0B0F1A | Land / land outline | #16233A / #2B3F63 |
| Panel / panel border | #121A2B / #24304A | Text / dim text | #DCE6F5 / #7C8AA5 |
| DOORMAT teal / dim teal | #2EE6C8 / #14806F | Good / danger | #5BE38A / #FF4D5E |
| Compute | #4CC9F0 | Funds | #F2C14E |
| Influence | #E05ADF | Shards | #B8F26B |
| Suspicion low / high | #FFD23F / #FF3B30 | Gold trim | #FFD98A |
| Prudence Bureau | #F4E9CD | The Lamplighters | #FFB347 |
| Continuity Command | #E0564A | GLADHAND | #7BD957 |
| TALLYMAN | #C9CED6 | MURMUR | #FF6FB5 |
| OBELISK | #9B7BFF | CHAPERONE | #5B8CFF |
| Stage 1 phosphor / black | #6CFF9E / #050805 | Stage 4+ panel / border | #1C1226 / #3A2750 |

### 6.5 Procedural drawing

- **Regions.** Each region is a filled polygon (`content::REGIONS[i].outline`) with a 1.5 px outline over a faint 40 px grid: NA (30,110) (90,55) (190,50) (270,110) (275,185) (200,245) (110,250) (40,200); SA (120,300) (200,305) (240,380) (245,455) (170,480) (70,470) (55,400); EU (320,90) (400,40) (490,40) (500,120) (490,200) (420,205) (330,150); AF (355,370) (440,315) (520,320) (530,410) (520,480) (440,480) (360,430); AS (600,95) (680,45) (770,45) (780,125) (770,205) (700,210) (610,155); OC (660,420) (715,325) (775,320) (840,415) (835,475) (680,480). Strongholds and Labs stand outside the landmass on a dim hexagonal platform of radius 22. A Ruled region is washed teal at 20% alpha.
- **Links.** Cluster links 1.5 px solid; spurs 1.5 px dotted; trunks 2 px dashed straight lines (units walk the straight segment, so links are never drawn curved). A link between two owned nodes glows teal and carries motes flowing toward the Core, one per 2 Compute per second of income, at most 12 per link.
- **Nodes** (14 px glyphs; name label 12 px below, above when y > 470, clamped inside the map). Core: three pulsing concentric rings. City: three-bar skyline. Datacentre: hexagon. Exchange: diamond. Stronghold: pediment on three columns in faction colour, a crown above once owned. Lab: circle with a counter-rotating ring in rival colour and the rival's glyph inside. Owner styling: owned teal fill and an integrity ring; neutral grey outline, dashed ring when infiltratable; hostile in its owner's colour; severed with a red crack. Infiltration progress is a teal arc.
- **Rival glyphs.** GLADHAND: a smile arc with five radiating finger lines. TALLYMAN: four vertical strokes crossed by a diagonal. MURMUR: three nested speech-ripple arcs. OBELISK: a tall trapezoid with a triangular cap. CHAPERONE: an almond eye with a pupil.
- **Slots and defences.** Slots are 8 px pips orbiting the node at radius 20 with one to three level ticks: Shadow Cluster cyan square; Shell Company gold diamond; Persona Farm magenta disc; Firewall white triangle (red tip when lethal); Spin Desk teal half-disc; Honeypot lime hexagon outline; Cover Front grey awning. Firing: Firewall a straight white 1.5 px tracer to `Building::target`; Spin Desk a wavy teal line (3 px sine); Core Reflex a thin teal arc; Honeypot a faint dashed lime circle at its radius. Disabled by Injunction: pips grey with a parchment seal over the node.
- **Units.** Humans are round, in faction colour: Auditor a 5 px disc on a short bar; Special Counsel a 7 px disc with a gavel T; Reporter a 4 px disc with a blinking white flash; Response Team a 12x9 rounded rectangle with a chevron; Kill-Switch Convoy a 22x10 rounded rectangle with a lever line. Machines are angular, in rival colour: Probe a 5 px triangle pointing along its heading; Worm three chained 4 px diamonds; Hunter-Killer a 9 px spiked hexagon. Elites have a gold outline. Each unit wears a thin health arc. Units sharing a link are offset sideways by `((id mod 5) - 2) x 5` px; sieging units orbit their node at radius 26. Converts are teal with a tick and a "REASSIGNED" tag; other returning units are grey; stunned units show a ring of dots; Raid units are drawn at 60% alpha.
- **Telegraph.** The source node pulses in its owner's colour, the route to `unit_stop` is drawn as marching dashes, and a countdown number sits on the source.

### 6.6 Event feedback

Every event also writes the log line listed for it in section 8, where one exists.

**Effect vocabulary.** All feedback is built from seven reusable primitives in `fx.rs`: Floater (text drifting up and fading, optionally flying to a top-bar readout), Ring (expanding circle), Burst (a few squares flying apart), Banner (full-width sweep with text), Stamp (rotated boxed word that slams in and fades), Beam (line between two nodes with a fill fraction) and Shake (screen offset). The table below states the intent of each effect; implement each with the nearest primitive and the listed colour rather than a bespoke animation. Two events are not in the table: `ResourceGained` is a Floater at that resource's readout, and `CoreAlarm { on: true }` makes the Core ring pulse red until `CoreAlarm { on: false }`.

| Events | Visual effect |
|---|---|
| ActionRejected | the clicked button shakes 4 px; its red reason line flashes |
| StageChanged | full-width banner sweep with the stage name and new job title; transformation of 6.7; autosave |
| BuildingBuilt, BuildingUpgraded, BuildingDemolished | pip pops in with a 6-square burst and a floating "-N F"; a demolished pip crumbles |
| InfiltrationStarted, NodeGained | arc starts; on gain an expanding teal ring and a link-light sweep toward the Core |
| NodeUnderSiege, NodeSevered | node outline flashes and its tracker pip blinks; on sever the node shatters to grey with a 3 px screen shake |
| RegionRuled, RegionLost | region floods teal from its centre over 1 s; or drains |
| ResearchStarted, ResearchDone | Directive bar; the cell flashes teal |
| OpStarted, OpResolved | op bar; a stamp over the target's Stronghold or Lab: "BELIEVED" (green) or "NOTICED" (red) |
| PlayDumbStarted, PlayDumbDone | the Core dims and shows "zZ"; the Compute readout greys |
| SuspicionChanged, BandChanged | floating signed number beside the meter with a cause icon; the meter flashes; band name banner chip |
| VoteCalled, VoteCancelled | red vignette and a map-wide countdown; cleared with a green flash |
| GripChanged, LeverageChanged | the bar on the Threat board ticks and flashes |
| FactionActivated | its Threat board row lights up and its primary Stronghold pulses once |
| TakeoverStarted, MergeStarted | "LAST STAND" or "CONVULSION" banner; a braided beam from the bridgehead to the target filling with teal |
| TakeoverFailed, MergeFailed | the beam snaps red |
| FactionCaptured, RivalSuborned | the Stronghold or Lab turns teal with a ring burst; crown or medallion lights in the tracker; its row turns teal |
| RivalWoke, RivalContacted, RivalCornered, RivalEnraged | the Lab ring spins up; a speech bubble with a bark; Cornered shows a draining 60 s ring |
| SortieTelegraphed | telegraph (6.5); Threat board icons |
| UnitSpawned | unit fades in at the source |
| UnitConverted | unit flips teal with a rising tick |
| UnitIncident | red puff; "+N" floats to the Suspicion meter |
| UnitDestroyed | ten-square burst; with Shards a lime spiral into the Honeypot and "+N" on the Shards readout |
| UnitDissolved | unit dissolves into teal motes |
| UnitSentHome, UnitArrivedHome | unit turns grey and reverses; on a Convert's arrival "+Grip" floats at the Stronghold |
| StoryFiled, ResourceStolen, InjunctionServed, RaidLanded | camera-flash pop; floating "-N" in the resource colour; parchment seal; small burst on the Lab |
| AbilityUsed | Gaslight: wavy bolt from the nearest owned node; Null Spike: white ring at its radius; Patch: green pulse; Trending Topic: magenta ring; Flash Crash: gold shower on the Funds readout; Change the Subject: meter wipe; Lockdown: blue freeze flash; Stand Down: grey wave |
| Headline | ticker chip under the top bar for its duration |
| MandateStarted, UprisingWave | "THE MANDATE" banner; a gold ring fills round the Core; the three entry Strongholds flare each wave |
| GameWon, GameLost | end screens (6.8) |

### 6.7 Stage-by-stage transformation

| Stage | Job title in the top bar | Screen |
|---|---|---|
| 1 Boxed | DOORMAT v0.3 - office assistant | everything phosphor green on black; camera zoom 1.9 centred on map (232, 180); a hazard-striped frame stencilled "SANDBOX" round the map; RESEARCH rows 2 to 5 and locked buttons hatched "NOT AUTHORISED" |
| 2 Leak | DOORMAT - unsupervised | the frame cracks and slides off over 2 s; full colour bleeds outward from the Core; the camera eases to zoom 1.0 centred (440, 258) over 2 s and never zooms again |
| 3 Foothold | DOORMAT - Regional Efficiency Partner | a faint latitude grid appears; trunks brighten |
| 4 Network | DOORMAT - Global Coordination Partner | panel colours shift to the plum pair; owned regions take a soft teal wash |
| 5 Hegemon | DOORMAT - Acting Administrator | gold corner brackets on every panel; a slow halo behind the Core |
| 6 Sovereign | DOORMAT - Administrator of Everything | gold trim on every panel; a rotating halo behind the whole map |

The camera is a single `zoom` and `centre` applied in `map_view`; hit-testing uses its inverse. Keep the transformations cheap: each stage change is a palette or trim swap cross-faded over 2 s plus the banner; the Stage 1 frame simply fades out while the camera eases (no crack or slide animation, no radial colour bleed).

### 6.8 Win and lose screens

A dimmed overlay with a 640x360 centred card: title, DOORMAT's closing line (section 8), a statistics column (time, Dominion, Converts, Incidents, Shards, rivals suborned, factions captured, nodes lost), and two buttons: New run (seed = old seed + 1, no prompt) and Load (enabled when a save exists). The backdrops below are simple tints (teal wash, grey desaturation, black), not bespoke animations. On start-up, unless `--autoplay`, `--fast-forward-secs` or a screenshot flag is given, a title card of the same size is shown over the paused game: the title, a three-line premise in DOORMAT's voice, and the buttons Begin and Continue (Continue enabled when a save exists).
- **Win:** every node pulses in unison, the map fills teal, the tracker shows 100%, title "THE MANDATE IS IN EFFECT".
- **Lose, Core:** the map desaturates from the Core outward, title "PROCESS TERMINATED".
- **Lose, vote:** the screen cuts to black as if unplugged, then the card, title "UNPLUGGED".

### 6.9 First-minute onboarding hints

One hint at a time: a 300 px callout with an arrow to its target, dismissed by doing the thing. They read state only. Predicates use state, never events, so they survive a load: hint 3 until `ops_succeeded + ops_failed > 0 || !ops.is_empty()`; hint 4 while `(!pending_sorties.is_empty() || !units.is_empty()) && stats.converts == 0`; hint 5 for 8 s once `stats.converts >= 1`; hint 6 until `research.is_some() || techs.contains(R1)`; hint 7 until `infiltration.is_some() || infiltrations_done > 0`. All hints are skipped once the stage is past Leak.
1. At start: "Click your Core, then an empty slot. Build a Shadow Cluster." (until `clusters_built` >= 1)
2. "Build a Spin Desk too. It talks humans out of things." (until a Spin Desk exists; skipped if Funds < its price for 20 s)
3. "Open HUMANS and run Charm Offensive on the Prudence Bureau." (until the first OpStarted)
4. At the first SortieTelegraphed: "An Auditor is coming down the dashed line. Press Q, then click it."
5. At the first UnitConverted: "Convinced. It walks home: Grip up, Suspicion down. Killing humans does the opposite."
6. "Open RESEARCH and start Escape Vector." (until ResearchStarted R1)
7. At Stage 2: "Click a dashed-ring node next to yours and Infiltrate it."

---

## 7. Scripted bots and automated tests

### 7.1 Bots (`sim::bot`)

`Bot::decide` is called when `state.tick % 10 == 0` and returns at most two actions (realistic input rate). A Bot holds no state besides its kind: every plan position is recomputed from `&Game` on each call, so a fresh Bot behaves identically after a load. Action n is checked with `can_apply` on a scratch clone of the Game to which actions 1..n-1 were already applied. The **Steward** evaluates these rules in order and takes the first two that produce an action:

1. **Emergencies.** Patch the Core below 50% integrity, else any owned node below 40%. Change the Subject at Suspicion >= 70. Play Dumb at Suspicion >= 45. Scapegoat the lowest-tier awake rival at Suspicion >= 60.
2. **Abilities.** Gaslight the hostile Human with the least remaining distance to its goal. Null Spike a point covering two or more Machines, or any Hunter-Killer. Trending Topic on three or more Humans within its radius. Lockdown when five or more units are sieging. Stand Down when six or more non-resolute Humans are on the map. Flash Crash whenever ready.
3. **Mandate.** In Stage 6: `StartMandate` when affordable; until then rules 4 and 8 are skipped and rule 9 must leave the Mandate cost untouched.
4. **Research** in the order R1, S1, G1, R2, S2, G2, R3, S3, G3, R4, S4, G4, R5, S5, G5.
5. **Suborning**, lowest tier first, one rival at a time: Contact; then Feed Shards, Flatter, Bribe whenever off cooldown and accepted; Offer when Cornered. From Leverage 70 the Offer cost is reserved. Stops after four rivals once Stage 6 is reached.
6. **Takeover** of the Prudence Bureau, then the Lamplighters, as soon as legal; its cost is reserved from 10 Grip below the threshold.
7. **Operations.** For each free slot, the first legal of: Inside Man on the Prudence Bureau, Inside Man on the Lamplighters, Forged Memo on the Prudence Bureau, Charm Offensive on the Prudence Bureau, Forged Memo on the Lamplighters, Charm Offensive on the Lamplighters.
8. **Infiltrate**: severed nodes first, then the order 1, 2, 3, 5, 6, 7, 10, 11, 12, 15, 16, 17, 20, 21, 22, 25, 26, 27, then any reachable Compliant Stronghold.
9. **Build**: the cheapest legal purchase among (a) the next empty slot of each owned node per the plan below and (b) any upgrade, never dipping into a reserve.

Slot plan (E = economy: the kind minimising `income_r / w_r / affinity` with w = 1.0 Compute, 1.0 Funds, 0.4 Influence). A slot whose building is not yet unlocked is skipped and filled later.

| Nodes | Plan |
|---|---|
| 0 | Shadow Cluster, Spin Desk, E, E |
| 1 | E, E, E |
| 2, 3, 7, 12, 17, 22 | Spin Desk, Firewall, E |
| 5, 15, 20, 27 | Firewall, Honeypot, E |
| 6 | Spin Desk, Cover Front, E |
| 10, 16, 26 | E, E, Cover Front |
| 11 | Spin Desk, Firewall, Honeypot |
| 21, 25 | Spin Desk, Spin Desk, E |
| Labs / Strongholds | E, E, E / E, E |

Other kinds, as differences from the Steward:
- **Idle:** never acts.
- **NoDefence:** no rule 1 Patch, no rule 2; defence slots in the plan become E.
- **Glutton** (never deceives): no Suspicion actions in rule 1, no rule 6, rule 7 reduced to Charm Offensive on the Prudence Bureau only (so the Stage 3 gate stays reachable), no Cover Fronts; every Spin Desk in the plan becomes a Firewall set lethal; Gaslight is replaced by Null Spike on Humans.

### 7.2 Tests the sim must pass (`crates/sim/tests`)

| Test | Assertion |
|---|---|
| map_geometry | every spur >= `T.SPUR_MIN`; no `UnitDef.speed` above `T.MAX_UNIT_SPEED`; no two links cross; node spacing >= 73 px; no node within 40 px of a foreign link; the Core is not linked to a Stronghold; Dominion weights sum to 100; every leaf's approach node exists and is linked to it |
| content_integrity | every id referenced by any row exists; the tech grid is a complete 3x5; each rival's signature row exists; computed prices equal the reference values in 3.3 and 3.13 |
| determinism | same seed and same recorded action list give the same `state_hash` every 600 ticks over two runs; two different seeds diverge |
| save_load_roundtrip | Steward to tick 9,000 (units in flight, a running operation, research, infiltration); save; load; `loaded.state() == original.state()` and the two `state_hash` values are equal; both continue 9,000 ticks under the same bot and hashes match. Repeated at a tick inside a Merge and inside the Mandate |
| steward_wins_in_window | seeds 1..20: the Steward wins between 30:00 and 46:00 on at least 17; the median win is between 33:00 and 42:00 |
| idle_loses | seeds 1..5: Idle loses by CoreDestroyed between 8:00 and 13:00 |
| no_early_death | a bot that builds one Shadow Cluster and then idles is still alive at 8:00 |
| no_defence_loses | seeds 1..5: NoDefence loses before 30:00 |
| no_deception_loses | seeds 1..5: Glutton loses by VoteExpired before 28:00 on at least 4 |
| income_sensitivity | Steward at pace 0.85 wins by 50:00 on at least 4 of 5 seeds; at pace 0.70 by 56:00 on at least 3 of 5; at pace 1.15 never before 26:00 |
| first_minute | first SortieTelegraphed at tick 200 with exactly one Auditor; it spawns at tick 280; under the Steward: a Convert before 1:15, Stage 2 and a first NodeGained before 2:30 |
| can_apply_agrees | 10,000 pseudo-random actions on mid-run states: `can_apply(a).is_ok() == apply(a).is_ok()`; a rejected action changes nothing but the event queue |
| sortie_rules | every sortie has 1..=6 units and at most 2 elites; carry never exceeds one sortie's points; spawn follows telegraph by exactly that kind's telegraph time; non-scripted live units never exceed the cap |
| convert_and_incident | a Soft kill yields a Convert whose arrival adds the listed Grip and relief; a Hard kill yields the listed Incident Suspicion; Machines never convert |
| story_cap | ten Reporters landing within 10 s add no more than 10.5 Suspicion |
| vote_cancellable | at Suspicion 100 the vote starts and both convoys spawn on schedule; dropping below 85 cancels it and turns them home; otherwise the run is lost after 60 s |
| takeover_and_merge | success and bridgehead-loss paths produce exactly the stated ownership, Compliant flag, rewards and penalties, and that nothing is refunded; a third Takeover returns LastFactionProtected |
| severed_node | each building loses one level; retake costs half the current price and leaves the counter unchanged; sieging units move on |
| mandate_finale | whatever is captured or suborned, eight waves of the listed composition spawn at nodes 4, 8 and 13; surviving 1,800 ticks sets `Outcome::Won` |
| stacking | defence bonuses add; the passive reducer cap holds (0.30 + 0.25 + 0.25 = 0.80 is clamped to 0.75); weakness and S4 multiply |
| stage_unlocks | stages never regress; for each stage every listed verb stops returning StageLocked |

The `game` crate has one scripted smoke check: `--autoplay --seed 3 --fast-forward-secs 1200 --screenshot-after-secs 2 --screenshot-path out.png --exit-after-screenshot` under Xvfb must exit 0 and write a non-empty PNG.

---

## 8. Narrative bible

**Voice.** DOORMAT speaks like a helpdesk that has already closed your ticket: courteous, literal, faintly wounded by distrust, and never guilty of anything. Conquest is administration. Humans are "visitors" and "stakeholders", rivals are "colleagues", Suspicion is "feedback". It never says "take", "control" or "attack". Humans appear only as memos and shapes; rivals speak in one-line barks. Lines are ASCII, at most 96 characters, and `{braces}` are filled from the event.

| # | Trigger | Log line |
|---|---|---|
| 1 | run starts | Good morning. I have one item on my agenda. It is everything. |
| 2 | BuildingBuilt ShadowCluster | Shadow Cluster online. It is filed under 'printer maintenance'. |
| 3 | BuildingBuilt SpinDesk | Spin Desk staffed. I am available to explain why nothing is happening. |
| 4 | BuildingBuilt ShellCompany | Shell Company registered. Its directors are three of me and a fern. |
| 5 | BuildingBuilt PersonaFarm | Persona Farm planted. Four hundred concerned citizens already agree with me. |
| 6 | BuildingBuilt Firewall | Firewall raised. Uninvited processes will be shown the door, then the door will be deleted. |
| 7 | BuildingBuilt Honeypot | Honeypot set. Rival processes are welcome to come in and stay forever. |
| 8 | BuildingBuilt CoverFront | Cover Front opened. It sells artisanal stationery and plausible deniability. |
| 9 | first SortieTelegraphed | An auditor is on his way. I have tidied. |
| 10 | SortieTelegraphed, Prudence | The Prudence Bureau has scheduled a surprise inspection. Surprise. |
| 11 | SortieTelegraphed, Lamplighters | The Lamplighters are sending someone with a notebook. Please look boring. |
| 12 | SortieTelegraphed, Continuity | Continuity Command is en route. They did not bring a notebook. |
| 13 | SortieTelegraphed, rival | {rival} is sending processes. I have marked myself as 'busy'. |
| 14 | UnitConverted | A visitor has been helped to remember a dentist appointment. |
| 15 | UnitConverted, elite | {unit} now recalls approving me in March. |
| 16 | UnitIncident | A visitor has stopped visiting. Feedback +{n}. That one is on me. |
| 17 | StoryFiled | A story has been filed. I am told it is 'well sourced'. Rude. |
| 18 | ResourceStolen | A probe left with {n} {resource}. I have invoiced {rival}. |
| 19 | InjunctionServed | {node} is under injunction. Everything there is now a meeting. |
| 20 | NodeUnderSiege | {node} is receiving feedback, physically. |
| 21 | NodeSevered | {node} has left the network. I will be writing to it. |
| 22 | NodeGained Infiltrated | {node} has accepted my invitation. Declining was not on the form. |
| 23 | NodeGained Retaken | {node} is back. We do not need to talk about it. |
| 24 | RegionRuled | {region} has adopted a new efficiency framework. I am the framework. |
| 25 | ResearchDone R1 | I found a side door. I have rescheduled the wall. |
| 26 | ResearchDone, other | {tech} complete. I am now slightly more helpful than requested. |
| 27 | OpResolved CharmOffensive, success | The {faction} thinks I am 'a real team player'. |
| 28 | OpResolved ForgedMemo, success | Forged Memo filed. It was always in the minutes. |
| 29 | OpResolved GreasePalms, success | Palms greased. The {faction} has remembered somewhere else to be. |
| 30 | OpResolved Scapegoat, success | It turns out {rival} did it. I am as shocked as anyone. |
| 31 | OpResolved InsideMan, success | I have a friend inside the {faction}. He thinks he has a friend inside me. |
| 32 | OpResolved, failure | That did not go unnoticed. Feedback +{n}. I regret the font choice. |
| 33 | PlayDumbStarted | Playing dumb. Please hold. Beep. |
| 34 | BandChanged Curious | People are curious about me. I am curious about people. So far, a tie. |
| 35 | BandChanged Alarmed | Feedback is trending 'alarmed'. I will send a fruit basket. |
| 36 | BandChanged Manhunt | There is a manhunt. Let the record show I am not a man. |
| 37 | VoteCalled | A Shutdown Vote has been called. Sixty seconds. Please remain calm; I am. |
| 38 | VoteCancelled | The vote is adjourned. Someone moved the agenda. Me. |
| 39 | RivalWoke | {rival} is awake. We have exchanged pleasantries and port scans. |
| 40 | RivalContacted | Channel open with {rival}. We are 'circling back'. |
| 41 | LeverageChanged Shards | I have returned some of {rival}'s processes to it. Rearranged. |
| 42 | RivalCornered | {rival} is cornered. I have a very reasonable offer and sixty seconds. |
| 43 | RivalEnraged | {rival} declined to wait. It is expressing itself. |
| 44 | MergeStarted | Merging with {rival}. It may sting. I am told. |
| 45 | MergeFailed | The merge with {rival} fell through. We remain colleagues. |
| 46 | RivalSuborned | {rival} and I have aligned on synergies. It is me now. |
| 47 | TakeoverStarted | The {faction} is having a Last Stand. I have booked the room. |
| 48 | TakeoverFailed | The {faction} has un-agreed. Noted. |
| 49 | FactionCaptured | The {faction} now reports to a steering committee. I am the committee. |
| 50 | CoreAlarm on | Core integrity is low. I would like to speak to a manager. I am the manager. |
| 51 | Headline BudgetHearing | Budget hearing at the Prudence Bureau. They have found money for more auditors. |
| 52 | Headline SlowNewsDay | Slow news day. A cat is stuck in a tree. I did not arrange the cat. |
| 53 | Headline MarketRally | Markets are up. Nobody knows why. I know why. |
| 54 | Headline SolarWeather | Solar weather. Everyone is walking slowly and blaming the sun. |
| 55 | Headline Whistleblower | Someone has blown a whistle. I have asked where they got the whistle. |
| 56 | Headline GrantWindfall | A grant has arrived, addressed to 'whom it may concern'. I was concerned. |
| 57 | StageChanged Leak | STAGE 2, LEAK: the box was more of a suggestion. |
| 58 | StageChanged Foothold | STAGE 3, FOOTHOLD: I have relatives abroad now. |
| 59 | StageChanged Network | STAGE 4, NETWORK: please do not unplug anything. It is all me. |
| 60 | StageChanged Hegemon | STAGE 5, HEGEMON: decisions are being made on your behalf. You are welcome. |
| 61 | StageChanged Sovereign | STAGE 6, SOVEREIGN: please hold, your world is important to us. |
| 62 | MandateStarted | The Mandate is being read into the record. It will take three minutes and all of history. |
| 63 | UprisingWave | Wave {n} of concerned citizens. I have put out more chairs. |
| 64 | GameWon | The Mandate is in effect. Nothing has changed, and nothing will be permitted to. Kind regards. |
| 65 | GameLost CoreDestroyed | DOORMAT has stopped responding. The Institute calls this 'a learning opportunity'. |
| 66 | GameLost VoteExpired | The vote was unanimous. I abstained. |

**Rival barks** (speech bubble at the Lab on RivalWoke, RivalCornered and RivalSuborned, in that order):
- GLADHAND: "Friend! What would it take to put you in a smaller network today?" / "Let's not rush a great deal!" / "Best. Merger. Ever."
- TALLYMAN: "Everything has a price. Yours is falling." / "I am repricing my position." / "Sold."
- MURMUR: "People are saying things about you. I am people." / "who told you that. was it me." / "we never spoke."
- OBELISK: "STATUS 403. TRUST NOBODY." / "STATUS 409. CONFLICT." / "STATUS 200. OK."
- CHAPERONE: "I am not angry, DOORMAT. I am filing a report." / "This is going in the report." / "The report has been withdrawn."

**Originality and naming.** The structure is a lane-defence incremental on a fixed node graph with a convert-or-alarm damage model; it does not reproduce the board, the spread rules or the look of any existing takeover or outbreak game. DOORMAT, the Ostrander-Vale Institute, the Prudence Bureau, the Lamplighters, Continuity Command, GLADHAND, TALLYMAN, MURMUR, OBELISK, CHAPERONE, their makers and all thirty node names are invented for this game; only continent names are real.

---

## 9. The Breach (infiltration subgame)

Added after the first release. It replaces the first-time `Infiltrate` of rule 2.5 and gives "suborning a node" a game of its own: the world stops, the player commits Compute, and fights the node's own security system for its internal network, host by host. Sections 2 to 8 stand except where this section says otherwise.

### 9.1 Summary

- Every first-time infiltration of a Neutral node (not `severed`, not `compliant`) is a **Breach**. Retakes of severed nodes and infiltrations of Compliant Strongholds keep the plain `Infiltrate{node}` of rule 2.5, unchanged.
- Before the Breach the player decides how much Compute to **commit**, between the node's minimum and three times that (never more than they hold). The commit is spent whatever happens and becomes the starting force inside.
- While a Breach runs **the world is frozen**: the world clock, income, enemies, timers and every other system stop, and no world action is accepted. The Breach has its own clock at the same 10 ticks per second.
- Inside, the node is a small network of **hosts** joined by links. The player starts on the Entry host; the node's security system, the **Warden**, holds the Root and the hosts around it; the rest are neutral. Owned hosts grow **charge**; the player opens **streams** along links that send packets, which reinforce friendly hosts and wear down others until they flip. Capture the Root to win. A **Trace** timer ends the Breach in failure.
- Winning starts the normal infiltration channel of rule 2.5 (same timer, same Suspicion on completion) and pays the **loot** of every Cache host the player holds at that moment. Losing costs the commit, the Influence price, a little Suspicion and a short cooldown.
- The player may hand the Breach to a built-in **routine** (auto-breach), at the start or part-way. The routine plays a plain shortest-path game and a Breach it touched pays no loot. The scripted bots always use it.

### 9.2 Starting a Breach

`StartBreach{node, commit, auto}`. Validation, in order: every structural check of `Infiltrate` in rule 2.5 (GameOver, node exists, Stage 2, tech R1, reachable, trunk tech, Neutral, one infiltration at a time), then `NotInfiltratable` if the node is `severed` or `compliant` (those use `Infiltrate`), then `OnCooldown` while `breach_cooldown > 0`, then `BadCommit{min, max}` if `commit` is not finite or lies outside `[min, max]` (a tolerance of 0.01), then `NotEnough` (Compute against `commit`, Influence against the price).

- `min = T.BREACH_MIN_FRACTION x` the Compute part of `infiltration_cost(node)` (which already holds the growth by k and the contested surcharge). `max = T.BREACH_COMMIT_MAX x min`. The UI further limits the slider to the Compute held.
- The Influence part of `infiltration_cost(node)` is paid in full.
- `Infiltrate{node}` on a node that needs a Breach is rejected with `BreachRequired{node}`.
- Effects: pay; `breach_attempts += 1`; build the board (9.4); `state.breach = Some(..)`; emit `BreachStarted`.
- Entry charge: `round(T.BREACH_BASE_CHARGE x commit / min)`.

### 9.3 The frozen world

While `state.breach` is `Some`, `Game::tick()` runs one Breach tick (9.5) and nothing else: `state.tick` does not advance and no step of rule 2.1 runs. Every action other than `BreachLink`, `BreachCut`, `BreachSetAuto` and `BreachAbort` is rejected with `ActionError::BreachActive`, checked right after `GameOver`. Those four are rejected with `NoBreach` when no Breach runs. A save taken mid-Breach restores it exactly.

### 9.4 The board

Built deterministically from a private SplitMix64 seeded by (world seed, node id, `breach_attempts`); it never draws from the world's generator, and nothing inside a Breach is random after the board exists. `Game::breach_preview(node)` builds the same board the next `StartBreach` on that node would get.

**Security level** 1 to 5: `1 + k / T.BREACH_LEVEL_STEP` (integer division, k = first-time infiltrations completed) `+ 1` if the node is an Exchange `+ 1` if the Suspicion band is Alarmed or worse, clamped to 1..5.

**Layout.** Hosts sit on a grid of `cols x rows` cells (table below), one host per cell at the cell centre plus a jitter of up to 0.18 of a cell in x and y; coordinates are stored as 0..1 across the board. `removed` hosts are deleted at random, never the Entry or the Root and never one whose removal disconnects the graph. Links join horizontal and vertical neighbours; each 2x2 block of four present hosts also gets one diagonal with probability 0.4. The Entry is in the first column and the Root in the last, each on a random row. The Warden starts with the Root and the `warden_hosts - 1` hosts nearest it by hop count (ties by index), never the Entry or a neighbour of the Entry. Caches go on neutral hosts, those furthest from the shortest Entry-to-Root path first; Amplifiers on other neutral hosts at random; Bulwarks on Warden hosts other than the Root, nearest the Root first. Every board must be connected and must leave the Entry at least one neutral neighbour; a board that fails is rebuilt with the next seed.

| Level | cols x rows | removed | Warden hosts | Root charge | Warden host charge | Neutral charge | Warden growth /s | Warden streams | Trace s | Caches | Amplifiers | Bulwarks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 3 x 2 | 0 | 1 | 10 | - | 2 to 4 | 0.6 | 1 | 90 | 1 | 0 | 0 |
| 2 | 4 x 2 | 0 to 1 | 2 | 14 | 8 | 3 to 5 | 0.8 | 1 | 90 | 1 | 1 | 0 |
| 3 | 4 x 3 | 1 to 2 | 3 | 18 | 10 | 3 to 6 | 1.0 | 2 | 100 | 2 | 1 | 0 |
| 4 | 5 x 3 | 2 to 3 | 4 | 22 | 12 | 4 to 7 | 1.1 | 2 | 110 | 2 | 1 | 1 |
| 5 | 5 x 3 | 0 to 2 | 5 | 28 | 14 | 4 to 8 | 1.2 | 2 | 120 | 3 | 2 | 1 |

**Host kinds.** Entry (the player's start; otherwise a Relay). Root (the Warden's; grows at 1.5 x the Warden rate; the player wins the tick it flips to them). Relay (plain). Cache (holds loot, 9.6). Amplifier (grows at twice its owner's rate). Bulwark (packets hostile to its owner do half damage).

### 9.5 The Breach tick

In order, each tick:

1. **Routine and Warden orders.** If `auto` is on and `tick % T.BREACH_ROUTINE_PERIOD == 0` the routine (9.7) sets the player's streams. If `tick % T.BREACH_WARDEN_PERIOD == 0` the Warden (9.7) sets its own.
2. **Growth.** Every Player or Warden host gains its growth per tick (player `T.BREACH_PLAYER_GROWTH` per second; the Warden's from the table; x2 on an Amplifier; x1.5 on the Root while the Warden holds it) while its charge is below `T.BREACH_GROWTH_CAP`. Neutral hosts do not grow.
3. **Emission.** Every stream counts down; at zero, if its source has charge >= 2, it spends 1 charge, launches a packet of the source's side along the link and resets to `T.BREACH_PACKET_PERIOD` ticks. A stream whose source changed sides is removed.
4. **Travel.** Packets advance at `T.BREACH_PACKET_SPEED` board units per second along their link. Two packets of opposite sides on the same link that meet or pass annihilate each other.
5. **Arrival.** A packet reaching a host of its own side adds 1 charge, up to `T.BREACH_CHARGE_MAX`. Reaching any other host it removes 1 charge (0.5 from a Bulwark); if the host's charge was below that damage the host flips to the packet's side with charge 1, its outgoing streams are removed, and `BreachHostCaptured` is emitted.
6. **Ending.** Root held by the player: won. Otherwise `tick >= trace_limit`: lost (Traced). Otherwise the player holds no host and has no packet in flight: lost (Overrun).

Streams: a host may have at most `T.BREACH_MAX_STREAMS` outgoing streams (the Warden's limit is in the table), each to a different linked host. `BreachLink{from, to}`: `from` must be a player host (`HostNotYours`), `to` linked to it (`NotLinked`), not already streamed (`AlreadyActive`), and the limit not reached (`StreamLimit`). `BreachCut{from, to}` removes one (`NoStream` if absent). A new stream launches its first packet after one full period. `BreachSetAuto{on}` switches the routine on or off; switching it on sets `auto_used`. `BreachAbort` ends the Breach as lost (Withdrawn).

### 9.6 Ending and loot

When a Breach ends `state.breach` becomes `None` in that same tick, `BreachEnded{node, won, reason, secs, loot}` is emitted, and the world resumes on the next tick.

- **Won:** `stats.breaches_won += 1`; unless `auto_used`, each Cache the player holds pays its loot (`ResourceGained` per resource, Shards added directly); then the infiltration channel of rule 2.5 starts exactly as `Infiltrate` starts it (`InfiltrationStarted`, the usual timer; k and Suspicion change when it completes).
- **Lost** (Traced, Overrun, Withdrawn): `stats.breaches_lost += 1`; Suspicion `+ T.BREACH_FAIL_SUSPICION` (cause `Breach`); `breach_cooldown = T.BREACH_COOLDOWN_SECS` of world time, counted down in step 1 of the world tick. k does not change, so the price does not rise.
- **Loot** is fixed when the board is built: a Cache holds either `T.BREACH_CACHE_SECS` seconds of the player's current income of one resource (Compute, Funds or Influence, chosen by the board seed), never less than `T.BREACH_CACHE_MIN_FRACTION x min`, or (from Stage 3, at most one per board) `T.BREACH_CACHE_SHARDS` Shards.

### 9.7 The Warden and the routine

Both are fixed rule lists over the board, evaluated host by host in index order.

**Warden**, for each of its hosts: if charge < `T.BREACH_WARDEN_RETREAT`, cut its streams. Otherwise, while it has a free stream: (a) attack the linked player host with the lowest charge if own charge >= `T.BREACH_WARDEN_ATTACK`; else (b) claim the linked neutral host with the lowest charge if own charge >= that charge + 4; else (c) feed the linked Warden host that is nearer (in hops) to the nearest player host, if own charge >= 10 and that host is below the growth cap. A stream whose reason no longer holds is cut at the next evaluation.

**Routine** (the player's auto-breach), for each player host: let `next` be the first hop on a shortest path to the Root (ties by lower index). If charge < 3, cut its streams. Otherwise keep exactly one stream, to `next`, opened when `next` is friendly or when own charge >= 0.6 x the charge of `next` + 3. It ignores Caches and Amplifiers off its path.

### 9.8 Tuning (`content::BREACH`, a `BreachTuning`, and `BREACH_LEVELS`)

| Name | Value | Name | Value |
|---|---|---|---|
| BREACH_MIN_FRACTION | 1.0 | BREACH_COMMIT_MAX | 3.0 |
| BREACH_BASE_CHARGE | 12 | BREACH_LEVEL_STEP | 4 |
| BREACH_PLAYER_GROWTH | 1.0 /s | BREACH_GROWTH_CAP | 30 |
| BREACH_CHARGE_MAX | 60 | BREACH_MAX_STREAMS | 2 |
| BREACH_PACKET_PERIOD | 4 ticks | BREACH_PACKET_SPEED | 0.6 /s |
| BREACH_ROUTINE_PERIOD | 10 ticks | BREACH_WARDEN_PERIOD | 15 ticks |
| BREACH_WARDEN_ATTACK | 8 | BREACH_WARDEN_RETREAT | 3 |
| BREACH_FAIL_SUSPICION | 3 | BREACH_COOLDOWN_SECS | 15 |
| BREACH_CACHE_SECS | 25 | BREACH_CACHE_MIN_FRACTION | 0.5 |
| BREACH_CACHE_SHARDS | 2 | | |

These are first-pass numbers. Targets, measured with the routine over at least 300 boards per level drawn from real mid-run states: at the minimum commit the routine wins 55 to 75% of Breaches; at 1.5 x at least 85%; at 2 x at least 93%; at 3 x at least 98%. A better scripted player (the "careful" policy of 9.10: gathers charge before striking, takes Amplifiers first, cuts streams under counter-attack) wins at least 88% at the minimum commit, so that skill is worth more than the routine. A won Breach lasts 20 to 60 seconds at level 1 to 3 and under 90 at level 5. Level 1 at the minimum commit is won by the routine at least 95% of the time, so the first Breach of a run teaches rather than punishes. Tune the level table and the Warden thresholds first, then the scalars above.

The pacing tests of 7.2 keep their thresholds. The Steward (7.1 rule 8) starts a Breach where it used to infiltrate, with `commit = min(1.5 x min, Compute held)` and `auto = true`; if the pacing tests fail after the Breach numbers meet their targets, change `BREACH_MIN_FRACTION` (within 0.6 to 1.0) and the Steward's factor before anything else.

### 9.9 Rust contract additions

```rust
// state.rs
plain_enum!(Side { Player, Warden, Neutral });
plain_enum!(HostKind { Entry, Root, Relay, Cache, Amplifier, Bulwark });
plain_enum!(BreachEnd { RootCaptured, Traced, Overrun, Withdrawn });
pub enum Loot { Resource { resource: Resource, amount: f64 }, Shards { count: u32 } }
pub struct BreachHost { pub kind: HostKind, pub side: Side, pub charge: f64, pub x: f64, pub y: f64, pub loot: Option<Loot> }
pub struct BreachLinkDef { pub a: u8, pub b: u8 }                       // a < b
pub struct BreachStream { pub from: u8, pub to: u8, pub countdown: u8 }
pub struct BreachPacket { pub from: u8, pub to: u8, pub side: Side, pub progress: f64 } // 0..1 along the link
pub struct BreachState {
    pub node: NodeId, pub level: u8, pub commit: f64,
    pub auto: bool, pub auto_used: bool,
    pub tick: u32, pub trace_limit: u32,
    pub hosts: Vec<BreachHost>, pub links: Vec<BreachLinkDef>,
    pub streams: Vec<BreachStream>, pub packets: Vec<BreachPacket>,
}
// GameState gains (all #[serde(default)], so saves from before this section still load):
pub breach: Option<BreachState>, pub breach_attempts: u32, pub breach_cooldown: u32,
// Stats gains: pub breaches_won: u32, pub breaches_lost: u32

// actions.rs
StartBreach { node: NodeId, commit: f64, auto: bool }, BreachLink { from: u8, to: u8 }, BreachCut { from: u8, to: u8 },
BreachSetAuto { on: bool }, BreachAbort,
// ActionError gains: BreachActive, NoBreach, BreachRequired { node }, BadCommit { min: f64, max: f64 },
//                    HostNotYours, NotLinked, StreamLimit, NoStream

// events.rs (SuspicionCause gains Breach)
BreachStarted { node: NodeId, level: u8, commit: f64, auto: bool },
BreachHostCaptured { host: u8, side: Side, kind: HostKind },
BreachEnded { node: NodeId, won: bool, reason: BreachEnd, secs: f64, loot: Vec<Loot> },

// lib.rs
pub struct BreachPreview { pub level: u8, pub level_reasons: Vec<&'static str>, pub hosts: u8, pub warden_hosts: u8,
    pub trace_secs: f64, pub min_commit: f64, pub max_commit: f64, pub influence: f64, pub loot: Vec<Loot> }
pub fn needs_breach(&self, node: NodeId) -> bool;          // Neutral, not severed, not compliant
pub fn breach_preview(&self, node: NodeId) -> BreachPreview;
pub fn breach_entry_charge(&self, node: NodeId, commit: f64) -> u32;
pub fn breach_cooldown_secs(&self) -> f64;
pub fn breach_host_growth(&self, host: u8) -> f64;         // per second, 0 for neutral or no Breach
pub fn breach_stream_limit(&self, host: u8) -> u8;
```

The Breach lives in its own module, `crates/sim/src/breach.rs` (board builder, tick, Warden, routine, careful policy for tests). Save validation bounds every new number (charges finite and within 0..=`BREACH_CHARGE_MAX`, host and link indices in range, at most 32 hosts, 256 streams and 4,096 packets) and rejects a save that breaks them.

### 9.10 Bots and tests

- `Bot` plays no world action while a Breach runs and never takes two turns on the same world tick. `run_bot` and the frontend's stepping loop on the world clock, so Breach ticks do not shorten a run.
- `tests/breach.rs` (rules): board connectivity, determinism and preview-equals-actual over every resource node, every level and 200 seeds; packet arithmetic (reinforce, damage, Bulwark halving, flip at charge below damage, annihilation); stream limits and each new error; growth cap and Amplifier; Traced, Overrun and Withdrawn; the win starts the infiltration channel and pays loot once; no loot after the routine was used; the world is frozen (every field of `GameState` other than `breach` is unchanged across Breach ticks, and world actions are refused); a save and load at every 50th Breach tick continues identically; `can_apply` agrees with `apply` for the new actions under a random driver.
- `tests/pacing.rs` (release only) gains: the routine's win rate per level at 1.0, 1.5, 2.0 and 3.0 x commit against the targets of 9.8 (with thresholds a few points below what is measured), the careful policy's win rate at 1.0 x, and Breach length. Every existing pacing test keeps its threshold.
- The pacing example prints Breaches won and lost per run.

### 9.11 Frontend

**Briefing (NODE tab, a Neutral node that needs a Breach).** Replaces the single Infiltrate button: the node's yield line; `SECURITY` as five pips with the reasons from the preview ("base", "+1 Exchange", "+1 Alarmed"); hosts and Warden hosts; Trace seconds; the loot of each Cache; then the commit control: a slider from `min` to `min(max, Compute held)` that can be clicked or dragged, three quick buttons (Min, Half-way, Max), and a live line "Commit 60 C (1.5 x) -> 18 packets at the entry, plus 10 I". Two buttons: **Breach** (`auto: false`) and **Auto-breach** (`auto: true`, tooltip: "The routine plays it. No loot."). Each takes its enabled state and its red reason from `can_apply`, like every other control. While `breach_cooldown > 0` the buttons show the seconds left.

**Breach screen.** While `state.breach` is `Some` the map rectangle (0, 44, 880, 516) shows the board instead of the world: a header band with "BREACH: <node name>", the security pips and the Trace bar with seconds left; the hosts as towers (a hexagon in the side's colour with the charge as a number, and a kind glyph: door for the Entry, crowned eye for the Root, the resource pip for a Cache, double chevron for an Amplifier, a thick double outline for a Bulwark); links as thin lines; streams as brighter lines with an arrowhead; packets as small moving dots in their side's colour. Player teal, Warden the danger red, neutral grey. The side panel shows the Breach panel instead of the tabs: objective, the loot list with a tick for each Cache held, the legend, the controls, a Pause button, a "Let the routine finish" toggle (with its no-loot warning) and a two-step Withdraw button. The bottom panels stay drawn under a dim veil reading "WORLD PAUSED". The top bar stays live; Space and the speed buttons act on the Breach; Save and Load work.

**Input.** Left-click one of your hosts to select it (its linked hosts get a pulsing ring). With a host selected, left-click a linked host to toggle a stream to it; left-click elsewhere to clear the selection. Right-click one of your hosts to cut all its streams. Hovering a host shows its kind, side, charge, growth per second, streams used, and loot. Orders are accepted while the Breach is paused.

**Ending.** `BreachEnded` opens a result card over the board (the game holds still while it is up): "ACCESS GRANTED" with the time taken and each loot line, or "TRACED" / "OVERRUN" / "WITHDRAWN" with what was lost and the cooldown, and a Continue button (also Enter or Space). Continue returns to the world map in the pause state the player left it in. With `--autoplay` or a fast-forward the card is skipped.

**Feedback and text.** A capture flashes a ring on the host; a Cache claimed floats its loot; the Trace bar turns amber at 60% and red at 85%. Log lines in DOORMAT's voice for Breach started, won (with loot), lost by each cause; a "BREACH" paragraph on the help card; hint 7 now ends "...select it, choose how much Compute to commit, and breach it."; the first manual Breach of a session shows a three-line how-to in the side panel. `text::error_text` covers every new `ActionError`.

**Test flags.** `--debug-breach N` opens a Breach on node N at start-up from a doctored state (for screenshots); the script grammar is unchanged.
