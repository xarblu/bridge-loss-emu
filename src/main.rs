use std::process::exit;
use csv::Reader;
use clap::{Parser, Subcommand};
use users::get_effective_uid;

// modules
mod download_test;
mod testbed;
mod trace;
mod webserver;
mod downloader;

/// Emulator for packet loss caused by bridges
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// CSV file with loss trace of form relative_time,loss
    #[arg(id = "file", short, long)]
    trace_file: String,

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
    #[command()]
    Download {
        /// File used in the download test
        #[arg(short, long)]
        dl_test_file: String,
    },
    /// Upload Test
    Upload,
    /// Stream Test
    Stream
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
        Test::Download { dl_test_file: file } => {
            download_test::run_test(&mut rdr, file);},
        Test::Upload => {eprintln!("Not implemented"); exit(1);},
        Test::Stream => {eprintln!("Not implemented"); exit(1);},
    }

}
