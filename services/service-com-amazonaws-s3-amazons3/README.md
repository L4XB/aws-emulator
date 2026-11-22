# service-com-amazonaws-s3-amazons3

Emulator implementation for the `com-amazonaws-s3-amazons3` service. This crate is
self-contained: building it does NOT rebuild the other services.

```bash
cargo build -p service-com-amazonaws-s3-amazons3
cargo test  -p service-com-amazonaws-s3-amazons3
```

## Layout

```
service-com-amazonaws-s3-amazons3/
├── build.rs              # calls emulator_core_build_support::generate(...)
├── smithy-model/
│   └── com-amazonaws-s3-amazons3.json      # the Smithy model — drives routing + wire shapes
├── OPERATIONS.md         # every operation's contract, re-rendered from the model
├── tests/
│   └── state_roundtrip.rs  # compiled example of the state API
└── src/
    ├── lib.rs            # #[smithy_service] decl (generated — do not hand-edit)
    ├── service.rs        # impl Service — can_handle + routing/refusal overrides (yours to edit)
    ├── state.rs          # ComAmazonawsS3Amazons3State — this service's in-memory state
    ├── scaffolded.rs     # Scaffolded<E> — the 501 stubs return, and `validation` for a 400
    └── ops/              # ONE FILE PER OPERATION
```

## The website surface

A bucket configured for static website hosting (`PutBucketWebsite`) is served on a surface
distinct from the REST API, reachable at two addresses:

- the bucket's own website host — `<bucket>.s3-website-<Region>.amazonaws.com`, or
  `<bucket>.s3-website.<Region>.amazonaws.com` — for a client that resolves Amazon's names;
- `<emulator endpoint>/_aws/s3-website/<bucket>/<key>`, for a client configured with this
  emulator's one endpoint, which resolves no such name. It is the reserved data-plane path
  this workspace addresses a local plane by (`service_common::data_plane_addressing`).

Both answer the one surface, from the bucket's stored configuration and objects: the index
document of a directory, the error document of a failed read, HTTP 301 to a stored
`x-amz-website-redirect-location`, the redirects the routing rules call for, and 405 for
anything but a read. The REST address `/<bucket>/<key>` is untouched by all of it and keeps
serving the object's own bytes (`src/service/website_endpoint.rs`).

## Implementing an operation

Each file in `src/ops/` owns exactly one operation and declares which one
via an `@op` header comment. `build.rs` globs this directory and generates
both the module list and the operation set, so adding a file is all that
is needed — there is no central registry to update.

Every generated stub starts out **scaffolded, not implemented**: it is fully typed
and registered, and it answers **HTTP 501 `NotImplemented`** until you write it. It
deliberately does not return an empty success — a hollow `200` with a zeroed body is
indistinguishable from a real implementation to a client, and to any check that only
asserts the call succeeded.

Each operation is a struct that owns the shared engine (constructor-injected) and
implements `smithy_server_runtime::Operation`. The framework constructs one per
service via `OpName::new(engine)` and drives it through the centralized request walk:
deserialize → validate → `handle` → serialize. You write ONLY the `handle` body.

To implement an operation, replace the `Err(..)` in `handle` with real behaviour and
delete the `SCAFFOLDED, NOT IMPLEMENTED` marker line at the top of the file. That
marker is how the still-stubbed operations are counted:

```sh
grep -rl '^//! SCAFFOLDED, NOT IMPLEMENTED:' */src/ops/
```

```rust
//! @op CreateThing

impl<Eng: Send + Sync + 'static> smithy_server_runtime::Operation for CreateThing<Eng> {
    type Input = CreateThingInput;
    type Output = CreateThingOutput;
    type Error = CreateThingError;

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &smithy_server_runtime::OperationContext<'_>,
    ) -> Result<Self::Output, Self::Error> {
        // Scaffolded: `Err(crate::scaffolded::Scaffolded::not_implemented("CreateThing"))`.
        // Modeled failures: `Err(CreateThingErrorModeled::Variant(..).into())`.
        /* ... */
    }
}
```

## State

A handler reaches this service's store through the engine held on `self`
(constructor-injected). The framework builds ONE shared engine and hands it to every
operation of this crate through its `new(engine)` constructor, so every handler in
this crate sees the same store:

```rust
async fn handle(
    &self,
    input: Self::Input,
    ctx: &smithy_server_runtime::OperationContext<'_>,
) -> Result<Self::Output, Self::Error> {
    let Some(state) = crate::state::service_state(&*self.engine) else {
        return Err(/* mounted on an engine that holds no state: a modelled server error */);
    };
    // A LOCKABLE handle, not the value — read and mutate through the guard:
    state.lock().your_field = /* ... */;
}
```

> **Never write `engine::InMemoryEngine::new()` inside a handler.** `engine` is the
> name of a CRATE as well as of the field, so that line compiles — and silently
> gives the request a private, empty store that is dropped when it returns. The write
> looks like it succeeded and the next call sees nothing. Reach the shared store
> through `self.engine`, never a fresh engine.

State lives in the SINGLE shared engine, keyed by type. Reach it through
`crate::state::service_state`, which `src/state.rs` generates.

Do not add a second engine crate — the router and every service must share
one `InMemoryEngine` type or the engine the router injects cannot be handed to this
service's operations.

### Experimental current-object expiration

For a disposable local calibration bucket, set
`AWS_EMULATOR_EXPERIMENTAL_S3_CURRENT_EXPIRATION_BUCKET` to its exact name
before starting a fresh engine. By default lifecycle configuration remains
passive. An authorized, unfiltered ListObjectVersions first-page request with default
parameters reconciles that bucket under its ordinary API. GET and HEAD stay
passive. Deleting the selected bucket disarms the mode for later recreations.
The mode skips buckets with more than 64 distinct stored keys. Reconciliation is demand-driven, under the store lock. No background deadline
or notification delivery is modeled.

An enabled matching current Expiration with Days or Date removes the current
unversioned object or adds a single versioned delete marker, subject to store
limits. Explicit reads of previous version IDs retain their bytes. The
calibration skips directory, suspended, replicated, notification-configured
and Object Lock buckets and versions carrying replication, retention or legal
hold state. Noncurrent expiration, storage transitions, expired marker removal
and multipart cleanup are outside this mode.
