"""The station clock.

Time is stored as fractional game minutes since 00:00 on day 1.  Everything
that happens "per hour" in the simulation is driven from here, so advancing
the clock by eight hours while the player sleeps runs exactly the same code
as eight hours of real play.
"""
from __future__ import annotations

from .. import config


class GameClock:
    def __init__(self, minutes: float = 0.0, time_scale: float = config.TIME_SCALE):
        self.minutes = float(minutes)
        self.time_scale = float(time_scale)   # game minutes per real second

    # -- reading --------------------------------------------------------------
    @property
    def day(self) -> int:
        return int(self.minutes // config.MINUTES_PER_DAY) + 1

    @property
    def minute_of_day(self) -> float:
        return self.minutes % config.MINUTES_PER_DAY

    @property
    def hour(self) -> int:
        return int(self.minute_of_day // 60)

    @property
    def minute(self) -> int:
        return int(self.minute_of_day % 60)

    @property
    def hour_float(self) -> float:
        return self.minute_of_day / 60.0

    def time_str(self) -> str:
        return "%02d:%02d" % (self.hour, self.minute)

    def stamp(self) -> str:
        return "Day %d %s" % (self.day, self.time_str())

    def is_between(self, start_hour: float, end_hour: float) -> bool:
        """True if the time of day lies in [start, end); handles ranges that
        wrap past midnight such as 22 -> 6."""
        h = self.hour_float
        if start_hour == end_hour:
            return True
        if start_hour < end_hour:
            return start_hour <= h < end_hour
        return h >= start_hour or h < end_hour

    def is_night(self) -> bool:
        return self.is_between(22, 6)

    # -- changing -------------------------------------------------------------
    def advance(self, game_minutes: float) -> None:
        self.minutes += game_minutes

    @staticmethod
    def to_minutes(day: int = 1, hour: float = 0, minute: float = 0) -> float:
        return (day - 1) * config.MINUTES_PER_DAY + hour * 60 + minute

    @staticmethod
    def format_stamp(minutes: float) -> str:
        c = GameClock(minutes)
        return c.stamp()

    def to_dict(self) -> dict:
        return {"minutes": self.minutes, "time_scale": self.time_scale}

    @classmethod
    def from_dict(cls, data: dict) -> "GameClock":
        return cls(data.get("minutes", 0.0), data.get("time_scale", config.TIME_SCALE))
