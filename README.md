# WP4

## dependecies

```
rust cargo iproute2
```

Additionally the kernel needs to be build with the following config options

```
CONFIG_NET_SCH_NETEM=m
CONFIG_NET_NS=y
```

## building

To build just run `cargo build --release` in the root of the cloned repo.
The resulting binary will be located under `target/release/bridge-loss-emu`.

## running

Creating/modifying/deleting network namespaces and interfaces requires elevated privileges so the program has to be run as root.

Distribution curves for the `--distribution` argument are shipped with the `iproute2` package and usually live under `/lib64/tc/` - but other distros might ship them different ways.

## TODO

- [ ] tshark packet capture
- [x] emulation scenarios
  - [x] download
        Implemented by spawning a `http` server that generates an
        infinite stream of data downloaded by a client
        (no actual data is written/read from disk)
  - [x] upload
        Implemented by spawning a `http` client that generates an
        infinite stream of data uploaded to a server
        (no actual data is written/read from disk)
  - [x] streaming (essentially rate limited download)
        Implemented by serving a video via `ffmpeg` as a http server
        and streaming from it using `mpv`.
        This allows testing different bandwidths via transcoding.
  - [x] ~~launcher for external app~~ host mode so users can experience
        loss from bridges at home
- [x] Auto generate data stream
- [x] cli
  - [x] scenario selection
  - [x] file selection
- [x] change netem params directly e.g. via sysfs (if possible)
      Now uses a forked version of the `rtnetlink` crate with custom messages
      (see `src/rtnetlink_utils.rs`)
- [ ] setup testbed interfaces directly via `rtnetlink` crate
