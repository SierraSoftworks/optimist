# The workbench and shared editing

The workbench is how most people use Optimist. Serve a directory of designs and
it, or any other client, talks to the same server.

```sh
optimist serve --designs ./designs
```

![The workbench's design picker, listing the designs the server is holding as cards with their summaries.](/screenshots/designs.png)

## What a workspace is

A workspace is a directory whose subdirectories are designs. Each is otherwise
independent: its own in-memory state, its own change feed. Editing one neither
blocks nor notifies anyone working on another, and nothing is shared but the
directory they happen to live under.

An engineer rarely reasons about one system in isolation. The design being
changed sits next to the one it depends on and the one that replaced it last
year, and being able to open each without restarting anything is the difference
between a tool and a batch job.

Listing a workspace reads only the header of each design, which is cheap and
stays cheap as designs grow. A design is fully loaded the first time someone
opens it and stays loaded afterwards, because everyone editing it has to share
one copy for the change feed to mean anything.

A design that cannot be read appears in the listing with the reason it could not,
under an `unreadable` field. Hiding it would be worse: an engineer who cannot
find a design they know exists has no way to discover that its file is malformed.

## Editing is a stream of mutations

There is no revision to send, nothing to retry, and no conflict to resolve. A
mutation names exactly one entity, and the last writer to touch it wins.

```http
POST /api/v1/designs/checkout/mutations
Content-Type: application/json

{
  "mutations": [
    {
      "kind": "set_component",
      "component": {
        "id": "api",
        "name": "Checkout API",
        "type": "compute",
        "properties": { "service_time": "0.02", "parallelism": "8" }
      }
    }
  ]
}
```

A mutation is tagged with `kind`; the `type` inside a component is the component
type it adopts.

Two editors working on different components never contend. Two editors working on
the same component replace it whole, so there is no interleaving of field edits
to reconcile. The full list of mutation types is in the
[HTTP API reference](../reference/http-api.md#mutations).

A mutation that would break the design structurally is rejected — a relationship
to a component that does not exist, a self-loop, a scale unit claiming a
component another already owns. A design that is merely *incomplete* is accepted,
because that is what somebody has halfway through making a change. A component
missing a required property is stored happily; it fails when the design is
solved, which is where the message belongs.

Mutations in one request apply in order and stop at the first failure. Earlier
ones stand, and the response says how many were applied.

## The change feed

```text
GET /api/v1/designs/checkout/feed      (WebSocket upgrade)
```

The socket sends the design as its first message and streams changes after it:

```json
{ "type": "snapshot", "name": "Checkout", "summary": "", "model": {}, "sequence": 41 }
{ "type": "change", "sequence": 42, "mutation": { "kind": "set_component", "component": {} } }
```

Feed messages are tagged with `type`; the mutation inside a `change` is tagged
with `kind`.

Opening with a snapshot has no gap to reason about. Fetching a design and then
subscribing would drop anything that changed in between; subscribing and then
fetching would deliver changes the fetch already contains.

The feed carries mutations rather than whole designs. Sending the whole design
would be simpler to implement and worse to use: it would clobber whatever the
recipient was midway through editing, and it would cost the size of the model on
every keystroke somebody else typed. Sending the mutation means a client applies
exactly what the server applied, to exactly the entity that changed, leaving
everything it is working on untouched.

A client that falls far enough behind receives a `lagged` message instead of the
backlog:

```json
{ "type": "lagged", "missed": 312 }
```

That is the one case where a client refetches, because the local copy cannot be
repaired by replay.

`sequence` increments on every applied change. An editor recognises its own
edits arriving back by the sequence the write returned.

## Persistence

Edits are held in memory and written back to the design directory after a short
quiet period, and again on shutdown. The whole directory is rewritten in
canonical form: `_system.yaml` for the design-wide document, one file per
component under `components/`, and relationships stored with the component they
leave.

Component files the model no longer contains are removed, because a stale file
would be read back as a component nobody declared.

The practical consequence is that a design edited in the workbench produces a
clean diff. It is meant to be reviewed in the same repository as the system it
describes, so `git diff` after a session should read as the change that was
actually made.

## Serving the workbench

The server serves the API and the Vue workbench from the same process. Browser
routes fall back to `index.html`; generated files under `/assets` use a one-year
immutable cache while HTML revalidates on every load. `/api` and every `/api/*`
path remain JSON-only and never fall back to the application, so a mistyped
endpoint is a 404 with an advice field rather than a page of HTML.

A released binary embeds a frontend build. A debug build from a checkout looks
for `workbench/dist` beside the repository. Either can be overridden:

```sh
optimist serve --web-root /path/to/dist
OPTIMIST_WEB_ROOT=/path/to/dist optimist serve
```

Rust builds do not invoke Node. If no valid web root is configured or discovered,
the server remains API-only.

For front-end work, run Vite's dev server instead. It proxies `/api`, including
the WebSocket upgrade the feed needs.

```sh
optimist serve --designs ./designs      # in one terminal
npm --prefix workbench run dev          # http://127.0.0.1:5173
```

Point the workbench at another server with `OPTIMIST_API_URL`.

## How the workbench stays current

Worth knowing if you are writing a client of your own: the workbench writes
through `POST /mutations` and puts *nothing* from the response into its local
cache. The same edit arrives back over the design's feed, and that is the path
that updates the screen.

Doing it that way means an edit made locally and an edit made by somebody else in
another tab are handled by identical code, rather than by two implementations
that can disagree. It also means the design is patched rather than replaced: the
feed carries the mutation, the client replays it onto the entity it names, and a
field being typed into elsewhere on the page is left alone.

## Health

```sh
curl http://127.0.0.1:3000/api/v1/health
```

```json
{ "status": "ok", "version": "0.1.0", "designs": 2, "unsaved": 0 }
```

`unsaved` is the number of loaded designs with edits that have not yet been
written back.

## What is not implemented

There is no authentication and no authorisation. Anyone who can reach the port
can read and edit every design in the workspace, so run it on a loopback address
or behind something that does enforce access.
