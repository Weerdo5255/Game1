"""Run the simulation headless and print what happens.

    python -m game.tools.simulate --days 5            # station report every 6 hours
    python -m game.tools.simulate --days 2 --log      # include the crew activity log
    python -m game.tools.simulate --days 3 --crew     # where everyone is at each report
    python -m game.tools.simulate --seed 7 --every 1  # hourly reports, fixed random seed

Nothing here opens a window: it is the same ``World`` the game uses, simply
fast-forwarded.  Use it to balance systems, to check that NPC goals actually
get done, or to see how the station fares when the player does nothing.
"""
from __future__ import annotations

import argparse
import sys
import time


def station_line(world) -> str:
    s = world.systems
    parts = ["%-11s" % world.clock.stamp(),
             "pwr %3.0f/%3.0f" % (s.power_supply, s.power_demand)]
    for name in s.resources:
        parts.append("%s %4.0f" % (name[:5], s.resource(name)))
    for sid, system in s.systems.items():
        if system.decay <= 0:
            continue
        parts.append("%s %2.0f%%/%2.0f" % (sid[:5], system.condition, system.efficiency * 100))
    return "  ".join(parts)


def crew_lines(world):
    out = []
    for ch in world.npcs():
        gmap = world.maps.get(ch.map_id)
        place = gmap.place_name(ch.x, ch.y) if gmap else "?"
        out.append("    %-22s %-24s hun %3.0f fat %3.0f mor %3.0f hp %3.0f  %s" % (
            ch.name[:22], place[:24], ch.needs["hunger"], ch.needs["fatigue"], ch.morale, ch.hp,
            ch.activity))
    return out


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Headless station simulation")
    parser.add_argument("--days", type=float, default=3.0)
    parser.add_argument("--every", type=float, default=6.0, help="hours between reports")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--log", action="store_true", help="print the station log as it happens")
    parser.add_argument("--crew", action="store_true", help="print crew status at each report")
    parser.add_argument("--data", default=None)
    args = parser.parse_args(argv)

    from game.content import Content
    from game.world.world import World

    content = Content.load(args.data)
    for problem in content.problems:
        print("content problem:", problem)
    world = World.new_game(content, seed=args.seed)
    world.requests.clear()
    total_hours = args.days * 24.0
    t0 = time.time()
    hours = 0.0
    log_mark = 0
    print(station_line(world))
    while hours < total_hours and not world.ended:
        step = min(args.every, total_hours - hours)
        world.fast_forward(step * 60.0)
        world.requests.clear()
        world.toasts.clear()
        hours += step
        if args.log:
            for entry in world.log[log_mark:]:
                print("      %s  %s" % (world.clock.format_stamp(entry["t"]), entry["text"]))
        log_mark = len(world.log)
        print(station_line(world))
        if args.crew:
            for line in crew_lines(world):
                print(line)
    elapsed = time.time() - t0
    print()
    print("Simulated %.1f game hours in %.1f s (%.0fx real time at the default clock)." % (
        hours, elapsed, hours * 60.0 / max(elapsed, 1e-6) / max(0.001, world.clock.time_scale)))
    if world.ended:
        print("The game ended: %s" % world.ended)
    food = sum(world.inventory_of(o).count_tag(content.items, "food") for o in world.containers())
    print("Food left in containers: %d   spare parts: %d   fuel cells: %d" % (
        food, world.stock("spare_parts"), world.stock("fuel_cell")))
    done = [(c.name, g["desc"]) for c in world.characters.values() for g in c.goals if g["status"] == "done"]
    if done:
        print("Goals completed: " + "; ".join("%s - %s" % d for d in done))
    if world.script.errors:
        print("\nScript errors (%d):" % len(world.script.errors))
        for err in world.script.errors[-10:]:
            print("   ", err)
    return 0


if __name__ == "__main__":
    sys.exit(main())
