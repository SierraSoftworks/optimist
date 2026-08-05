# Workbench

A Vue frontend for the design server. It opens a directory of designs, edits
them, solves them, and weighs the proposals attached to them.

## Running it

The workbench is useless on its own; it needs a server holding designs.

```sh
cargo run -- serve --designs ./examples       # in the repository root
npm --prefix workbench run dev                # http://127.0.0.1:5173
```

Point it somewhere else with `OPTIMIST_API_URL`. The dev server proxies `/api`,
including the WebSocket upgrade the change feed needs.

```sh
npm --prefix workbench test          # unit tests
npm --prefix workbench run build     # type-check and bundle
```

## How it stays current

Editing writes through `POST /mutations` and nothing is written into the local
cache from the response. The same edit arrives back over the design's WebSocket
feed, and that is the path that updates the screen.

Doing it that way means an edit made here and an edit made by somebody else in
another tab are handled by identical code, rather than by two implementations
that can disagree. It also means the design is patched rather than replaced: the
feed carries the edit, `applyMutation` replays it onto the entity it names, and a
field being typed into elsewhere on the page is left alone. Refetching would
discard it.

A `lagged` message means changes were dropped and the local copy cannot be
repaired by replay. That is the one case that refetches.

## Charts

`DistributionChart` draws a kernel density estimate rather than a summary,
because the summaries cannot express the result that matters most.

The solver relaxes over aligned draws, so each draw settles on its own fixed
point. A design near a fold therefore returns a genuine mixture — some draws
healthy, some collapsed — and a mean, a median and a percentile pair describe
that mixture exactly as they would describe one broad unimodal spread. The
density estimate in `domain/density.ts` preserves this distinction visually, and
when it finds more than one mode the chart says so in words rather than leaving
it to a few pixels of dip. The API's draws and mixture metadata preserve the
same information for other clients.

That module carries the reasoning for its bandwidth choice, including an approach
that was tried, measured, and removed for reporting modes that were not there.

## Layout

```
src/api/          wire types, HTTP client, WebSocket feed
src/composables/  query and mutation hooks
src/components/   panels and the distribution chart
src/domain/       density estimation, mutation replay, formatting
src/stores/       what is on screen, and nothing that can go stale
```

Anything fetched from the server lives in the query cache rather than the store,
so state that can go stale is never held somewhere nothing knows to refresh it.
