//! `cargo test -p structurizr-evals` — grades every committed reference
//! candidate against its task's rubric. This is the eval suite's own
//! regression test: it stays green as long as the reference solutions and
//! the rubric-checking logic agree. To grade an agent's attempt instead,
//! see `rust/structurizr-evals/README.md`.

use std::path::Path;

use structurizr_dsl::parse_file;

#[test]
fn reference_candidates_pass_their_rubric() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tasks_dir = crate_dir.join("tasks");
    let tasks = structurizr_evals::load_tasks(&tasks_dir).expect("failed to load eval tasks");
    assert!(
        !tasks.is_empty(),
        "no eval tasks found in {}",
        tasks_dir.display()
    );

    let mut any_failed = false;
    for task in &tasks {
        let candidate_path = crate_dir.join(&task.candidate);
        let workspace = match parse_file(&candidate_path) {
            Ok(ws) => ws,
            Err(e) => {
                any_failed = true;
                eprintln!(
                    "[{}] FAIL — candidate {} failed to parse: {e:#}",
                    task.id,
                    candidate_path.display()
                );
                continue;
            }
        };

        let failures = structurizr_evals::grade(&workspace, &task.rubric);
        if failures.is_empty() {
            eprintln!("[{}] PASS — {}", task.id, task.title);
        } else {
            any_failed = true;
            eprintln!("[{}] FAIL — {}", task.id, task.title);
            for f in &failures {
                eprintln!("    - {f}");
            }
        }
    }

    assert!(
        !any_failed,
        "one or more eval tasks failed their rubric (see stderr above; rerun with `cargo test -p structurizr-evals -- --nocapture`)"
    );
}
