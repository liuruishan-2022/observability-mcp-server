use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, short)]
    config: String,
}

impl Args {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn config(&self) -> &str {
        self.config.as_str()
    }
}

impl ToString for Args {
    fn to_string(&self) -> String {
        format!("config: {}", self.config)
    }
}
