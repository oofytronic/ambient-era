use std::time::Instant;

fn peak_rss_kib() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "small".into());
    match mode.as_str() {
        "small" => {
            let t = Instant::now();
            let r = openmls_eval::small_scenario().expect("scenario failed");
            println!("{r:#?}");
            println!("elapsed: {:?}", t.elapsed());
            println!("peak RSS: {} KiB", peak_rss_kib().unwrap_or(0));
        }
        "scale" => {
            for n in [2usize, 10, 100, 500] {
                let t = Instant::now();
                let r = openmls_eval::scale_scenario(n).expect("scale failed");
                println!("{r:?} in {:?}", t.elapsed());
            }
            println!("peak RSS: {} KiB", peak_rss_kib().unwrap_or(0));
        }
        other => eprintln!("unknown mode {other}"),
    }
}
