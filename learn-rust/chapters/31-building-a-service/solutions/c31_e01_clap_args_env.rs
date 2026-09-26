// c31_e01_clap_args_env.rs
//
// CONCEPT: command-line + environment configuration with clap's derive API.
// GUIDE:   chapters/31-building-a-service/guide.md  (section "Configuration with clap")
// DATABROKER: databroker/src/main.rs — `Command::new("Kuksa Databroker")` and its `Arg::new(...)`
//             chain (`.long("address").alias("addr").env("KUKSA_DATABROKER_ADDR").default_value(...)`)
// EXPECT:  tests fail: clap panics "Argument 'insecure' is positional and it must take a value..."
//          (without `#[arg(long)]` every field is a positional argument)
//
// The databroker uses clap's *builder* API (a long chain of `Arg::new`). The
// *derive* API expresses the same thing as a struct: each field is an
// argument, attributes say how. A field of type `Option<T>` is optional,
// `Vec<T>` can repeat, `bool` is a flag. Every argument can also come from an
// environment variable (12-factor style config — handy in containers):
// precedence is  command line  >  env var  >  default.
//
// YOUR TASK
//   Add `#[arg(...)]` attributes so that:
//     address         --address <IP>, alias --addr, env LEARN_KUKSA_ADDR, default "127.0.0.1"
//     port            --port <PORT>, env LEARN_KUKSA_PORT, default 55555 (hint: default_value_t)
//     vss_files       --vss a.json,b.json   (comma separated; hint: value_delimiter)
//     insecure        --insecure            (a flag)
//     worker_threads  --worker-threads <N>, env LEARN_KUKSA_WORKER_THREADS, optional
//   and implement `socket_addr`.
//   Do not edit the tests.

use clap::Parser;
use std::net::{AddrParseError, IpAddr, SocketAddr};

#[derive(Parser, Debug)]
#[command(
    name = "mini-databroker",
    version,
    about = "A tiny databroker-shaped CLI"
)]
struct Args {
    /// Bind address
    #[arg(
        long,
        alias = "addr",
        env = "LEARN_KUKSA_ADDR",
        default_value = "127.0.0.1"
    )]
    address: String,

    /// Bind port
    #[arg(long, env = "LEARN_KUKSA_PORT", default_value_t = 55555)]
    port: u16,

    /// Populate the broker from these VSS files
    #[arg(long = "vss", value_delimiter = ',')]
    vss_files: Vec<String>,

    /// Allow connections without TLS
    #[arg(long)]
    insecure: bool,

    /// Number of tokio worker threads (default: number of cores)
    #[arg(long, env = "LEARN_KUKSA_WORKER_THREADS")]
    worker_threads: Option<usize>,
}

impl Args {
    fn socket_addr(&self) -> Result<SocketAddr, AddrParseError> {
        let ip: IpAddr = self.address.parse()?;
        Ok(SocketAddr::new(ip, self.port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn definition_is_consistent() {
        // clap's own sanity check of the argument definitions.
        Args::command().debug_assert();
    }

    #[test]
    fn defaults() {
        let args = Args::try_parse_from(["mini-databroker"]).expect("no args needed");
        assert_eq!(args.address, "127.0.0.1");
        assert_eq!(args.port, 55555);
        assert!(args.vss_files.is_empty());
        assert!(!args.insecure);
    }

    #[test]
    fn everything_on_the_command_line() {
        let args = Args::try_parse_from([
            "mini-databroker",
            "--address",
            "0.0.0.0",
            "--port",
            "6000",
            "--vss",
            "vss.json,overlay.json",
            "--insecure",
        ])
        .expect("valid args");
        assert_eq!(args.address, "0.0.0.0");
        assert_eq!(args.port, 6000);
        assert_eq!(args.vss_files, ["vss.json", "overlay.json"]);
        assert!(args.insecure);
    }

    #[test]
    fn alias_works() {
        let args = Args::try_parse_from(["mini-databroker", "--addr", "10.0.0.1"]).unwrap();
        assert_eq!(args.address, "10.0.0.1");
    }

    #[test]
    fn bad_port_is_rejected_by_the_type() {
        assert!(Args::try_parse_from(["mini-databroker", "--port", "fast"]).is_err());
        assert!(
            Args::try_parse_from(["mini-databroker", "--port", "70000"]).is_err(),
            "u16 max is 65535"
        );
    }

    #[test]
    fn env_var_is_used_when_flag_missing() {
        // Only this test touches this variable, so parallel tests can't race on it.
        std::env::set_var("LEARN_KUKSA_WORKER_THREADS", "3");
        let args = Args::try_parse_from(["mini-databroker"]).unwrap();
        assert_eq!(args.worker_threads, Some(3));
        let args = Args::try_parse_from(["mini-databroker", "--worker-threads", "8"]).unwrap();
        assert_eq!(args.worker_threads, Some(8), "command line beats env");
        std::env::remove_var("LEARN_KUKSA_WORKER_THREADS");
    }

    #[test]
    fn socket_addr() {
        let args = Args::try_parse_from(["mini-databroker", "--port", "1234"]).unwrap();
        assert_eq!(
            args.socket_addr().unwrap(),
            "127.0.0.1:1234".parse().unwrap()
        );
        let args = Args::try_parse_from(["mini-databroker", "--address", "localhost"]).unwrap();
        assert!(
            args.socket_addr().is_err(),
            "a hostname is not an IP address"
        );
    }
}
