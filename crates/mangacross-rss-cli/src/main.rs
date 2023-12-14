use clap::Parser;
use std::path::Path;
use tokio::time::Instant;
use tracing::info;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

use mangacross_rss::config::Config;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: String,
    #[arg(short, long)]
    output: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?;
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let start = Instant::now();

    let args = &Args::parse();

    // Use `std` instead of `tokio` due to the signature of `serde_json::fron_reader`.
    // This is fine since there is nothing to run in concurrent.
    let config = {
        use std::fs::File;
        use std::io::BufReader;
        let file = File::open(&args.config)?;
        let reader = BufReader::new(file);
        serde_json::from_reader::<_, Config>(reader)?
    };

    mangacross_rss::build_rss(&config, Path::new(&args.output)).await?;

    let end = start.elapsed();
    info!(
        "Done. {}.{:03} secs elapsed.",
        end.as_secs(),
        end.subsec_millis()
    );

    Ok(())
}
