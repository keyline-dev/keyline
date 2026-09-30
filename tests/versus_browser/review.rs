//! The owner's blind review: anonymised PNG sets to pass or fail, and how
//! often the owner agrees with the judge.

use crate::prompts::Task;
use crate::{bench_root, random_hex, table};

/// Exports the counted runs for the owner's blind review: shuffled PNG sets
/// in `review/<task>/set-NN/`, a `review.tsv` to fill in (pass as 1 or 0,
/// look 1–5) and a `key.tsv` naming each set's run, not to be opened
/// until the review is done.
#[test]
#[ignore = "writes the review folder; run with --ignored"]
fn export_review() {
    let runs = table::runs();
    for task in [Task::ReferenceAd, Task::SpeakerCard] {
        let dir = bench_root().join("review").join(task.name());
        assert!(
            !dir.join("review.tsv").exists(),
            "{} exists; delete it to export again",
            dir.display()
        );
        let mut sets: Vec<(String, &table::Run)> = runs
            .iter()
            .filter(|r| r.counted() && r.summary["task"] == task.name())
            .filter(|r| r.summary["filesOk"] == true)
            .map(|r| (random_hex(), r))
            .collect();
        sets.sort_by(|a, b| a.0.cmp(&b.0));
        let (mut review, mut key) = (
            String::from("set\tpass\tlook\tnotes\n"),
            String::from("set\trun\n"),
        );
        for (i, (_, run)) in sets.iter().enumerate() {
            let set = format!("set-{:02}", i + 1);
            std::fs::create_dir_all(dir.join(&set)).unwrap();
            for (size, ..) in task.sizes() {
                let png = format!("{size}.png");
                std::fs::copy(run.dir.join(&png), dir.join(&set).join(&png)).unwrap();
            }
            review.push_str(&format!("{set}\t\t\t\n"));
            let name = run.dir.file_name().unwrap().to_string_lossy();
            key.push_str(&format!("{set}\t{name}\n"));
        }
        std::fs::write(dir.join("review.tsv"), review).unwrap();
        std::fs::write(dir.join("key.tsv"), key).unwrap();
        println!("{} sets in {}", sets.len(), dir.display());
    }
}

/// How often the owner's blind review agrees with the judge, once filled in.
pub fn agreement(runs: &[table::Run]) -> String {
    let mut out = String::new();
    for task in [Task::ReferenceAd, Task::SpeakerCard] {
        let dir = bench_root().join("review").join(task.name());
        let (Ok(review), Ok(key)) = (
            std::fs::read_to_string(dir.join("review.tsv")),
            std::fs::read_to_string(dir.join("key.tsv")),
        ) else {
            continue;
        };
        let (mut agree, mut total) = (0, 0);
        for line in review.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            let (Some(set), Some(pass)) = (cols.first(), cols.get(1).filter(|p| !p.is_empty()))
            else {
                continue;
            };
            let Some(name) = key
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{set}\t")))
            else {
                continue;
            };
            let judged = runs
                .iter()
                .find(|r| r.summary["task"] == task.name() && r.dir.ends_with(name))
                .and_then(table::Run::correct);
            if let Some(judged) = judged {
                total += 1;
                agree += usize::from(judged == (*pass == "1"));
            }
        }
        if total > 0 {
            out.push_str(&format!(
                "{}: owner and judge agree on {agree}/{total} runs\n",
                task.name()
            ));
        }
    }
    out
}
