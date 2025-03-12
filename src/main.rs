use std::process::exit;
use csv::Reader;
use clap::{Parser, Subcommand};
use users::get_effective_uid;

// modules
mod test_download;
mod test_upload;
mod test_stream;
mod test_host;
mod testbed;
mod trace;
mod webserver;
mod webclient;
mod rtnetlink_utils;

/// Emulator for packet loss caused by bridges
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// CSV file with loss trace of form relative_time,loss
    #[arg(id = "file", short, long)]
    trace_file: String,

    /// Path to a delay distribution file
    /// Defaults to /lib64/tc/pareto.dist
    #[arg(id = "distribution", short, long)]
    distribution_file: Option<String>,

    /// Test to run
    #[command(subcommand)]
    test: Test,
}

/// Test to run
#[derive(Subcommand)]
#[derive(Debug)]
#[command()]
enum Test {
    /// Download Test
    Download,
    /// Upload Test
    Upload,
    /// Stream Test
    Stream {
        /// Video file used for the stream test
        #[arg(id = "video", short, long)]
        video_file: String,

        /// ffmpeg style video bitrate
        /// defaults to 5000k
        #[arg(id = "bitrate", short, long)]
        video_bitrate: Option<String>,
    },
    /// Play trace on a host interface
    /// WARNING: this will replace your current qdisc
    Host {
        /// Host interface to be used for trace playback
        #[arg(short, long)]
        interface: String
    }
}

fn main() {
    // setup and checks
    let args = Args::parse();

    // we need to be root in order to create network namespaces or interfaces
    if get_effective_uid() != 0 {
        eprintln!("Elevated privileges are required \
            to create network namespaces or interfaces");
        exit(1);
    }

    // try to read file
    let mut rdr = Reader::from_path(args.trace_file.as_str()).unwrap_or_else(|_| {
        eprintln!("Could not open csv file {} for reading", args.trace_file.as_str());
        exit(1);
    });

    // setup test
    match args.test {
        Test::Download => test_download::run_test(
            &mut rdr, args.distribution_file.clone()),
        Test::Upload => test_upload::run_test(
            &mut rdr, args.distribution_file.clone()),
        Test::Stream {
            video_file: vfile,
            video_bitrate: vrate
        } => test_stream::run_test(
            &mut rdr, args.distribution_file.clone(), vfile.clone(), vrate.clone()),
        Test::Host {
            interface: iface
        } => test_host::run_test(
            &mut rdr, args.distribution_file.clone(), iface.clone()),
    }

    exit(0);
}
