use super::*;

#[test]
fn an_amend_rewrites_the_last_commit() {
    let root = repository("git-amend");
    assert!(
        Backend::Local.commit(&root, "nothing yet", true).is_err(),
        "no commit to amend"
    );
    write(&root, "a.txt", "a\n");
    Backend::Local
        .stage(&root, &[change("a.txt", GitStatus::Untracked)])
        .expect("a stage");
    Backend::Local
        .commit(&root, "first", false)
        .expect("a commit");
    Backend::Local
        .commit(&root, "first, reworded", true)
        .expect("an amend with nothing staged");
    let commits = Backend::Local.log(&root, 10).expect("a log");
    let subjects: Vec<&str> = commits
        .iter()
        .map(|commit| commit.subject.as_ref())
        .collect();
    assert_eq!(subjects, ["first, reworded"]);
}
