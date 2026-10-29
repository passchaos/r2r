#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
fn main() -> r2r::Result<()> {
    use r2r::{std_msgs::msg::UInt64, FutureExecutionOptions, NodeOptions, QosProfile};
    use std::{
        io::{self, Write},
        time::Duration,
    };
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "usage: type_description_interop client|server node-name");
    let role = &args[1];
    let name = &args[2];
    let context = r2r::Context::create()?;
    if role == "server" {
        let mut node = r2r::Node::create_with_options(
            context,
            name,
            "",
            NodeOptions::default().start_type_description_service(true),
        )?;
        let _publisher =
            node.create_publisher::<UInt64>(&format!("/{name}/values"), QosProfile::default())?;
        println!(
            "{{\"kind\":\"ready\",\"role\":\"server\",\"hash\":\"{}\"}}",
            r2r::message_type_hash::<UInt64>()?
        );
        io::stdout().flush().unwrap();
        let (finish, completed) = futures::channel::oneshot::channel();
        let input = std::thread::spawn(move || {
            let mut line = String::new();
            io::stdin().read_line(&mut line).unwrap();
            assert_eq!(line.trim(), "finish");
            let _ = finish.send(());
        });
        node.spin_until_future_complete_with_options(
            async { completed.await.map_err(|_| r2r::Error::RCL_RET_ERROR) },
            Duration::from_secs(20),
            FutureExecutionOptions::wake_driven(),
        )?;
        input.join().unwrap();
    } else {
        assert_eq!(role, "client");
        let mut node = r2r::Node::create(context, "type_description_rust_client", "")?;
        let client = node.create_type_description_client(&format!("/{name}"))?;
        let hash = r2r::message_type_hash::<UInt64>()?;
        let ready = client.wait_for_service()?;
        let (with_sources, without_sources) = node.spin_until_future_complete_with_options(
            async {
                ready.await?;
                let a = client.describe("std_msgs/msg/UInt64", &hash, true)?.await?;
                let b = client
                    .describe("std_msgs/msg/UInt64", &hash, false)?
                    .await?;
                Ok((a, b))
            },
            Duration::from_secs(10),
            FutureExecutionOptions::wake_driven(),
        )?;
        assert_eq!(with_sources.type_description.type_description.fields.len(), 1);
        assert_eq!(with_sources.type_description.type_description.fields[0].name, "data");
        assert!(with_sources
            .type_sources
            .iter()
            .any(|source| source.type_name == "std_msgs/msg/UInt64"
                && !source.raw_file_contents.as_bytes().is_empty()));
        assert!(without_sources.type_sources.is_empty());
        let source = with_sources
            .type_sources
            .iter()
            .find(|source| source.type_name == "std_msgs/msg/UInt64")
            .unwrap();
        let distro = std::env::var("ROS_DISTRO").unwrap();
        let path = format!("/opt/ros/{distro}/share/std_msgs/msg/UInt64.{}", source.encoding);
        let exact_source = source.raw_file_contents.as_bytes() == std::fs::read(path).unwrap();
        let mut source_hex = String::with_capacity(source.raw_file_contents.len() * 2);
        use std::fmt::Write as _;
        for byte in source.raw_file_contents.as_bytes() {
            write!(&mut source_hex, "{byte:02x}").unwrap();
        }
        println!(
            "{}",
            serde_json::json!({"kind":"result", "implementation":"r2r", "type_name":"std_msgs/msg/UInt64", "hash":hash.to_string(), "fields":1, "sources":true, "omission":true, "exact_source":exact_source, "source_hex":source_hex, "encoding":source.encoding})
        );
    }
    Ok(())
}

#[cfg(not(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
)))]
fn main() {
    println!("type-description interoperability requires Jazzy or newer");
    std::process::exit(2);
}
