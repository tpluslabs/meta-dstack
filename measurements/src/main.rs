use std::env;
use std::fs;
use std::process;

use sha2::Digest;

fn aggregate_pods(pods: Vec<[u8;48]>) -> Vec<u8> {
    pods.concat().to_vec()
}

fn powerset<T: Clone>(set: &[T]) -> Vec<Vec<T>> {
    // nb: we probably don't want to powerset here just have clear guidelines
    // on how to deploy in a correct sequence so we need to check less cases, also
    // because powerset is actually wrong here it should be permutations and for each
    // "lenght".
    let n = set.len();
    let mut result = Vec::new();

    for mask in 1..(1 << n) {
        let mut subset = Vec::new();
        for i in 0..n {
            if mask & (1 << i) != 0 {
                subset.push(set[i].clone());
            }
        }
        result.push(subset);
    }

    result
}


fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: measurements <pod1> <pod2> ...");
        process::exit(1);
    }

    let mut loaded_pods = Vec::new();

    for path in &args {
        if let Ok(pod_config) = fs::read(path) {
            let pod_hash: [u8;48] = sha2::Sha384::digest(&pod_config).try_into().unwrap();
            loaded_pods.push(pod_hash);
        } else {
            println!("warn: file doesn't exist")
        }
    }

    for subset in powerset(&loaded_pods) {
        let aggregated = aggregate_pods(subset.clone());
        
        println!("Measurement: {}", hex::encode(aggregated));
    }
}
