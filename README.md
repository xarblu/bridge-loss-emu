# WP4

In case of `No distribution data for pareto (/lib/tc/pareto.dist: No such file or directory)` errors set the correct `TC_LIB_DIR`. `tc` seems to be somewhat broken and tries to find it under build time configured `LIBDIR/tc`.
