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

## running

Creating/modifying/deleting network namespaces and interfaces requires elevated privileges so the program has to be run as root.

In case of `No distribution data for pareto (/lib/tc/pareto.dist: No such file or directory)` errors set the correct `TC_LIB_DIR`. `tc` seems to be somewhat broken on some distros and tries to find it under build time configured `LIBDIR/tc`.

## TODO

- [ ] tshark packet capture
- [ ] emulation scenarios
  - [x] download
  - [x] upload
  - [ ] streaming (essentially rate limited download)
  - [ ] launcher for external app
- [x] Auto generate data stream
- [x] cli
  - [x] scenario selection
  - [x] file selection
- [x] change netem params directly e.g. via sysfs (if possible)
      Now uses a forked version of the `rtnetlink` crate with custom messages
      (see `src/rtnetlink_utils.rs`)
- [ ] setup testbed interfaces directly via `rtnetlink` crate
