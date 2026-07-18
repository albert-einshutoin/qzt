#[test]
fn bench_profile_runs_only_the_named_profile_test() {
    let makefile = include_str!("../Makefile");
    let recipe = makefile
        .split("\nbench-profile:")
        .nth(1)
        .and_then(|tail| tail.split("\nbench-profile-matrix:").next())
        .expect("bench-profile recipe should exist before bench-profile-matrix");

    assert!(
        recipe.contains("release_benchmark_profile -- --ignored --exact --nocapture"),
        "bench-profile must use libtest's exact filter so it cannot also run the matrix: {recipe}"
    );
}

#[test]
fn bench_profile_matrix_runs_only_the_named_matrix_test() {
    let makefile = include_str!("../Makefile");
    let recipe = makefile
        .split("\nbench-profile-matrix:")
        .nth(1)
        .expect("bench-profile-matrix recipe should exist");

    assert!(
        recipe.contains("release_benchmark_profile_matrix -- --ignored --exact --nocapture"),
        "bench-profile-matrix must use libtest's exact filter: {recipe}"
    );
}
