"""Immutable deltas with bounded-cost random access; independent of Qt."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Event:
    changes: tuple[tuple[str, object], ...] = ()
    removed: tuple[str, ...] = ()


class Replay:
    def __init__(self, events, interval=64):
        self.events = tuple(events)
        self.interval = max(1, interval)
        state = {}
        self.checkpoints = {0: ()}
        for i, event in enumerate(self.events, 1):
            self._apply(state, event)
            if i % self.interval == 0:
                self.checkpoints[i] = tuple(state.items())

    @staticmethod
    def _apply(state, event):
        for key in event.removed:
            state.pop(key, None)
        state.update(event.changes)

    def at(self, count):
        count = max(0, min(count, len(self.events)))
        base = count // self.interval * self.interval
        state = dict(self.checkpoints[base])
        for event in self.events[base:count]:
            self._apply(state, event)
        return state


def graph_replay(steps):
    """Use semantic node/edge identities, including removals caused by merging."""
    events, previous, edge_ids = [], set(), {}
    for step in steps:
        graph = step.graph
        values = {f"node/{n}": n for n in graph.states}
        for edge in graph.edges:
            if edge not in edge_ids:
                edge_ids[edge] = len(edge_ids)
            index = edge_ids[edge]
            values[f"edge/{index}"] = index
        keys = set(values)
        changes = tuple((key, value) for key, value in values.items() if key not in previous)
        events.append(Event(changes + (("step", step), ("accepts", graph.accepts)), tuple(sorted(previous-keys))))
        previous = keys
    return Replay(events)
