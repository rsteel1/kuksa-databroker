# Chapter 31 hints

## c31_e01_clap_args_env
Pattern: `#[arg(long, alias = "addr", env = "LEARN_KUKSA_ADDR", default_value = "127.0.0.1")]`.
For a typed default use `default_value_t = 55555`. For the VSS list:
`#[arg(long = "vss", value_delimiter = ',')]`. A `bool` field with
`#[arg(long)]` becomes a flag. `Option<usize>` with `#[arg(long, env = "...")]`
is optional; clap derives `--worker-threads` from `worker_threads`.
`socket_addr`: `let ip: IpAddr = self.address.parse()?; Ok(SocketAddr::new(ip, self.port))`.

## c31_e02_tracing_basics
`filter_from`: `spec.and_then(|s| EnvFilter::try_new(s).ok()).unwrap_or_else(|| EnvFilter::new("info"))`.
Events: `warn!(path, "rejected negative value");` and
`debug!(path, value, "value updated");` — a bare identifier inside the macro
records a field with that name. Put `#[tracing::instrument(skip(value))]`
directly above `fn apply_update`.

## c31_e03_graceful_shutdown
In `spawn_worker`: `let mut shutdown = self.shutdown_trigger.subscribe();`
and `Arc::clone` the counters *before* `tokio::spawn(async move { ... })`.
Inside: `let mut ticker = tokio::time::interval(period); loop { tokio::select! { _ = shutdown.recv() => break, _ = ticker.tick() => { ticks.fetch_add(1, Ordering::Relaxed); } } }`,
then bump `stopped`. Push the JoinHandle into `self.workers`.
`run_until`: `signal.await;` `let _ = self.shutdown_trigger.send(());`
then `tokio::time::timeout(grace, futures::future::join_all(self.workers)).await`
and map `Ok(_)` → `Ok(())`, `Err(_)` → `Err(TimedOut)`.

## c31_e04_cfg_feature_gates
Put `#[cfg(target_os = "linux")]` above the "systemd" function and
`#[cfg(not(target_os = "linux"))]` above the "noop" one — now exactly one
exists on any platform. A field can be gated too: `#[cfg(unix)] unix_socket: ...`
— and so must be the line that initialises it in `default_config`.
`build_kind`: `if cfg!(debug_assertions) { "debug" } else { "release" }`.
