#!/usr/bin/env python3
"""Compile the public lifecycle API and reject safe publication/lifetime escapes."""

import json
import pathlib
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
PREFIX = "use r2r::{LifecyclePublisher, LifecycleLoanedMessage, LoanedMessage, NativeMsg, Node, Publisher, std_msgs::msg::Int32};\n"
CASES = [
    (
        "gated_publication_and_endpoint_api",
        """
fn publish(p: &LifecyclePublisher<Int32>, native: &mut NativeMsg<Int32>) {
    p.clone().publish(&Int32 { data: 1 }).unwrap();
    p.publish_native(native).unwrap();
    p.topic_name().unwrap();
    p.get_gid().unwrap();
    p.get_actual_qos().unwrap();
    p.wait_for_subscribers().unwrap();
    p.wait_for_all_acked_async().unwrap();
}
fn loan<'node>(p: &LifecyclePublisher<Int32>, node: &'node mut Node) -> LifecycleLoanedMessage<'node, Int32> {
    p.borrow_loaned_message(node).unwrap()
}
fn publish_loan(mut loan: LifecycleLoanedMessage<'_, Int32>) {
    loan.data = 2;
    loan.publish().unwrap();
}
fn options(node: &mut r2r::LifecycleNode) -> LifecyclePublisher<Int32> {
    node.create_publisher_with_options("values", r2r::QosProfile::default(), r2r::PublisherOptions::default()).unwrap()
}
fn destroy(p: LifecyclePublisher<Int32>, node: &mut Node) {
    p.destroy(node);
}
fn timer(node: &mut r2r::LifecycleNode) -> r2r::LifecycleTimer {
    node.create_wall_timer_with_options(std::time::Duration::from_millis(5), r2r::TimerOptions::default()).unwrap()
}
fn timer_controls(timer: &mut r2r::LifecycleTimer, node: &mut Node) {
    timer.cancel(node).unwrap();
    timer.reset(node).unwrap();
    timer.period(node).unwrap();
    drop(timer.tick());
}
""",
        None,
    ),
    (
        "publisher_deref_escape",
        "fn escape(p: &LifecyclePublisher<Int32>) -> &Publisher<Int32> { p }",
        "E0308",
    ),
    (
        "publisher_clone_escape",
        "fn escape(p: &LifecyclePublisher<Int32>) -> Publisher<Int32> { p.clone() }",
        "E0308",
    ),
    (
        "publisher_field_escape",
        "fn escape(p: &LifecyclePublisher<Int32>) -> Publisher<Int32> { p.publisher.clone() }",
        "E0616",
    ),
    (
        "loan_deref_escape",
        "fn escape<'a, 'node>(loan: &'a LifecycleLoanedMessage<'node, Int32>) -> &'a LoanedMessage<'node, Int32> { loan }",
        "E0308",
    ),
    (
        "loan_field_escape",
        "fn escape<'node>(loan: LifecycleLoanedMessage<'node, Int32>) -> LoanedMessage<'node, Int32> { loan.message }",
        "E0616",
    ),
    (
        "timer_deref_escape",
        "fn escape(timer: &r2r::LifecycleTimer) -> &r2r::Timer { timer }",
        "E0308",
    ),
    (
        "timer_field_escape",
        "fn escape(timer: &r2r::LifecycleTimer) { let _ = timer.state; }",
        "E0616",
    ),
    (
        "loan_outlives_node",
        "fn escape(p: &LifecyclePublisher<Int32>, node: &mut Node) -> LifecycleLoanedMessage<'static, Int32> { p.borrow_loaned_message(node).unwrap() }",
        "lifetime may not live long enough",
    ),
]


def main():
    build = subprocess.run(
        ["cargo", "build", "-p", "r2r", "--lib", "--message-format=json"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    artifacts = [json.loads(line) for line in build.stdout.splitlines()]
    libraries = [
        pathlib.Path(filename)
        for artifact in artifacts
        if artifact.get("reason") == "compiler-artifact"
        and artifact["target"]["name"] == "r2r"
        for filename in artifact["filenames"]
        if filename.endswith(".rlib")
    ]
    if len(libraries) != 1:
        raise RuntimeError(f"expected one native r2r library, found {libraries}")
    library = libraries[0]
    dependencies = library.parent / "deps"
    if not dependencies.is_dir():
        dependencies = library.parent
    with tempfile.TemporaryDirectory(prefix="r2r-lifecycle-api-") as directory:
        source = pathlib.Path(directory) / "boundary.rs"
        for name, body, expected_error in CASES:
            source.write_text(PREFIX + body)
            result = subprocess.run(
                [
                    "rustc", "--edition=2021", "--crate-type=lib", "--emit=metadata",
                    "--crate-name=lifecycle_boundary", "--extern", f"r2r={library}",
                    "-L", f"dependency={dependencies}", "--out-dir", directory, str(source),
                ],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            if expected_error is None:
                valid = result.returncode == 0
            else:
                valid = result.returncode != 0 and expected_error in result.stderr
            if not valid:
                raise RuntimeError(f"{name}: unexpected compilation result\n{result.stderr}")
            print(f"PASS {name}")
    print(f"{len(CASES)} public API boundaries verified")


if __name__ == "__main__":
    main()
